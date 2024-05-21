#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]

//! This is a basic example

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

/// The common data that all windows have access to
pub struct AppCommon {
    /// Number of times a button has been clicked
    clicks: u32,
}

#[tokio::main]
async fn main() {
    println!("Startup 1");
    let mut multi_window: MultiWindow<egui_multiwin::async_winit::DefaultThreadSafety> =
        MultiWindow::new();
    println!("Startup 2");
    multi_window.add_font(
        "computermodern".to_string(),
        egui_multiwin::egui::FontData::from_static(COMPUTER_MODERN_FONT),
    );
    println!("Startup 3");
    let root_window = root::RootWindow::request();
    println!("Startup 4");
    let root_window2 = popup_window::PopupWindow::request("initial popup".to_string());
    println!("Startup 5");

    let ac = AppCommon { clicks: 0 };
    println!("Startup 6");

    multi_window.add(root_window);
    println!("Startup 7");
    multi_window.add(root_window2);
    println!("Startup 8");
    multi_window.run(ac).unwrap();
    println!("Startup 9");
}
