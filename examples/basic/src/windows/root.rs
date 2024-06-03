//! Code for the root window of the project.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::egui_multiwin_dynamic::{
    multi_window::NewWindowRequest,
    tracked_window::{RedrawResponse, TrackedWindow},
};
use egui_multiwin::egui::FontId;
use egui_multiwin::egui_glow_async::EguiGlow;

use crate::AppCommon;

use super::popup_window::PopupWindow;

/// The root window
pub struct RootWindow {
    /// The nnumber of times a button has been clicked
    pub button_press_count: u32,
    /// The number of popus created
    pub num_popups_created: u32,
    /// True when the groot viewport should be visible
    summon_groot: bool,
    /// The last time an update was performed
    prev_time: std::time::Instant,
    /// The calculated frames per second of the application
    fps: Option<f32>,
}

impl RootWindow {
    /// Request a new window
    pub fn request() -> NewWindowRequest {
        NewWindowRequest::new(
            super::MyWindows::Root(RootWindow {
                button_press_count: 0,
                num_popups_created: 0,
                summon_groot: false,
                prev_time: std::time::Instant::now(),
                fps: None,
            }),
            egui_multiwin::async_winit::window::WindowBuilder::new()
                .with_resizable(true)
                .with_inner_size(egui_multiwin::async_winit::dpi::LogicalSize {
                    width: 800.0,
                    height: 600.0,
                })
                .with_title("egui-multiwin root window"),
            egui_multiwin::tracked_window::TrackedWindowOptions {
                vsync: false,
                shader: None,
            },
        )
    }
}

impl TrackedWindow for RootWindow {
    fn is_root(&self) -> bool {
        true
    }

    fn set_root(&mut self, _root: bool) {}

    async fn redraw<TS: egui_multiwin::async_winit::ThreadSafety>(
        &mut self,
        c: &mut AppCommon,
        egui: &mut EguiGlow,
        _window: &egui_multiwin::async_winit::window::Window<TS>,
        _clipboard: Arc<Mutex<egui_multiwin::arboard::Clipboard>>,
    ) -> RedrawResponse {
        let mut quit = false;

        let egui_ctx = &egui.egui_ctx;
        egui_ctx.request_repaint_after(Duration::from_millis(95));

        let cur_time = std::time::Instant::now();
        let delta = cur_time.duration_since(self.prev_time);
        self.prev_time = cur_time;

        let new_fps = 1_000_000_000.0 / delta.as_nanos() as f32;
        if let Some(fps) = &mut self.fps {
            *fps = (*fps * 0.95) + (0.05 * new_fps);
        } else {
            self.fps = Some(new_fps);
        }

        let mut windows_to_create = vec![];

        egui_multiwin::egui::SidePanel::left("my_side_panel").show_async(egui_ctx, |ui| {
            ui.heading("Hello World!");
            if ui.button("New popup").clicked() {
                windows_to_create.push(PopupWindow::request(format!(
                    "popup window #{}",
                    self.num_popups_created
                )));
                self.num_popups_created += 1;
            }
            if ui.button("New transparent window").clicked() {
                windows_to_create.push(crate::windows::transparent_window::PopupWindow::request(
                    "Transparent".to_string(),
                ));
            }
            if ui.button("Quit").clicked() {
                quit = true;
            }
        }).await;
        egui_multiwin::egui::CentralPanel::default().show_async(egui_ctx, |ui| async move {
            let mut ui = ui.lock();
            ui.label(format!("The fps is {}", self.fps.unwrap()));
            ui.heading(format!("number {}", c.clicks));
            let t = egui_multiwin::egui::widget_text::RichText::new("Example custom font text");
            let t = t.font(FontId {
                size: 12.0,
                family: egui_multiwin::egui::FontFamily::Name("computermodern".into()),
            });
            ui.label(t);
            ui.checkbox(&mut self.summon_groot, "summon groot");
            if self.summon_groot {
                egui_ctx.show_viewport_deferred(
                    egui_multiwin::egui::viewport::ViewportId::from_hash_of("Testing"),
                    egui_multiwin::egui::viewport::ViewportBuilder {
                        title: Some("Test title".to_string()),
                        ..Default::default()
                    },
                    |a, _b| {
                        egui_multiwin::egui::CentralPanel::default().show(a, |ui| {
                            ui.label("I am groot");
                        });
                    },
                );
            }
        }).await;
        RedrawResponse {
            quit,
            new_windows: windows_to_create,
        }
    }
}
