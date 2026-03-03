// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2025–2026 Loqa Contributors
//! Loqa — shared Tauri application builder.
//!
//! This lib is the core bootstrap shared by both the desktop `main.rs`
//! and the mobile entry point. It registers plugins, wires up
//! commands from [`commands`], and (on desktop) delegates tray
//! setup to [`tray`].

use tauri::Manager;
#[cfg(desktop)]
use std::sync::Mutex;

pub mod commands;
pub mod settings;
#[cfg(desktop)]
pub mod tray;

pub fn build_app() -> tauri::Builder<tauri::Wry> {
    let builder = tauri::Builder::default();

    // ── Managed state (desktop only — sysinfo for game detection) ──
    #[cfg(desktop)]
    let builder = builder.manage(Mutex::new(sysinfo::System::new()));

    let builder = builder
        // ── Cross-platform plugins ──────────────────────────────────
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        ;

    // ── Desktop-only plugins ────────────────────────────────────
    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }));

    // ── Commands ─────────────────────────────────────────────────
    #[cfg(desktop)]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::open_external,
        commands::set_process_priority,
        commands::get_close_to_tray,
        commands::set_close_to_tray,
        commands::set_badge_count,
        commands::flash_taskbar,
        commands::prevent_sleep,
        commands::allow_sleep,
        commands::detect_activity,
        commands::save_window_state,
        commands::restore_window_state,
        commands::load_custom_css,
        commands::save_custom_css,
        commands::open_overlay,
        commands::close_overlay,
        commands::set_overlay_interactive,
        commands::set_overlay_opacity,
    ]);

    #[cfg(mobile)]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::open_external,
        commands::get_close_to_tray,
        commands::set_close_to_tray,
        commands::load_custom_css,
        commands::save_custom_css,
    ]);

    // ── Setup (tray + global shortcuts — desktop only) ───────────
    #[cfg(desktop)]
    let builder = builder.setup(|app| {
        tray::setup(app)?;
        Ok(())
    });

    // ── Close-to-tray (desktop only) ────────────────────────────
    // When close_to_tray is true, clicking X hides to tray instead of closing.
    // When false (default), X closes the app normally. Tray icon is always visible.
    #[cfg(desktop)]
    let builder = builder.on_window_event(|window, event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            if window.label() != "main" {
                return;
            }
            let close_to_tray = commands::get_close_to_tray(window.app_handle().clone());
            if close_to_tray {
                api.prevent_close();
                let _ = window.hide();
            }
        }
    });

    builder
}

// ── Mobile entry point ──────────────────────────────────────────────
#[cfg(mobile)]
#[tauri::mobile_entry_point]
pub fn run() {
    build_app()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
