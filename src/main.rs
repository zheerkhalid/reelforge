#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod bidi;
mod engine;
mod gpu;
mod profiles;
mod selftest;
mod settings;
mod text;
use selftest::selftest;

use eframe::egui;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut screenshot: Option<PathBuf> = None;
    let mut files: Vec<PathBuf> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--selftest" && i + 1 < args.len() {
            selftest(PathBuf::from(&args[i + 1]));
            return Ok(());
        }
        if args[i] == "--screenshot" && i + 1 < args.len() {
            screenshot = Some(PathBuf::from(&args[i + 1]));
            i += 2;
            continue;
        }
        files.push(PathBuf::from(&args[i]));
        i += 1;
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("ReelForge – Video & Audio Converter")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([980.0, 620.0])
        .with_drag_and_drop(true)
        .with_icon(window_icon());
    if screenshot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(windows)]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if screenshot.is_some() {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            b.with_activate_ignoring_other_apps(false);
        }));
    }
    run(options, screenshot, files)
}

fn run(options: eframe::NativeOptions, screenshot: Option<PathBuf>, files: Vec<PathBuf>) -> eframe::Result<()> {
    eframe::run_native(
        "ReelForge",
        options,
        Box::new(move |cc| {
            let mut a = app::App::new(cc, screenshot);
            if !files.is_empty() {
                a.add_files(files);
            }
            Ok(Box::new(a))
        }),
    )
}

fn window_icon() -> egui::IconData {
    let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/icon-256.png"));
    match image::load_from_memory(bytes) {
        Ok(img) => {
            let rgba = img.into_rgba8();
            let (width, height) = rgba.dimensions();
            egui::IconData { rgba: rgba.into_raw(), width, height }
        }
        Err(_) => egui::IconData::default(),
    }
}
