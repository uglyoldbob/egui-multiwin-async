#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]

//! Shows a very simple example with minimal code

/// Macro generated code
pub mod egui_multiwin_dynamic {
    egui_multiwin::tracked_window!(crate::AppCommon, crate::MyWindows);
    egui_multiwin::multi_window!(crate::AppCommon, crate::MyWindows);
}

use std::sync::Mutex;

/// The windows for the program
#[enum_dispatch(TrackedWindow)]
pub enum MyWindows {
    /// A popup window
    Popup(PopupWindow),
}

use egui_multiwin::egui_glow_async::EguiGlow;
use egui_multiwin::enum_dispatch::enum_dispatch;
use egui_multiwin_dynamic::multi_window::NewWindowRequest;
use egui_multiwin_dynamic::tracked_window::RedrawResponse;
use egui_multiwin_dynamic::tracked_window::TrackedWindow;
use std::sync::Arc;

/// Data common to all windows
pub struct AppCommon {
    /// Number of times a button has been clicked
    clicks: u32,
}

/// The popup window
pub struct PopupWindow {}

impl PopupWindow {
    /// Create a request to create a window
    pub fn request() -> NewWindowRequest {
        NewWindowRequest::new(
            MyWindows::Popup(PopupWindow {}),
            egui_multiwin::async_winit::window::WindowBuilder::new()
                .with_resizable(false)
                .with_inner_size(egui_multiwin::async_winit::dpi::LogicalSize {
                    width: 400.0,
                    height: 200.0,
                })
                .with_title("A window"),
            egui_multiwin::tracked_window::TrackedWindowOptions {
                vsync: false,
                shader: None,
            },
        )
    }
}

impl TrackedWindow for PopupWindow {
    fn is_root(&self) -> bool {
        true
    }

    async fn redraw<TS: egui_multiwin::async_winit::ThreadSafety>(
        &mut self,
        c: &mut AppCommon,
        egui: &mut EguiGlow,
        _window: &egui_multiwin::async_winit::window::Window<TS>,
        _clipboard: Arc<Mutex<egui_multiwin::arboard::Clipboard>>,
    ) -> RedrawResponse {
        let quit = false;
        egui_multiwin::egui::CentralPanel::default().show(&egui.egui_ctx, |ui| {
            ui.heading(format!("number {}", c.clicks));
        });
        RedrawResponse {
            quit,
            new_windows: Vec::new(),
        }
    }
}

#[tokio::main]
async fn main() {
    let mut multi_window = egui_multiwin_dynamic::multi_window::MultiWindow::new();
    let root_window = PopupWindow::request();
    let ac = AppCommon { clicks: 0 };
    multi_window.add(root_window).await;
    multi_window.run(ac).unwrap();
}
