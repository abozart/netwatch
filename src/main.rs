#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod app;
mod defaults;
mod features;
mod hotkeys;
mod icon;
mod settings;
mod state;
mod tray;
#[cfg(windows)]
mod actions;
#[cfg(windows)]
mod click_through;
#[cfg(windows)]
mod elevate;
#[cfg(windows)]
mod etw;
#[cfg(windows)]
mod services;

use eframe::egui;
use parking_lot::RwLock;
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    let loaded_settings = settings::Settings::load();

    let state = Arc::new(RwLock::new(state::AppState::new()));
    loaded_settings.apply_to(&mut state.write());

    #[cfg(windows)]
    {
        let s = state.clone();
        std::thread::spawn(move || {
            if let Err(e) = etw::run(s.clone()) {
                s.write().etw_error = Some(format!("{e:#}"));
            }
        });
        services::spawn_refresher(state.clone());
    }

    let initial_height = if loaded_settings.show_processes {
        defaults::WINDOW_HEIGHT_WITH_PROCESSES
    } else {
        defaults::WINDOW_HEIGHT_NO_PROCESSES
    };
    let icon_rgba = icon::ring_rgba(defaults::WINDOW_ICON_SIZE);
    let window_icon = egui::IconData {
        rgba: icon_rgba,
        width: defaults::WINDOW_ICON_SIZE,
        height: defaults::WINDOW_ICON_SIZE,
    };
    let viewport = egui::ViewportBuilder::default()
        .with_title("netwatch")
        .with_icon(window_icon)
        .with_inner_size([defaults::WINDOW_WIDTH, initial_height])
        .with_min_inner_size([defaults::WINDOW_MIN_WIDTH, defaults::WINDOW_MIN_HEIGHT])
        .with_transparent(true)
        .with_decorations(loaded_settings.show_title_bar)
        // Mirror of FeatureToggle::ShowTitleBar's side effect: chromed window
        // is resizable, chromeless is fixed-size. See features.rs.
        .with_resizable(loaded_settings.show_title_bar)
        .with_window_level(if loaded_settings.always_on_top {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        });

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "netwatch",
        options,
        Box::new(move |cc| Ok(Box::new(app::NetWatchApp::new(cc, state)))),
    )
}
