#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]

//! An example showing how to use vsync

use egui_multiwin_dynamic::multi_window::MultiWindow;

/// Macro generated code
pub mod egui_multiwin_dynamic {
    egui_multiwin::tracked_window!(crate::AppCommon, crate::windows::MyWindows);
    egui_multiwin::multi_window!(crate::AppCommon, crate::windows::MyWindows);
}

mod windows;

/// The custom font to use for the example
const COMPUTER_MODERN_FONT: &[u8] = include_bytes!("./cmunbtl.ttf");

use windows::{
    popup_window,
    root::{self},
};

/// Data common to all windows in the program
pub struct AppCommon {
    /// The number of times a button has been clicked
    clicks: u32,
}

#[tokio::main]
async fn main() {
    let mut multi_window: MultiWindow = MultiWindow::new();
    multi_window.add_font(
        "computermodern".to_string(),
        egui_multiwin::egui::FontData::from_static(COMPUTER_MODERN_FONT),
    );
    let root_window = root::RootWindow::request();
    let root_window2 = popup_window::PopupWindow::request("initial popup".to_string());

    let ac = AppCommon { clicks: 0 };

    let _e = multi_window.add(root_window).await;
    let _e = multi_window.add(root_window2).await;
    multi_window.run(ac).unwrap();
}
