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
                    clipboard: Arc<Mutex<egui_multiwin::arboard::Clipboard>>,
                ) -> RedrawResponse;
                /// Allows opengl rendering to be done underneath all of the egui stuff of the window
                /// # Safety
                ///
                /// opengl functions are unsafe. This function would require calling opengl functions.
                async unsafe fn opengl_before(
                    &mut self,
                    _c: &mut $common,
                    _gl: &Arc<egui_multiwin::egui_glow_async::painter::Context>,
                ) {
                }
                /// Allows opengl rendering to be done on top of all of the egui stuff of the window
                /// # Safety
                ///
                /// opengl functions are unsafe. This function would require calling opengl functions.
                async unsafe fn opengl_after(
                    &mut self,
                    _c: &mut $common,
                    _gl: &Arc<egui_multiwin::egui_glow_async::painter::Context>,
                ) {
                }
            }

            /// Contains the differences between window types
            pub enum WindowInstanceThings {
                /// A root window
                PlainWindow {
                    /// The contents of the window
                    window: Arc<Mutex<$window>>,
                },
                /// A viewport window
                Viewport {
                    /// A placeholder value
                    b: u8,
                },
            }

            impl WindowInstanceThings {
                /// Get an optional mutable reference to the window data
                fn window_data(&self) -> Option<Arc<Mutex<$window>>> {
                    match self {
                        WindowInstanceThings::PlainWindow{window} => {
                            Some(window.to_owned())
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
                window: WindowInstanceThings,
                /// The viewport set
                viewportset: &'a Arc<Mutex<ViewportIdSet>>,
                /// The viewport id
                viewportid: &'a ViewportId,
                /// The optional callback for the window
                viewport_callback: &'a Option<Arc<DeferredViewportUiCallback>>,
                /// Separate window id
                id: u32,
            }

            impl<'a> TrackedWindowContainerInstance<'a> {
                /// Take input and run egui begin_frame
                async fn begin_frame<TS: egui_multiwin::async_winit::ThreadSafety>(&mut self, window: &egui_multiwin::async_winit::window::Window<TS>) {
                    let mut egui = &mut self.egui;
                    let mut l = egui.egui_winit.lock();
                    let input = l.take_egui_input(window).await;
                    drop(l);
                    egui.egui_ctx.begin_frame(input);
                }

                /// run egui end_frame
                fn end_frame(&mut self) -> egui::FullOutput {
                    let mut egui = &mut self.egui;
                    egui.egui_ctx.end_frame()
                }

                /// Redraw the contents of the window
                async fn redraw<TS: egui_multiwin::async_winit::ThreadSafety>(&mut self,
                    c: &mut $common,
                    window: &egui_multiwin::async_winit::window::Window<TS>,
                    clipboard: std::sync::Arc<Mutex<egui_multiwin::arboard::Clipboard>>,
                ) -> Option<RedrawResponse> {
                    if let Some(cb) = self.viewport_callback {
                        let egui = &self.egui;
                        cb(&egui.egui_ctx);
                        None
                    }
                    else if let Some(window_data) = self.window.window_data() {
                        Some(window_data.lock().unwrap().redraw(c, &mut self.egui, window, clipboard).await)
                    }
                    else {
                        None
                    }
                }

                /// Clear the window by filling the window with transparency
                fn gl_clear(&mut self) {
                    let color = egui_multiwin::egui::Rgba::from_white_alpha(0.0);
                    let mut egui = &mut self.egui;
                    unsafe {
                        use glow::HasContext as _;
                        egui.painter
                            .gl()
                            .clear_color(color[0], color[1], color[2], color[3]);
                        egui.painter.gl().clear(glow::COLOR_BUFFER_BIT);
                    }
                }

                /// Run the gl before callback
                async fn gl_before(&mut self,
                    c: &mut $common,
                ) {
                    let mut egui = &mut self.egui;
                    // draw things behind egui here
                    if let Some(window) = self.window.window_data() {
                        unsafe { window.lock().unwrap().opengl_before(c, egui.painter.gl()).await };
                    }
                }

                async fn draw_main<TS: egui_multiwin::async_winit::ThreadSafety>(&mut self,
                    full_output: egui::FullOutput,
                    window: &egui_multiwin::async_winit::window::Window<TS>,
                ) {
                    let mut egui = &mut self.egui;
                    let ppp = egui.egui_ctx.pixels_per_point();
                    let prim = egui
                        .egui_ctx
                        .tessellate(full_output.shapes, ppp);
                    egui.painter.paint_and_update_textures(
                        window.inner_size().await.into(),
                        ppp,
                        &prim[..],
                        &full_output.textures_delta,
                    );
                }

                /// Run the gl after callback
                async fn gl_after(&mut self,
                    c: &mut $common,
                ) {
                    let mut egui = &mut self.egui;
                    if let Some(window) = self.window.window_data() {
                        unsafe { window.lock().unwrap().opengl_after(c, egui.painter.gl()).await };
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

            impl<TS: egui_multiwin::async_winit::ThreadSafety + Clone + 'static> TrackedWindowContainer<TS> {
                /// Get the common data reference
                pub fn get_common(&self) -> &CommonWindowData<TS> {
                    match self {
                        Self::PlainWindow(p) => &p.common,
                        Self::Viewport(v) => &v.common,
                    }
                }

                /// Perform a redraw of the window
                pub async fn redraw(&mut self,
                    c: &std::sync::Arc<Mutex<$common>>,
                    clipboard: &std::sync::Arc<Mutex<egui_multiwin::arboard::Clipboard>>,
                )
                {
                    let mut gl_window = self.gl_window_option().take().unwrap().make_current();
                    let mut com = c.lock().unwrap();
                    if let Some(mut s) = self.prepare_for_events() {
                        let mut viewportset = s.viewportset.lock().unwrap();
                        let redraw_thing = {
                            let gl_window2 = gl_window.context().unwrap();
                            s.begin_frame(&gl_window2.window).await;
                            let mut rr = RedrawResponse::default();
                            if let Some(rr2) = s.redraw(&mut com, &gl_window2.window, clipboard.to_owned()).await {
                                rr = rr2;
                            }
                            let full_output = s.end_frame();

                            if s.viewport_callback.is_none() {
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
                                if !viewportset.contains(s.viewportid) {
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
                                        s.viewportset.to_owned(),
                                        viewport_output.viewport_ui_cb.to_owned(),
                                    );
                                    viewportset.insert(viewport_id.to_owned());
                                    rr.new_windows.push(vp);
                                }
                            }

                            let vp_output = full_output
                                .viewport_output
                                .get(s.viewportid);
                            let repaint_after = vp_output.map(|v| v.repaint_delay).unwrap_or(std::time::Duration::from_millis(1000));

                            {
                                s.gl_clear();
                                s.gl_before(&mut com).await;
                                s.draw_main(full_output, &gl_window2.window).await;
                                s.gl_after(&mut com).await;
                                let e = gl_window2.swap_buffers();
                                drop(gl_window2);
                            }
                            Some(rr)
                        };
                    }
                    self.gl_window_option().replace(gl_window.make_not_current());
                }
            }

            /// The common data for all window types
            pub struct CommonWindowData<TS: egui_multiwin::async_winit::ThreadSafety> {
                /// The context for the window
                pub gl_window: Option<IndeterminateWindowedContext<TS>>,
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
                /// A seperate id from the window id
                id: u32,
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
                pub window: Arc<Mutex<$window>>,
            }

            impl<TS: egui_multiwin::async_winit::ThreadSafety + Clone + 'static> TrackedWindowContainer<TS> {
                /// Apply viewport data if required to do so
                pub async fn check_viewport_builder(&mut self) {
                    let common = self.common();
                    if let Some(vb) = &common.vb {
                        let gl_window = self.gl_window();
                        egui_multiwin::egui_glow_async::egui_async_winit::apply_viewport_builder_to_window(
                            &common.egui.as_ref().unwrap().egui_ctx,
                            &gl_window.unwrap().window(),
                            vb,
                        ).await;
                    }
                }

                /// Get the optional window data contained by the window
                pub fn get_window_data(&self) -> Option<Arc<Mutex<$window>>> {
                    match self {
                        Self::PlainWindow(w) => Some(w.window.clone()),
                        Self::Viewport(_) => None,
                    }
                }

                /// Get the common data for the window
                pub fn common(&self) -> &CommonWindowData<TS> {
                    match self {
                        Self::PlainWindow(w) => &w.common,
                        Self::Viewport(w) => &w.common,
                    }
                }

                /// Get the common data, mutably, for the window
                pub fn common_mut(&mut self) -> &mut CommonWindowData<TS> {
                    match self {
                        Self::PlainWindow(w) => &mut w.common,
                        Self::Viewport(w) => &mut w.common,
                    }
                }

                /// Get the gl window for the container
                pub fn gl_window(&self) -> Option<&IndeterminateWindowedContext<TS>> {
                    match self {
                        Self::PlainWindow(w) => w.common.gl_window.as_ref(),
                        Self::Viewport(w) => w.common.gl_window.as_ref(),
                    }
                }

                /// Get the gl_window option
                pub fn gl_window_option(&mut self) -> &mut Option<IndeterminateWindowedContext<TS>> {
                    match self {
                        Self::PlainWindow(w) => &mut w.common.gl_window,
                        Self::Viewport(w) => &mut w.common.gl_window,
                    }
                }

                /// Get the gl window, mutably for the container
                pub fn gl_window_mut(&mut self) -> Option<&mut IndeterminateWindowedContext<TS>> {
                    match self {
                        Self::PlainWindow(w) => w.common.gl_window.as_mut(),
                        Self::Viewport(w) => w.common.gl_window.as_mut(),
                    }
                }

                /// Create a new window.
                pub async fn create(
                    window: Option<Arc<Mutex<$window>>>,
                    viewportset: Arc<Mutex<ViewportIdSet>>,
                    viewportid: &ViewportId,
                    viewportcb: Option<std::sync::Arc<DeferredViewportUiCallback>>,
                    window_builder: egui_multiwin::async_winit::window::WindowBuilder,
                    event_loop: &egui_multiwin::async_winit::event_loop::EventLoopWindowTarget<TS>,
                    options: &TrackedWindowOptions,
                    vb: Option<ViewportBuilder>
                ) -> Result<TrackedWindowContainer<TS>, DisplayCreationError> {
                    let rdh = event_loop.raw_display_handle();
                    let winitwindow = window_builder.build().await.unwrap();
                    let rwh = winitwindow.raw_window_handle();
                    #[cfg(target_os = "windows")]
                    let pref = glutin::display::DisplayApiPreference::Wgl(Some(rwh));
                    #[cfg(target_os = "linux")]
                    let pref = egui_multiwin::glutin::display::DisplayApiPreference::Egl;
                    #[cfg(target_os = "macos")]
                    let pref = glutin::display::DisplayApiPreference::Cgl;
                    let display = unsafe { glutin::display::Display::new(rdh, pref) };
                    if let Ok(display) = display {
                        let configt = glutin::config::ConfigTemplateBuilder::default().build();
                        let mut configs: Vec<glutin::config::Config> =
                            unsafe { display.find_configs(configt) }.unwrap().collect();
                        configs.sort_by(|a, b| a.num_samples().cmp(&b.num_samples()));
                        // Try all configurations until one works
                        for config in configs {
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
                                    gl_window: Some(IndeterminateWindowedContext::NotCurrent(
                                        egui_multiwin::tracked_window::ContextHolder::new(
                                            gl_window,
                                            winitwindow,
                                            ws,
                                            display,
                                            *options,
                                        )
                                    )),
                                    vb,
                                    viewportcb,
                                    egui: None,
                                    shader: options.shader,
                                    id: egui_multiwin::rand::Rng::gen(&mut egui_multiwin::rand::thread_rng()),
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
                    panic!("No window created");
                }

                /// Build an instance that can have events dispatched to it
                fn prepare_for_events(&mut self) -> Option<TrackedWindowContainerInstance> {
                    match self {
                        Self::PlainWindow(w) => {
                            if let Some(egui) = &mut w.common.egui {
                                let w2 = WindowInstanceThings::PlainWindow { window: w.window.clone(), };
                                Some(TrackedWindowContainerInstance { egui,
                                    window: w2,
                                    viewportset: &w.common.viewportset,
                                    viewportid: &w.common.viewportid,
                                    viewport_callback: &w.common.viewportcb,
                                    id: w.common.id,
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
                                    id: w.common.id,
                                })
                            }
                            else {
                                None
                            }
                        }
                    }
                }

                fn try_quit(&mut self, c: &mut $common) {
                    match self {
                        Self::PlainWindow(w) => {
                            if w.window.lock().unwrap().can_quit(c) {
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

            impl<TS: egui_multiwin::async_winit::ThreadSafety + Clone> IndeterminateWindowedContext<TS> {
                /// Get the window handle
                pub fn window(&self) -> Arc<async_winit::window::Window<TS>> {
                    match self {
                        IndeterminateWindowedContext::PossiblyCurrent(pc) => pc.window(),
                        IndeterminateWindowedContext::NotCurrent(nc) => nc.window(),
                        IndeterminateWindowedContext::None => panic!("No window"),
                    }
                }

                /// Get the proc address from glutin
                pub fn get_proc_address(&self, s: &str) -> *const std::ffi::c_void {
                    match self {
                        IndeterminateWindowedContext::PossiblyCurrent(pc) => pc.get_proc_address(s),
                        IndeterminateWindowedContext::NotCurrent(nc) => nc.get_proc_address(s),
                        IndeterminateWindowedContext::None => panic!("No window"),
                    }
                }

                /// Attempt to make the current context current
                pub fn make_current(self) -> Self {
                    match self {
                        IndeterminateWindowedContext::PossiblyCurrent(mut pc) => {
                            pc.make_current();
                            IndeterminateWindowedContext::PossiblyCurrent(pc)
                        }
                        IndeterminateWindowedContext::NotCurrent(mut nc) => {
                            let c = nc.make_current().unwrap();
                            IndeterminateWindowedContext::PossiblyCurrent(c)
                        }
                        IndeterminateWindowedContext::None => {
                            IndeterminateWindowedContext::None
                        }
                    }
                }

                /// Make the current context not current
                pub fn make_not_current(self) -> Self {
                    match self {
                        IndeterminateWindowedContext::PossiblyCurrent(mut pc) => {
                            let newc = pc.make_not_current().unwrap();
                            IndeterminateWindowedContext::NotCurrent(newc)
                        }
                        IndeterminateWindowedContext::NotCurrent(nc) => {
                            IndeterminateWindowedContext::NotCurrent(nc)
                        }
                        IndeterminateWindowedContext::None => {
                            IndeterminateWindowedContext::None
                        }
                    }
                }

                /// Get a possibly current context
                pub fn context(&self) -> Option<&ContextHolder<PossiblyCurrentContext, TS>> {
                    match self {
                        IndeterminateWindowedContext::PossiblyCurrent(pc) => Some(pc),
                        IndeterminateWindowedContext::NotCurrent(nc) => None,
                        IndeterminateWindowedContext::None => None,
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

            use egui_multiwin::egui_glow_async::{self, glow};
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
                CommonWindowData, DisplayCreationError, IndeterminateWindowedContext,
                TrackedWindow, TrackedWindowContainer,
            };

            /// The main struct of the crate. Manages multiple `TrackedWindow`s by forwarding events to them.
            /// `T` represents the common data struct for the user program. `U` is the type representing custom events.
            pub struct MultiWindow {
                /// The event loop for the application
                event_loop: Option<egui_multiwin::async_winit::event_loop::EventLoop<async_winit::ThreadSafe>>,
                /// List of windows to be created
                pending_windows: Vec<NewWindowRequest>,
                /// A list of fonts to install on every egui instance
                fonts: HashMap<String, egui_multiwin::egui::FontData>,
                /// The clipboard
                clipboard: Arc<Mutex<egui_multiwin::arboard::Clipboard>>,
            }

            impl Default for MultiWindow {
                fn default() -> Self {
                    Self::new()
                }
            }

            impl MultiWindow {
                /// Creates a new `MultiWindow`.
                pub fn new() -> Self {
                    MultiWindow {
                        event_loop: Some(egui_multiwin::async_winit::event_loop::EventLoop::new()),
                        pending_windows: vec![],
                        fonts: HashMap::new(),
                        clipboard: Arc::new(Mutex::new(egui_multiwin::arboard::Clipboard::new().unwrap())),
                    }
                }

                /// A simpler way to start up a user application. The provided closure should initialize the root window, add any fonts desired, store the proxy if it is needed, and return the common app struct.
                pub async fn start(
                    t: impl FnOnce(
                        &mut Self,
                        &EventLoop<async_winit::ThreadSafe>,
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

                async fn init_egui(
                    fontmap: &HashMap<String, egui_multiwin::egui::FontData>,
                    twc: &mut TrackedWindowContainer<async_winit::ThreadSafe>,
                    elwt: &async_winit::event_loop::EventLoopWindowTarget<async_winit::ThreadSafe>,
                    window: &Arc<egui_multiwin::async_winit::window::Window<async_winit::ThreadSafe>>,
                ) {
                    let gl_window = twc.gl_window_option().take().unwrap().make_current();
                    let gl = Arc::new(unsafe {
                        glow::Context::from_loader_function(|s| {
                            gl_window.get_proc_address(s)
                        })
                    });

                    unsafe {
                        use glow::HasContext as _;
                        gl.enable(glow::FRAMEBUFFER_SRGB);
                    }
                    let mut egui = {
                        let common = twc.common_mut();
                        let egui = egui_glow_async::EguiGlow::new(elwt, gl, common.shader, None);
                        let mut fonts = egui::FontDefinitions::default();
                        for (name, font) in fontmap.iter() {
                            fonts.font_data.insert(name.clone(), font.clone());
                            fonts.families.insert(
                                egui::FontFamily::Name(name.to_owned().into()),
                                vec![name.to_owned()],
                            );
                        }
                        egui.egui_ctx.set_fonts(fonts);
                        egui
                    };
                    twc.gl_window_option().replace(gl_window);
                    egui.egui_ctx.set_embed_viewports(false);
                    egui_multiwin::egui_glow_async::egui_async_winit::State::register_event_handlers(&egui.egui_winit, window);
                    twc.common_mut().egui = Some(egui);
                    twc.check_viewport_builder().await;
                }

                async fn process_pending_windows(&mut self,
                    c: Arc<Mutex<$common>>,
                    elwt: &async_winit::event_loop::EventLoopWindowTarget<async_winit::ThreadSafe>,
                    events: &mut egui_multiwin::Events,
                ) -> Result<(), DisplayCreationError> {
                    while let Some(window) = self.pending_windows.pop() {
                        let twc = TrackedWindowContainer::create(
                            window.window_state.map(|a| Arc::new(Mutex::new(a))),
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
                        let twc = Arc::new(Mutex::new(twc));
                        let twc2 = twc.clone();
                        let clipboard = self.clipboard.to_owned();
                        let fonts = self.fonts.clone();
                        let c2 = c.to_owned();
                        let elwt2 = elwt.clone();
                        let window_process = async move {
                            let id : usize = egui_multiwin::rand::Rng::gen(&mut egui_multiwin::rand::thread_rng());
                            let glw = {
                                let twc3 = twc2.lock().unwrap();
                                twc3.get_common().gl_window.as_ref().unwrap().window()
                            };
                            let glw3 = glw.clone();
                            let close = glw3.close_requested().wait();
                            let (t, mut r) = egui_multiwin::async_channel::bounded(10);
                            let (t2, mut r2) = egui_multiwin::async_channel::bounded(10);
                            let ta = t.clone();
                            glw3.close_requested().wait_direct_async(move |a| {
                                let t = ta.clone();
                                async move {
                                    t.send(true).await.unwrap();
                                    println!("Close window {}", id);
                                    false
                                }
                            });
                            // This runs the drawing on the proper thread, preventing async-winit from trying to run two draw events at the same time
                            glw3.redraw_requested().wait_direct_async(move |c| {
                                let t = t.clone();
                                let r2 = r2.clone();
                                async move {
                                    t.send(true).await.unwrap();
                                    let a: bool = r2.recv().await.unwrap();
                                    true
                                }
                            });
                            let twc4 = twc2.clone();
                            let draw = async move {
                                let mut glw2 = glw.clone();
                                {
                                    let mut twc5 = twc4.lock().unwrap();
                                    Self::init_egui(&fonts, &mut *twc5, &elwt2, &mut glw2).await;
                                };
                                loop {
                                    let a = r.recv().await.unwrap();
                                    let mut t = twc4.lock().unwrap();
                                    t.redraw(&c2, &clipboard).await;
                                    drop(t);
                                    t2.send(true).await.unwrap();
                                }
                            };
                            use egui_multiwin::futures_lite::FutureExt;
                            close.or(draw).await;
                        };
                        if let Some(s) = twc.clone().lock().unwrap().get_window_data() {
                            if s.lock().unwrap().is_root() {
                                events.window_close.get().add_future(window_process);
                            }
                            else {
                                events.non_root_windows.get().add_future(window_process);
                            }
                        }
                        else {
                            events.non_root_windows.get().add_future(window_process);
                        }
                    }
                    Ok(())
                }

                /// Runs the event loop until all `TrackedWindow`s are closed.
                pub fn run(
                    mut self,
                    c: $common,
                ) -> Result<(), EventLoopError> {
                    let event_loop_window_target: async_winit::event_loop::EventLoopWindowTarget<async_winit::ThreadSafe> =
                        self.event_loop
                            .as_ref()
                            .unwrap()
                            .window_target()
                            .clone();
                    let c = Arc::new(Mutex::new(c));
                    self.event_loop.take().unwrap().block_on(
                        async move {
                            event_loop_window_target.resumed().await;
                            let e = event_loop_window_target.exit();
                            let mut events = egui_multiwin::Events::new();
                            self.process_pending_windows(c, &event_loop_window_target, &mut events).await.unwrap();
                            let mut wc = events.window_close.clone();
                            let mut oc = events.non_root_windows.clone();
                            egui_multiwin::deadlock().await;
                            loop {
                                tokio::select! {
                                    _ = &mut wc => { println!("All the root windows closed"); break; }
                                    _ = egui_multiwin::futures_lite::stream::StreamExt::next(&mut oc) => { }
                                }
                            }
                            println!("Waiting for program to exit");
                            drop(oc);
                            event_loop_window_target.set_exit();
                            let w = e.await;
                            println!("Program exiting now");
                            w
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
