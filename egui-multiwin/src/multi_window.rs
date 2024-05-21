//! This defines the MultiWindow struct. This is the main struct used in the main function of a user application.

/// Create the dynamic tracked_window module for a egui_multiwin application. Takes three arguments. First argument is the type name of the common data structure for your application.
/// Second argument is the type for custom events (or egui_multiwin::NoEvent if that functionality is not desired). Third argument is the enum of all windows. It needs to be enum_dispatch.
#[macro_export]
macro_rules! tracked_window {
    ($common:ty,$window:ty) => {
        pub mod tracked_window {
            //! This module covers definition and functionality for an individual window.

            use std::collections::HashMap;
            use std::{mem, sync::{Arc, Mutex, MutexGuard}};

            use super::multi_window::NewWindowRequest;

            use egui_multiwin::egui;
            use egui::viewport::{DeferredViewportUiCallback, ViewportBuilder, ViewportId, ViewportIdSet};
            use egui_multiwin::egui_glow_async::EguiGlow;
            use egui_multiwin::egui_glow_async::{self, glow};
            use egui_multiwin::glutin::context::{NotCurrentContext, PossiblyCurrentContext};
            use egui_multiwin::glutin::prelude::{GlConfig, GlDisplay};
            use egui_multiwin::glutin::surface::SurfaceAttributesBuilder;
            use egui_multiwin::glutin::surface::WindowSurface;
            use egui_multiwin::raw_window_handle_5::{HasRawDisplayHandle, HasRawWindowHandle};
            use egui_multiwin::tracked_window::{ContextHolder, TrackedWindowOptions};
            use egui_multiwin::async_winit::{
                event::Event,
                event_loop::{ControlFlow, EventLoopWindowTarget},
            };
            use egui_multiwin::{arboard, glutin, async_winit};

            use $window;

            /// The return value of the redraw function of trait `TrackedWindow`
            pub struct RedrawResponse {
                /// Should the window exit?
                pub quit: bool,
                /// A list of windows that the window desires to have created.
                pub new_windows: Vec<NewWindowRequest>,
            }

            impl Default for RedrawResponse {
                fn default() -> Self {
                    Self {
                        quit: false,
                        new_windows: Vec::new(),
                    }
                }
            }

            /// A window being tracked by a `MultiWindow`. All tracked windows will be forwarded all events
            /// received on the `MultiWindow`'s event loop.
            #[egui_multiwin::enum_dispatch::enum_dispatch]
            pub trait TrackedWindow {
                /// Returns true if the window is a root window. Root windows will close all other windows when closed. Windows are not root windows by default.
                /// It is completely valid to have more than one root window open at the same time. The program will exit when all root windows are closed.
                fn is_root(&self) -> bool {
                    false
                }

                /// Returns true when the window is allowed to close. Default is windows are always allowed to close. Override to change this behavior.
                fn can_quit(&mut self, _c: &mut $common) -> bool {
                    true
                }

                /// Sets whether or not the window is a root window. Does nothing by default
                fn set_root(&mut self, _root: bool) {}

                /// Runs the redraw for the window. See RedrawResponse for the return value.
                async fn redraw<TS: egui_multiwin::async_winit::ThreadSafety>(
                    &mut self,
                    c: &mut $common,
                    egui: &mut EguiGlow,
                    window: &egui_multiwin::async_winit::window::Window<TS>,
                    clipboard: &mut egui_multiwin::arboard::Clipboard,
                ) -> RedrawResponse;
                /// Allows opengl rendering to be done underneath all of the egui stuff of the window
                /// # Safety
                ///
                /// opengl functions are unsafe. This function would require calling opengl functions.
                unsafe fn opengl_before(
                    &mut self,
                    _c: &mut $common,
                    _gl: &Arc<egui_multiwin::egui_glow_async::painter::Context>,
                ) {
                }
                /// Allows opengl rendering to be done on top of all of the egui stuff of the window
                /// # Safety
                ///
                /// opengl functions are unsafe. This function would require calling opengl functions.
                unsafe fn opengl_after(
                    &mut self,
                    _c: &mut $common,
                    _gl: &Arc<egui_multiwin::egui_glow_async::painter::Context>,
                ) {
                }
            }

            /// Contains the differences between window types
            pub enum WindowInstanceThings<'a> {
                /// A root window
                PlainWindow {
                    /// The contents of the window
                    window: &'a mut $window,
                },
                /// A viewport window
                Viewport {
                    /// A placeholder value
                    b: u8,
                },
            }

            impl<'a> WindowInstanceThings<'a> {
                /// Get an optional mutable reference to the window data
                fn window_data(&mut self) -> Option<&mut $window> {
                    match self {
                        WindowInstanceThings::PlainWindow{window} => {
                            Some(window)
                        }
                        WindowInstanceThings::Viewport{b} => None,
                    }
                }
            }

            /// This structure is for dispatching events
            pub struct TrackedWindowContainerInstance<'a> {
                /// The egui reference
                egui: &'a mut EguiGlow,
                /// The window differences
                window: WindowInstanceThings<'a>,
                /// The viewport set
                viewportset: &'a Arc<Mutex<ViewportIdSet>>,
                /// The viewport id
                viewportid: &'a ViewportId,
                /// The optional callback for the window
                viewport_callback: &'a Option<Arc<DeferredViewportUiCallback>>,
            }

            impl<'a> TrackedWindowContainerInstance<'a> {
                /// Handles one event from the event loop. Returns true if the window needs to be kept alive,
                /// otherwise it will be closed. Window events should be checked to ensure that their ID is one
                /// that the TrackedWindow is interested in.
                async fn handle_event<TS: egui_multiwin::async_winit::ThreadSafety>(
                    &mut self,
                    event: &egui_multiwin::async_winit::event::Event<TS>,
                    el: &EventLoopWindowTarget<TS>,
                    c: &mut $common,
                    root_window_exists: bool,
                    gl_window: &mut egui_multiwin::tracked_window::ContextHolder<
                        PossiblyCurrentContext,
                        TS
                    >,
                    clipboard: &mut egui_multiwin::arboard::Clipboard,
                ) -> TrackedWindowControl {
                    // Child window's requested control flow.
                    let mut control_flow = Some(ControlFlow::Wait); // Unless this changes, we're fine waiting until the next event comes in.
                    let mut viewportset = self.viewportset.lock().unwrap();

                    let response = match event {
                        egui_multiwin::async_winit::event::Event::WindowEvent { event, window_id } => {
                            let mut redraw_thing = None;
                            match event {
                                egui_multiwin::async_winit::event::WindowEvent::Resized(physical_size) => {
                                    gl_window.resize(*physical_size);
                                }
                                egui_multiwin::async_winit::event::WindowEvent::CloseRequested => {
                                    control_flow = None;
                                }
                                egui_multiwin::async_winit::event::WindowEvent::RedrawRequested => {
                                    println!("Window id is {:?}", window_id);
                                    redraw_thing = {
                                        let input = self.egui.egui_winit.take_egui_input(&gl_window.window).await;
                                        let ppp = self.egui.egui_ctx.pixels_per_point();
                                        self.egui.egui_ctx.begin_frame(input);
                                        let mut rr = RedrawResponse::default();
                                        if let Some(cb) = self.viewport_callback {
                                            cb(&self.egui.egui_ctx);
                                        }
                                        else if let Some(window) = self.window.window_data() {
                                            rr = window.redraw(c, self.egui, &gl_window.window, clipboard).await;
                                        }
                                        let full_output = self.egui.egui_ctx.end_frame();

                                        if self.viewport_callback.is_none() {
                                            let mut remove_id = Vec::new();
                                            for id in viewportset.iter() {
                                                if !full_output.viewport_output.contains_key(&id) {
                                                    remove_id.push(id.to_owned());
                                                }
                                            }
                                            for id in remove_id {
                                                viewportset.remove(&id);
                                            }
                                        }
                                        else {
                                            if !viewportset.contains(self.viewportid) {
                                                rr.quit = true;
                                            }
                                        }

                                        for (viewport_id, viewport_output) in &full_output.viewport_output {
                                            if viewport_id != &egui::viewport::ViewportId::ROOT && !viewportset.contains(viewport_id) {
                                                let builder = egui_multiwin::async_winit::window::WindowBuilder::new();
                                                /*
                                                    egui_multiwin::egui_glow::egui_winit::create_winit_window_builder(
                                                        &self.egui.egui_ctx,
                                                        el,
                                                        viewport_output.builder.to_owned(),
                                                    );
                                                */
                                                let options = TrackedWindowOptions {
                                                    shader: None,
                                                    vsync: false,
                                                };
                                                let vp = NewWindowRequest::new_viewport(
                                                    builder,
                                                    options,
                                                    viewport_output.builder.clone(),
                                                    viewport_id.to_owned(),
                                                    self.viewportset.to_owned(),
                                                    viewport_output.viewport_ui_cb.to_owned(),
                                                );
                                                viewportset.insert(viewport_id.to_owned());
                                                rr.new_windows.push(vp);
                                            }
                                        }

                                        let vp_output = full_output
                                            .viewport_output
                                            .get(self.viewportid);
                                        let repaint_after = vp_output.map(|v| v.repaint_delay).unwrap_or(std::time::Duration::from_millis(1000));
                                        println!("Repaint is {:?}", repaint_after);

                                        if rr.quit {
                                            control_flow = None;
                                        } else if repaint_after.is_zero() {
                                            gl_window.window.request_redraw();
                                            control_flow = Some(egui_multiwin::async_winit::event_loop::ControlFlow::Poll);
                                        } else if repaint_after.as_millis() > 0 && repaint_after.as_millis() < 10000 {
                                            control_flow =
                                                Some(egui_multiwin::async_winit::event_loop::ControlFlow::WaitUntil(
                                                    std::time::Instant::now() + repaint_after,
                                                ));
                                        } else {
                                            control_flow = Some(egui_multiwin::async_winit::event_loop::ControlFlow::Wait);
                                        };

                                        {
                                            let color = egui_multiwin::egui::Rgba::from_white_alpha(0.0);
                                            unsafe {
                                                use glow::HasContext as _;
                                                self.egui.painter
                                                    .gl()
                                                    .clear_color(color[0], color[1], color[2], color[3]);
                                                self.egui.painter.gl().clear(glow::COLOR_BUFFER_BIT);
                                            }

                                            // draw things behind egui here
                                            if let Some(window) = self.window.window_data() {
                                                unsafe { window.opengl_before(c, self.egui.painter.gl()) };
                                            }

                                            let prim = self.egui
                                                .egui_ctx
                                                .tessellate(full_output.shapes, self.egui.egui_ctx.pixels_per_point());
                                            self.egui.painter.paint_and_update_textures(
                                                gl_window.window.inner_size().await.into(),
                                                ppp,
                                                &prim[..],
                                                &full_output.textures_delta,
                                            );

                                            // draw things on top of egui here
                                            if let Some(window) = self.window.window_data() {
                                                unsafe { window.opengl_after(c, self.egui.painter.gl()) };
                                            }

                                            gl_window.swap_buffers().unwrap();
                                        }
                                        Some(rr)
                                    };
                                }
                                _ => {}
                            }

                            let resp = self.egui.on_window_event(&gl_window.window, event);
                            if resp.repaint {
                                println!("Requesting repaint because of window event");
                                gl_window.window.request_redraw();
                            }

                            redraw_thing
                        }
                        egui_multiwin::async_winit::event::Event::LoopExiting => {
                            self.egui.destroy();
                            None
                        }

                        _ => None,
                    };

                    if let Some(window) = self.window.window_data() {
                        if !root_window_exists && !window.is_root() {
                            control_flow = None;
                        }
                    }

                    println!("Control flow is {:?}", control_flow);

                    TrackedWindowControl {
                        requested_control_flow: control_flow,
                        windows_to_create: if let Some(a) = response {
                            a.new_windows
                        } else {
                            Vec::new()
                        },
                    }
                }
            }

            /// Defines a window
            pub enum TrackedWindowContainer<TS: egui_multiwin::async_winit::ThreadSafety> {
                /// A root window
                PlainWindow(PlainWindowContainer<TS>),
                /// A viewport window
                Viewport(ViewportWindowContainer<TS>),
            }

            impl<TS: egui_multiwin::async_winit::ThreadSafety> TrackedWindowContainer<TS> {
                /// Get the common data reference
                pub fn get_common(&self) -> &CommonWindowData<TS> {
                    match self {
                        Self::PlainWindow(p) => &p.common,
                        Self::Viewport(v) => &v.common,
                    }
                }
            }

            /// The common data for all window types
            pub struct CommonWindowData<TS: egui_multiwin::async_winit::ThreadSafety> {
                /// The context for the window
                pub gl_window: IndeterminateWindowedContext<TS>,
                /// The egui instance for this window, each window has a separate egui instance.
                pub egui: Option<EguiGlow>,
                /// The viewport set
                viewportset: Arc<Mutex<ViewportIdSet>>,
                /// The viewport id for the window
                viewportid: ViewportId,
                /// The optional shader version for the window
                pub shader: Option<egui_multiwin::egui_glow_async::ShaderVersion>,
                /// The viewport builder
                pub vb: Option<ViewportBuilder>,
                /// The viewport callback
                viewportcb: Option<std::sync::Arc<DeferredViewportUiCallback>>,
            }

            /// The container for a viewport window
            pub struct ViewportWindowContainer<TS: egui_multiwin::async_winit::ThreadSafety> {
                /// The common data
                common: CommonWindowData<TS>,
            }

            /// The main container for a root window.
            pub struct PlainWindowContainer<TS: egui_multiwin::async_winit::ThreadSafety> {
                /// The common data
                common: CommonWindowData<TS>,
                /// The actual window
                pub window: $window,
            }

            impl<TS: egui_multiwin::async_winit::ThreadSafety + 'static> TrackedWindowContainer<TS> {
                /// Get the optional window data contained by the window
                pub fn get_window_data(&self) -> Option<& $window> {
                    match self {
                        Self::PlainWindow(w) => Some(&w.window),
                        Self::Viewport(_) => None,
                    }
                }

                /// Get the optional window data, mutable, contained by the window
                pub fn get_window_data_mut(&mut self) -> Option<&mut $window> {
                    match self {
                        Self::PlainWindow(w) => Some(&mut w.window),
                        Self::Viewport(_) => None,
                    }
                }

                /// Get the common data for the window
                fn common(&self) -> &CommonWindowData<TS> {
                    match self {
                        Self::PlainWindow(w) => &w.common,
                        Self::Viewport(w) => &w.common,
                    }
                }

                /// Get the common data, mutably, for the window
                fn common_mut(&mut self) -> &mut CommonWindowData<TS> {
                    match self {
                        Self::PlainWindow(w) => &mut w.common,
                        Self::Viewport(w) => &mut w.common,
                    }
                }

                /// Get the gl window for the container
                fn gl_window(&self) -> &IndeterminateWindowedContext<TS> {
                    match self {
                        Self::PlainWindow(w) => &w.common.gl_window,
                        Self::Viewport(w) => &w.common.gl_window,
                    }
                }

                /// Get the gl window, mutably for the container
                fn gl_window_mut(&mut self) -> &mut IndeterminateWindowedContext<TS> {
                    match self {
                        Self::PlainWindow(w) => &mut w.common.gl_window,
                        Self::Viewport(w) => &mut w.common.gl_window,
                    }
                }

                /// Create a new window.
                pub async fn create(
                    window: Option<$window>,
                    viewportset: Arc<Mutex<ViewportIdSet>>,
                    viewportid: &ViewportId,
                    viewportcb: Option<std::sync::Arc<DeferredViewportUiCallback>>,
                    window_builder: egui_multiwin::async_winit::window::WindowBuilder,
                    event_loop: &egui_multiwin::async_winit::event_loop::EventLoopWindowTarget<TS>,
                    options: &TrackedWindowOptions,
                    vb: Option<ViewportBuilder>
                ) -> Result<TrackedWindowContainer<TS>, DisplayCreationError> {
                    println!("Create window function");
                    let rdh = event_loop.raw_display_handle();
                    println!("Create window function 1.1");
                    let winitwindow = window_builder.build().await.unwrap();
                    println!("Create window function 1.2");
                    let rwh = winitwindow.raw_window_handle();
                    println!("Create window function 2");
                    #[cfg(target_os = "windows")]
                    let pref = glutin::display::DisplayApiPreference::Wgl(Some(rwh));
                    #[cfg(target_os = "linux")]
                    let pref = egui_multiwin::glutin::display::DisplayApiPreference::Egl;
                    #[cfg(target_os = "macos")]
                    let pref = glutin::display::DisplayApiPreference::Cgl;
                    println!("Create window function 3");
                    let display = unsafe { glutin::display::Display::new(rdh, pref) };
                    println!("Create window function 4");
                    if let Ok(display) = display {
                        println!("Display is ok! {:?}", display);
                        let configt = glutin::config::ConfigTemplateBuilder::default().build();
                        let mut configs: Vec<glutin::config::Config> =
                            unsafe { display.find_configs(configt) }.unwrap().collect();
                        configs.sort_by(|a, b| a.num_samples().cmp(&b.num_samples()));
                        // Try all configurations until one works
                        for config in configs {
                            println!("Examining a config {:?}", config);
                            let sab: SurfaceAttributesBuilder<WindowSurface> =
                                egui_multiwin::glutin::surface::SurfaceAttributesBuilder::default();
                            let sa = sab.build(
                                rwh,
                                std::num::NonZeroU32::new(winitwindow.inner_size().await.width).unwrap(),
                                std::num::NonZeroU32::new(winitwindow.inner_size().await.height).unwrap(),
                            );
                            let ws = unsafe { display.create_window_surface(&config, &sa) };
                            if let Ok(ws) = ws {
                                let attr =
                                    egui_multiwin::glutin::context::ContextAttributesBuilder::new()
                                        .build(Some(rwh));

                                let gl_window =
                                    unsafe { display.create_context(&config, &attr) }.unwrap();

                                let wcommon = CommonWindowData {
                                    viewportid: viewportid.to_owned(),
                                    viewportset: viewportset.clone(),
                                    gl_window: IndeterminateWindowedContext::NotCurrent(
                                        egui_multiwin::tracked_window::ContextHolder::new(
                                            gl_window,
                                            winitwindow,
                                            ws,
                                            display,
                                            *options,
                                        ),
                                    ),
                                    vb,
                                    viewportcb,
                                    egui: None,
                                    shader: options.shader,
                                };
                                if let Some(window) = window {
                                    let w = PlainWindowContainer {
                                        window,
                                        common: wcommon,
                                    };
                                    return Ok(TrackedWindowContainer::PlainWindow(w));
                                }
                                else {
                                    let w = ViewportWindowContainer {
                                        common: wcommon,
                                    };
                                    return Ok(TrackedWindowContainer::Viewport(w));
                                }
                            }
                        }
                    }
                    println!("Unable to create a window");
                    panic!("No window created");
                }

                /// Returns true if the specified event is for this window. A UserEvent (one generated by the EventLoopProxy) is not for any window.
                pub fn is_event_for_window(&self, event: &async_winit::event::Event<TS>) -> bool {
                    // Check if the window ID matches, if not then this window can pass on the event.
                    match (event, self.gl_window()) {
                        (
                            Event::WindowEvent {
                                window_id: id,
                                event: _,
                                ..
                            },
                            IndeterminateWindowedContext::PossiblyCurrent(gl_window),
                        ) => gl_window.window.id() == *id,
                        (
                            Event::WindowEvent {
                                window_id: id,
                                event: _,
                                ..
                            },
                            IndeterminateWindowedContext::NotCurrent(gl_window),
                        ) => gl_window.window.id() == *id,
                        _ => true, // we weren't able to check the window ID, maybe this window is not initialized yet. we should run it.
                    }
                }

                /// Build an instance that can have events dispatched to it
                fn prepare_for_events(&mut self) -> Option<TrackedWindowContainerInstance> {
                    match self {
                        Self::PlainWindow(w) => {
                            if let Some(egui) = &mut w.common.egui {
                                let w2 = WindowInstanceThings::PlainWindow { window: &mut w.window, };
                                Some(TrackedWindowContainerInstance { egui,
                                    window: w2,
                                    viewportset: &w.common.viewportset,
                                    viewportid: &w.common.viewportid,
                                    viewport_callback: &w.common.viewportcb,
                                })
                            }
                            else {
                                None
                            }
                        }
                        Self::Viewport(w) => {
                            if let Some(egui) = &mut w.common.egui {
                                let w2 = WindowInstanceThings::Viewport { b: 42, };
                                Some(TrackedWindowContainerInstance { egui,
                                    window: w2,
                                    viewportset: &w.common.viewportset,
                                    viewportid: &w.common.viewportid,
                                    viewport_callback: &w.common.viewportcb,
                                })
                            }
                            else {
                                None
                            }
                        }
                    }
                }

                /// The outer event handler for a window. Responsible for activating the context, creating the egui context if required, and calling handle_event.
                pub async fn handle_event_outer(
                    &mut self,
                    c: &mut $common,
                    event: &async_winit::event::Event<TS>,
                    el: &EventLoopWindowTarget<TS>,
                    root_window_exists: bool,
                    fontmap: &HashMap<String, egui::FontData>,
                    clipboard: &mut arboard::Clipboard,
                ) -> TrackedWindowControl {
                    // Activate this gl_window so we can use it.
                    // We cannot activate it without full ownership, so temporarily move the gl_window into the current scope.
                    // It *must* be returned at the end.
                    let gl_window =
                        mem::replace(self.gl_window_mut(), IndeterminateWindowedContext::None);
                    let mut gl_window = match gl_window {
                        IndeterminateWindowedContext::PossiblyCurrent(w) => {
                            let _e = w.make_current();
                            w
                        }
                        IndeterminateWindowedContext::NotCurrent(w) => w.make_current().unwrap(),
                        IndeterminateWindowedContext::None => {
                            panic!("there's no window context???")
                        }
                    };

                    // Now that the window is active, create a context if it is missing.
                    match self.common().egui.as_ref() {
                        None => {
                            let gl = Arc::new(unsafe {
                                glow::Context::from_loader_function(|s| {
                                    gl_window.get_proc_address(s)
                                })
                            });

                            unsafe {
                                use glow::HasContext as _;
                                gl.enable(glow::FRAMEBUFFER_SRGB);
                            }
                            let egui = egui_glow_async::EguiGlow::new(el, gl, self.common().shader, None);
                            {
                                let mut fonts = egui::FontDefinitions::default();
                                for (name, font) in fontmap {
                                    fonts.font_data.insert(name.clone(), font.clone());
                                    fonts.families.insert(
                                        egui::FontFamily::Name(name.to_owned().into()),
                                        vec![name.to_owned()],
                                    );
                                }
                                egui.egui_ctx.set_fonts(fonts)
                            }
                            if let Some(vb) = &self.common().vb {
                                egui_multiwin::egui_glow_async::egui_async_winit::apply_viewport_builder_to_window(
                                    &egui.egui_ctx,
                                    gl_window.window(),
                                    vb,
                                );
                            }
                            egui.egui_ctx.set_embed_viewports(false);
                            self.common_mut().egui = Some(egui);
                        }
                        Some(_) => (),
                    };

                    let result = if let Some(mut thing) = self.prepare_for_events() {
                        let result = thing.handle_event(
                            event,
                            el,
                            c,
                            root_window_exists,
                            &mut gl_window,
                            clipboard,
                        ).await;
                        result
                    } else {
                        panic!("Window wasn't fully initialized");
                    };

                    if result.requested_control_flow.is_none() {
                        self.try_quit(c);
                    };

                    match mem::replace(
                        self.gl_window_mut(),
                        IndeterminateWindowedContext::PossiblyCurrent(gl_window),
                    ) {
                        IndeterminateWindowedContext::None => (),
                        _ => {
                            panic!("Window had a GL context while we were borrowing it?");
                        }
                    }
                    result
                }

                fn try_quit(&mut self, c: &mut $common) {
                    match self {
                        Self::PlainWindow(w) => {
                            if w.window.can_quit(c) {
                                if let Some(egui) = &mut w.common.egui {
                                    egui.destroy();
                                }
                            }
                        }
                        Self::Viewport(w) => {
                            w.common.egui = None;
                        }
                    }
                }
            }

            /// Enum of the potential options for a window context
            pub enum IndeterminateWindowedContext<TS: egui_multiwin::async_winit::ThreadSafety> {
                /// The window context is possibly current
                PossiblyCurrent(ContextHolder<PossiblyCurrentContext, TS>),
                /// The window context is not current
                NotCurrent(ContextHolder<NotCurrentContext, TS>),
                /// The window context is empty
                None,
            }

            impl<TS: egui_multiwin::async_winit::ThreadSafety> IndeterminateWindowedContext<TS> {
                /// Get the window handle
                pub fn window(&self) -> &async_winit::window::Window<TS> {
                    match self {
                        IndeterminateWindowedContext::PossiblyCurrent(pc) => pc.window(),
                        IndeterminateWindowedContext::NotCurrent(nc) => nc.window(),
                        IndeterminateWindowedContext::None => panic!("No window"),
                    }
                }
            }

            /// The eventual return struct of the `TrackedWindow` trait update function. Used internally for window management.
            pub struct TrackedWindowControl {
                /// Indicates how the window desires to respond to future events
                pub requested_control_flow: Option<ControlFlow>,
                /// A list of windows to be created
                pub windows_to_create: Vec<NewWindowRequest>,
            }

            #[derive(egui_multiwin::thiserror::Error, Debug)]
            /// Enumerates the kinds of errors that display creation can have.
            pub enum DisplayCreationError {}
        }
    };
}

/// This macro creates a dynamic definition of the multi_window module. It has the same arguments as the [`tracked_window`](macro.tracked_window.html) macro.
#[macro_export]
macro_rules! multi_window {
    ($common:ty, $window:ty) => {
        pub mod multi_window {
            //! This defines the MultiWindow struct. This is the main struct used in the main function of a user application.

            use std::collections::HashMap;
            use std::sync::{Arc, Mutex};

            use egui_multiwin::{
                tracked_window::TrackedWindowOptions,
                async_winit::{
                    self,
                    error::EventLoopError,
                    event_loop::{ControlFlow, EventLoop},
                },
            };

            use egui::viewport::{DeferredViewportUiCallback, ViewportId, ViewportIdSet};
            use egui_multiwin::egui;

            use super::tracked_window::{
                DisplayCreationError, TrackedWindow, TrackedWindowContainer,
            };

            /// The main struct of the crate. Manages multiple `TrackedWindow`s by forwarding events to them.
            /// `T` represents the common data struct for the user program. `U` is the type representing custom events.
            pub struct MultiWindow<TS: egui_multiwin::async_winit::ThreadSafety> {
                /// The event loop for the application
                event_loop: Option<egui_multiwin::async_winit::event_loop::EventLoop<TS>>,
                /// List of windows to be created
                pending_windows: Vec<NewWindowRequest>,
                /// A list of fonts to install on every egui instance
                fonts: HashMap<String, egui_multiwin::egui::FontData>,
                /// The clipboard
                clipboard: egui_multiwin::arboard::Clipboard,
            }

            impl<TS: egui_multiwin::async_winit::ThreadSafety + 'static> Default for MultiWindow<TS> {
                fn default() -> Self {
                    Self::new()
                }
            }

            impl<TS: egui_multiwin::async_winit::ThreadSafety + 'static> MultiWindow<TS> {
                /// Creates a new `MultiWindow`.
                pub fn new() -> Self {
                    MultiWindow {
                        event_loop: Some(egui_multiwin::async_winit::event_loop::EventLoop::new()),
                        pending_windows: vec![],
                        fonts: HashMap::new(),
                        clipboard: egui_multiwin::arboard::Clipboard::new().unwrap(),
                    }
                }

                /// A simpler way to start up a user application. The provided closure should initialize the root window, add any fonts desired, store the proxy if it is needed, and return the common app struct.
                pub async fn start(
                    t: impl FnOnce(
                        &mut Self,
                        &EventLoop<TS>,
                    ) -> $common,
                ) -> Result<(), EventLoopError> {
                    let mut event_loop =
                        egui_multiwin::async_winit::event_loop::EventLoopBuilder::new();
                    let event_loop = event_loop.build();
                    let mut multi_window = Self::new();

                    let ac = t(&mut multi_window, &event_loop);

                    multi_window.run(ac)
                }

                /// Add a font that is applied to every window. Be sure to call this before calling [add](crate::multi_window::MultiWindow::add)
                /// multi_window is an instance of [MultiWindow](crate::multi_window::MultiWindow), DATA is a static `&[u8]` - most like defined with a `include_bytes!()` macro
                /// ```
                /// use egui_multiwin::multi_window::NewWindowRequest;
                /// struct Custom {}
                ///
                /// impl egui_multiwin::multi_window::CommonEventHandler for Custom {
                ///     fn process_event(&mut self, _event: egui_multiwin::multi_window::DefaultCustomEvent)  -> Vec<NewWindowRequest>{
                ///         vec!()
                ///     }
                /// }
                ///
                /// let mut multi_window: egui_multiwin::multi_window::MultiWindow = egui_multiwin::multi_window::MultiWindow::new();
                /// let DATA = include_bytes!("cmunbtl.ttf");
                /// multi_window.add_font("my_font".to_string(), egui_multiwin::egui::FontData::from_static(DATA));
                /// ```
                pub fn add_font(&mut self, name: String, fd: egui_multiwin::egui::FontData) {
                    self.fonts.insert(name, fd);
                }

                /// Adds a new `TrackedWindow` to the `MultiWindow`. If custom fonts are desired, call [add_font](crate::multi_window::MultiWindow::add_font) first.
                pub fn add(
                    &mut self,
                    window: NewWindowRequest,
                ) {
                    self.pending_windows.push(window);
                }

                async fn process_pending_windows(&mut self, 
                    elwt: &async_winit::event_loop::EventLoopWindowTarget<TS>,
                    wclose: &egui_multiwin::future_set::FuturesHashSetFirst<()>,
                ) -> Result<(), DisplayCreationError> {
                    while let Some(window) = self.pending_windows.pop() {
                        let twc = TrackedWindowContainer::create(
                            window.window_state,
                            window.viewportset,
                            &window
                                .viewport_id
                                .unwrap_or(egui::viewport::ViewportId::ROOT),
                            window.viewport_callback,
                            window.builder,
                            elwt,
                            &window.options,
                            window.viewport,
                        ).await?;
                        if let Some(s) = &twc.get_window_data() {
                            if s.is_root() {
                                wclose.get().add_future(async move {
                                    let w = twc.get_common().gl_window.window().close_requested().wait();
                                    println!("Waiting for window to close");
                                    w.await;
                                });
                            }
                            else {
                                todo!("Do something with twc")
                            }
                        }
                        else {
                            todo!("Do something with twc")
                        }
                    }
                    Ok(())
                }

                /// Runs the event loop until all `TrackedWindow`s are closed.
                pub fn run(
                    mut self,
                    mut c: $common,
                ) -> Result<(), EventLoopError> {
                    let event_loop_window_target: async_winit::event_loop::EventLoopWindowTarget<TS> = 
                        self.event_loop
                            .as_ref()
                            .unwrap()
                            .window_target()
                            .clone();
                    println!("Stuff 1");
                    self.event_loop.take().unwrap().block_on(
                        async move {
                            println!("App startup");
                            event_loop_window_target.resumed().await;
                            let mut wclose = egui_multiwin::future_set::FuturesHashSetFirst::new();
                            self.process_pending_windows(&event_loop_window_target, &wclose).await;
                            wclose.clone().await;
                            println!("Exiting?");
                            event_loop_window_target.exit().await
                        })
                }
            }

            /// A struct defining how a new window is to be created.
            pub struct NewWindowRequest {
                /// The actual struct containing window data. The struct must implement the `TrackedWindow` trait.
                pub window_state: Option<$window>,
                /// Specifies how to build the window with a WindowBuilder
                pub builder: egui_multiwin::async_winit::window::WindowBuilder,
                /// Other options for the window.
                pub options: TrackedWindowOptions,
                /// The viewport options
                viewport: Option<egui_multiwin::egui::ViewportBuilder>,
                /// The viewport id
                viewport_id: Option<ViewportId>,
                /// The viewport set, shared among the set of related windows
                viewportset: Arc<Mutex<ViewportIdSet>>,
                /// The viewport callback
                viewport_callback: Option<std::sync::Arc<DeferredViewportUiCallback>>,
            }

            impl NewWindowRequest {
                /// Create a new root window
                pub fn new(
                    window_state: $window,
                    builder: egui_multiwin::async_winit::window::WindowBuilder,
                    options: TrackedWindowOptions,
                ) -> Self {
                    Self {
                        window_state: Some(window_state),
                        builder,
                        options,
                        viewport: None,
                        viewport_id: None,
                        viewportset: Arc::new(Mutex::new(egui::viewport::ViewportIdSet::default())),
                        viewport_callback: None,
                    }
                }

                /// Construct a new viewport window
                pub fn new_viewport(
                    builder: egui_multiwin::async_winit::window::WindowBuilder,
                    options: TrackedWindowOptions,
                    vp_builder: egui_multiwin::egui::ViewportBuilder,
                    vp_id: ViewportId,
                    viewportset: Arc<Mutex<ViewportIdSet>>,
                    vpcb: Option<std::sync::Arc<DeferredViewportUiCallback>>,
                ) -> Self {
                    Self {
                        window_state: None,
                        builder,
                        options,
                        viewport: Some(vp_builder),
                        viewport_id: Some(vp_id),
                        viewport_callback: vpcb,
                        viewportset,
                    }
                }
            }
        }
    };
}
