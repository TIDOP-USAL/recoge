#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#![expect(rustdoc::missing_crate_level_docs)] // it's an example

pub mod configuration;
pub mod container;
pub mod error;
pub mod tasks;
pub mod ui;
pub mod utils;
pub mod zot;

use env_logger::{Builder, Env};
use log::info;

fn main() {
    // Setup logger with default level info. Override with `RUST_LOG=debug`
    let mut log_builder = Builder::from_env(Env::default().default_filter_or("info"));
    if std::env::var("RUST_LOG").is_err() || std::env::var("LOG_QUIET").is_ok() {
        let mute_list = [
            "zbus",
            "eframe",
            "calloop",
            "egui_wgpu",
            "wgpu_core",
            "wgpu_hal",
            "winit",
            "egui_winit",
            "naga",
            "bollard",
            "h2",
            "hyper_util",
            "reqwest",
            "rustls_platform_verifier",
        ];
        for m in mute_list {
            log_builder.filter(Some(m), log::LevelFilter::Warn);
        }
    }
    log_builder.filter(Some("tracing"), log::LevelFilter::Warn);
    log_builder.init();

    info!("recoge v{}", env!("CARGO_PKG_VERSION"));
    let rt = tokio::runtime::Runtime::new().expect("Unable to create background threads");
    let _enter = rt.enter();
    ui::ui_start().unwrap();
}
