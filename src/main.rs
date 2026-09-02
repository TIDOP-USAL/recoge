#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#![expect(rustdoc::missing_crate_level_docs)] // it's an example

pub mod config_window;
pub mod configuration;
pub mod docker;
pub mod error;
pub mod image_info_window;
pub mod tasks;
pub mod ui;
pub mod upload_window;
pub mod zot;

fn main() {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let rt = tokio::runtime::Runtime::new().expect("Unable to create background threads");
    let _enter = rt.enter();
    ui::ui_start().unwrap();
}
