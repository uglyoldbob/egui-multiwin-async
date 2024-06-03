//! The code for all of the actual windows of the program

use egui_multiwin::enum_dispatch::enum_dispatch;

use crate::egui_multiwin_dynamic::tracked_window::{RedrawResponse, TrackedWindow};
use egui_multiwin::egui_glow_async::EguiGlow;
use std::sync::{Arc, Mutex};

pub mod popup_window;
pub mod root;

/// The various windows for the application
#[enum_dispatch(TrackedWindow)]
pub enum MyWindows {
    /// The root window
    Root(root::RootWindow),
    /// The popup window
    Popup(popup_window::PopupWindow),
}
