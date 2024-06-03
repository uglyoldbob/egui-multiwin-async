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
//! See the examples in the repository for example applications that can be used to start your application.
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

pub use {
    arboard, async_channel, async_winit, egui, egui_glow_async, enum_dispatch, futures_lite,
    glutin, rand, raw_window_handle_5, raw_window_handle_6, thiserror,
};
pub mod multi_window;
pub mod tracked_window;

pub mod future_set;

/// Represents the events that we care about
pub struct Events {
    /// For root windows
    pub window_close: future_set::FuturesHashSetAll<()>,
    /// For non-root windows
    pub non_root_windows: future_set::FuturesHashSet<()>,
}

impl Events {
    /// Construct a new event handler
    pub fn new() -> Self {
        Self {
            window_close: future_set::FuturesHashSetAll::new(),
            non_root_windows: future_set::FuturesHashSet::new(),
        }
    }
}

lazy_static::lazy_static! {
    /// Mutex used for drawing
    pub static ref DRAW_MUTEX: tokio::sync::Mutex<bool> = tokio::sync::Mutex::new(false);
}

/// Peridocially check for mutex deadlocks
pub async fn deadlock() {
    use rust_mutex::parking_lot::deadlock;
    use std::time::Duration;
    tokio::time::sleep(Duration::from_secs(10)).await;
    let deadlocks = deadlock::check_deadlock();
    if deadlocks.is_empty() {
        return;
    }

    println!("{} deadlocks detected", deadlocks.len());
    for (i, threads) in deadlocks.iter().enumerate() {
        println!("Deadlock #{}", i);
        for t in threads {
            println!("Thread Id {:#?}", t.thread_id());
            println!("{:#?}", t.backtrace());
        }
    }
    println!("End of deadlocks detected");
}
