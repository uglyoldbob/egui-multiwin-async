use enum_dispatch::enum_dispatch;

use crate::egui_multiwin::tracked_window::{RedrawResponse, TrackedWindow};
use crate::egui_multiwin::egui_glow::EguiGlow;
use std::sync::Arc;

pub mod popup_window;
pub mod root;

#[enum_dispatch(TrackedWindow)]
pub enum MyWindows {
    Root(root::RootWindow),
    Popup(popup_window::PopupWindow),
}