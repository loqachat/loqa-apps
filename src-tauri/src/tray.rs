// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2025–2026 Loqa Contributors
//! System tray setup and push-to-talk global shortcut registration.

use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

/// Build the system tray icon, context menu, and register the global
/// push-to-talk shortcut. Called once during `App::setup`.
pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // ── System tray menu ────────────────────────────────────────
    let show = MenuItemBuilder::with_id("show", "Show Loqa").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit Loqa").build(app)?;
    let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;

    TrayIconBuilder::with_id("main")
        .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png"))?)
        .menu(&menu)
        .tooltip("Loqa")
        .on_menu_event(|app: &tauri::AppHandle, event: tauri::menu::MenuEvent| match event.id().as_ref() {
            "show" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
            }
            "quit" => {
                std::process::exit(0);
            }
            _ => {}
        })
        // Click tray icon → show + focus window
        .on_tray_icon_event(|tray: &tauri::tray::TrayIcon, event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if let Some(w) = tray.app_handle().get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
            }
        })
        .build(app)?;

    // ── Global push-to-talk shortcut ────────────────────────────
    let app_handle = app.handle().clone();
    use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};
    let shortcut = Shortcut::new(Some(Modifiers::empty()), Code::ControlRight);
    if let Err(e) = app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
        let pressed = event.state == ShortcutState::Pressed;
        let _ = app_handle.emit("push-to-talk", serde_json::json!({ "pressed": pressed }));
    }) {
        eprintln!("[Loqa] Push-to-talk global shortcut registration failed (non-fatal): {e}");
    }

    Ok(())
}
