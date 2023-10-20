//! [![Rust Windows](https://github.com/uglyoldbob/egui-multiwin/actions/workflows/windows_build.yml/badge.svg)](https://github.com/uglyoldbob/egui-multiwin/actions/workflows/windows_build.yml)
//! [![Rust MacOS](https://github.com/uglyoldbob/egui-multiwin/actions/workflows/macos_build.yml/badge.svg)](https://github.com/uglyoldbob/egui-multiwin/actions/workflows/macos_build.yml)
//! [![Rust Linux](https://github.com/uglyoldbob/egui-multiwin/actions/workflows/linux_build.yml/badge.svg)](https://github.com/uglyoldbob/egui-multiwin/actions/workflows/linux_build.yml)
//!
//! This crate is based on the work by vivlim (<https://github.com/vivlim>) and repository located (<https://github.com/vivlim/egui-glow-multiwin>).
//! Vivlim's example repository combines the work at <https://github.com/shivshank/mini_gl_fb/blob/master/examples/multi_window.rs> and egui to form a
//! nice package. This crate makes some modifications to make it useful as an external crate by defining a few traits for users to implement on their
//! custom structs.
//!
//! There are several examples (<https://github.com/uglyoldbob/egui-multiwin/tree/master/examples>) that show how to use this crate in your project.
//! 
//! The majority of the code is created by the pair of macros named [`multi_window`](macro.multi_window.html) and [`tracked_window`](macro.tracked_window.html)
//!
//! The main struct for this crate is defined by the [`multi_window`](macro.multi_window.html) macro.
//!
//! Generally you will create a struct for data that is common to all windows, implement the `CommonEventHandler` trait on it.
//! 
//! It will be useful to run `cargo doc --open` on your application to fully see the documentation for this module. This is because the majority of the code is generated by a pair of macros.
//!
//! ```
//! pub mod egui_multiwin_dynamic {
//!     egui_multiwin::tracked_window!(crate::AppCommon, crate::CustomEvent, crate::MyWindows);
//!     egui_multiwin::multi_window!(crate::AppCommon, crate::CustomEvent, crate::MyWindows);
//! }
//! 
//! #[enum_dispatch(TrackedWindow)]
//! pub enum MyWindows {
//!     Popup(PopupWindow),
//! }
//! 
//! use egui_multiwin::arboard;
//! use egui_multiwin::egui_glow::EguiGlow;
//! use egui_multiwin::enum_dispatch::enum_dispatch;
//! use egui_multiwin_dynamic::multi_window::NewWindowRequest;
//! use egui_multiwin_dynamic::tracked_window::RedrawResponse;
//! use egui_multiwin_dynamic::tracked_window::TrackedWindow;
//! use std::sync::Arc;
//! 
//! pub struct AppCommon {
//!     clicks: u32,
//! }
//! 
//! #[derive(Debug)]
//! pub struct CustomEvent {
//!     window: Option<egui_multiwin::winit::window::WindowId>,
//!     message: u32,
//! }
//! 
//! impl CustomEvent {
//!     fn window_id(&self) -> Option<egui_multiwin::winit::window::WindowId> {
//!         self.window
//!     }
//! }
//! 
//! pub struct PopupWindow {}
//! 
//! impl PopupWindow {
//!     pub fn request() -> NewWindowRequest {
//!         NewWindowRequest {
//!             window_state: MyWindows::Popup(PopupWindow {}),
//!             builder: egui_multiwin::winit::window::WindowBuilder::new()
//!                 .with_resizable(false)
//!                 .with_inner_size(egui_multiwin::winit::dpi::LogicalSize {
//!                     width: 400.0,
//!                     height: 200.0,
//!                 })
//!                 .with_title("A window"),
//!             options: egui_multiwin::tracked_window::TrackedWindowOptions {
//!                 vsync: false,
//!                 shader: None,
//!             },
//!             id: egui_multiwin::multi_window::new_id(),
//!         }
//!     }
//! }
//! 
//! impl TrackedWindow for PopupWindow {
//!     fn is_root(&self) -> bool {
//!         true
//!     }
//! 
//!     fn redraw(
//!         &mut self,
//!         c: &mut AppCommon,
//!         egui: &mut EguiGlow,
//!         _window: &egui_multiwin::winit::window::Window,
//!         _clipboard: &mut arboard::Clipboard,
//!     ) -> RedrawResponse {
//!         let quit = false;
//!         egui_multiwin::egui::CentralPanel::default().show(&egui.egui_ctx, |ui| {
//!             ui.heading(format!("number {}", c.clicks));
//!         });
//!         RedrawResponse {
//!             quit,
//!             new_windows: Vec::new(),
//!         }
//!     }
//! }
//! 
//! impl AppCommon {
//!     fn process_event(&mut self, event: CustomEvent) -> Vec<NewWindowRequest> {
//!         let mut windows_to_create = vec![];
//!         println!("Received an event {:?}", event);
//!         if event.message == 42 {
//!             windows_to_create.push(PopupWindow::request());
//!         }
//!         windows_to_create
//!     }
//! }
//! 
//! fn main() {
//!     egui_multiwin_dynamic::multi_window::MultiWindow::start(|multi_window, event_loop, _proxy| {
//!         let root_window = PopupWindow::request();
//! 
//!         let mut ac = AppCommon { clicks: 0 };
//! 
//!         if let Err(e) = multi_window.add(root_window, &mut ac, event_loop) {
//!             println!("Failed to create main window {:?}", e);
//!         }
//!         ac
//!     });
//! }
//! 
//! ```
//!
//! Check github issues to see if wayland (linux) still has a problem with the clipboard. That issue should give a temporary solution to a segfault that
//! occurs after closing a window in your program.
//!
//! In your main event, create an event loop, create an event loop proxy (if desired). The event loop proxy can be cloned and sent to other threads,
//! allowing custom logic to send events that can create windows and modify the common state of the application as required. Create a multiwindow instance,
//! then create window requests to make initial windows, and add them to the multiwindow with the add function. Create an instance of your common data
//! structure, and finally call run of your multiwindow instance.

#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]

use winit::window::WindowId;

pub use {arboard, egui, egui_glow, enum_dispatch, glutin, raw_window_handle, thiserror, winit};
pub mod multi_window;
pub mod tracked_window;

/// A generic non-event providing struct that users can use when they don't need custom events.
pub struct NoEvent {}

impl NoEvent {
    /// Returns a None for window_id
    pub fn window_id(&self) -> Option<WindowId> {
        None
    }
}
