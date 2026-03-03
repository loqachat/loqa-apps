// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2025–2026 Loqa Contributors
//! All `#[tauri::command]` handler functions.
//!
//! Grouped by feature:
//! - External links & process priority
//! - Close-to-tray preference
//! - Sleep prevention
//! - Game/app detection (rich presence)
//! - Window position memory
//! - Custom CSS injection
//! - Overlay window management

use std::fs;
#[cfg(desktop)]
use std::sync::Mutex;
use tauri::Manager;

use crate::settings;

// ── External links ──────────────────────────────────────────────────

/// Open a URL in the default browser.
/// Only allows http:// and https:// URLs to prevent protocol-handler abuse.
#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("Only HTTP(S) URLs are allowed".into());
    }
    open::that(&url).map_err(|e| e.to_string())
}

// ── Process priority (desktop only) ─────────────────────────────────

/// Set the process priority for streamer mode.
/// "high" = encoding headroom, "below_normal" = yield to games, "normal" = default.
#[cfg(desktop)]
#[tauri::command]
pub fn set_process_priority(level: String) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::{
            GetCurrentProcess, SetPriorityClass,
            HIGH_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS,
        };
        let priority = match level.as_str() {
            "high" => HIGH_PRIORITY_CLASS,
            "below_normal" => BELOW_NORMAL_PRIORITY_CLASS,
            _ => NORMAL_PRIORITY_CLASS,
        };
        unsafe {
            let handle = GetCurrentProcess();
            SetPriorityClass(handle, priority);
        }
    }
    #[cfg(unix)]
    {
        use std::process::Command;
        let niceness = match level.as_str() {
            "high" => "-10",
            "below_normal" => "10",
            _ => "0",
        };
        let pid = std::process::id();
        let _ = Command::new("renice").args([niceness, "-p", &pid.to_string()]).output();
    }
}

// ── Close-to-tray preference ────────────────────────────────────────

/// Get the close-to-tray preference.
#[tauri::command]
pub fn get_close_to_tray(app: tauri::AppHandle) -> bool {
    let s = settings::read(&app);
    s.get("close_to_tray")
        .and_then(|v| v.as_bool())
        .unwrap_or(false) // default: X closes the app; tray icon stays visible independently
}

/// Set the close-to-tray preference.
#[tauri::command]
pub fn set_close_to_tray(app: tauri::AppHandle, value: bool) {
    let mut s = settings::read(&app);
    s.as_object_mut().unwrap().insert("close_to_tray".to_string(), serde_json::json!(value));
    settings::write(&app, &s);
}

// ── Badge count (desktop only — taskbar title + attention flash) ────

/// Set the unread badge count — updates the window title and optionally
/// flashes the taskbar icon. Pass 0 to clear.
#[cfg(desktop)]
#[tauri::command]
pub fn set_badge_count(app: tauri::AppHandle, count: u32, flash: bool) {
    if let Some(w) = app.get_webview_window("main") {
        if count > 0 {
            let _ = w.set_title(&format!("Loqa ({})", count));
            if flash && !w.is_focused().unwrap_or(true) {
                let _ = w.request_user_attention(Some(tauri::UserAttentionType::Informational));
            }
        } else {
            let _ = w.set_title("Loqa");
        }
    }
    // Update tray tooltip in sync
    update_tray_tooltip_inner(&app, count);
}

/// Flash the taskbar icon without changing focus (lighter than focusWindow).
#[cfg(desktop)]
#[tauri::command]
pub fn flash_taskbar(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if !w.is_focused().unwrap_or(true) {
            let _ = w.request_user_attention(Some(tauri::UserAttentionType::Informational));
        }
    }
}

/// Update the system tray tooltip text with the unread count.
#[cfg(desktop)]
fn update_tray_tooltip_inner(app: &tauri::AppHandle, count: u32) {
    if let Some(tray) = app.tray_by_id("main") {
        let tooltip = if count > 0 {
            format!("Loqa — {} unread message{}", count, if count == 1 { "" } else { "s" })
        } else {
            "Loqa".to_string()
        };
        let _ = tray.set_tooltip(Some(&tooltip));
    }
}

// ── Sleep prevention (desktop only) ─────────────────────────────────

/// Prevent the system from sleeping (used during voice calls).
#[cfg(desktop)]
#[tauri::command]
pub fn prevent_sleep() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Power::SetThreadExecutionState;
        use windows_sys::Win32::System::Power::{
            ES_CONTINUOUS, ES_SYSTEM_REQUIRED, ES_DISPLAY_REQUIRED,
        };
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED);
        }
    }
    #[cfg(target_os = "macos")]
    {
        // macOS: use caffeinate process to prevent sleep
        use std::process::Command;
        let _ = Command::new("caffeinate").args(["-d", "-i", "-w", &std::process::id().to_string()]).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        // Linux: use systemd-inhibit if available
        use std::process::Command;
        let _ = Command::new("systemd-inhibit")
            .args(["--what=idle:sleep", "--who=Loqa", "--why=Voice call active", "--mode=block", "sleep", "infinity"])
            .spawn();
    }
}

/// Allow the system to sleep again.
#[cfg(desktop)]
#[tauri::command]
pub fn allow_sleep() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Power::SetThreadExecutionState;
        use windows_sys::Win32::System::Power::ES_CONTINUOUS;
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
    // macOS/Linux: the caffeinate/systemd-inhibit processes are tied to the app PID
    // and will terminate automatically when the app exits or the call ends.
}

// ── Rich Presence — Game/App Detection (desktop only) ───────────────

#[cfg(desktop)]
#[derive(serde::Deserialize, Clone)]
struct GameEntry {
    exe: String,
    name: String,
}

/// Load game entries: prefer user-custom file, fall back to bundled data.
#[cfg(desktop)]
fn load_games(app: &tauri::AppHandle) -> Vec<GameEntry> {
    if let Ok(config_dir) = app.path().app_config_dir() {
        let custom_path = config_dir.join("games.json");
        if let Ok(contents) = fs::read_to_string(&custom_path) {
            if let Ok(games) = serde_json::from_str::<Vec<GameEntry>>(&contents) {
                return games;
            }
        }
    }
    let bundled = include_str!("../data/games.json");
    serde_json::from_str(bundled).unwrap_or_default()
}

/// Detect the currently running game/app by scanning processes.
/// Returns the display name of the first matched game, or null.
///
/// Optimization: uses ProcessRefreshKind::nothing() since we only need
/// process names (no CPU/memory/disk stats). This dramatically reduces
/// the per-scan cost from reading /proc/*/stat to just /proc/*/cmdline.
#[cfg(desktop)]
#[tauri::command]
pub fn detect_activity(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<sysinfo::System>>,
) -> Option<String> {
    let games = load_games(&app);
    if games.is_empty() { return None; } // fast path: no games configured
    let mut sys = state.lock().unwrap();
    sys.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        sysinfo::ProcessRefreshKind::nothing(),
    );

    for process in sys.processes().values() {
        let exe_name = process.name().to_string_lossy().to_lowercase();
        let name = exe_name.trim_end_matches(".exe");
        for game in &games {
            if name.contains(&game.exe) {
                return Some(game.name.clone());
            }
        }
    }
    None
}

// ── Window position memory (desktop only) ───────────────────────────

/// Save the current window position and size to settings.
#[cfg(desktop)]
#[tauri::command]
pub fn save_window_state(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let mut s = settings::read(&app);
        if let Ok(pos) = w.outer_position() {
            s["window_x"] = serde_json::json!(pos.x);
            s["window_y"] = serde_json::json!(pos.y);
        }
        if let Ok(size) = w.outer_size() {
            s["window_width"] = serde_json::json!(size.width);
            s["window_height"] = serde_json::json!(size.height);
        }
        if let Ok(maximized) = w.is_maximized() {
            s["window_maximized"] = serde_json::json!(maximized);
        }
        settings::write(&app, &s);
    }
}

/// Restore the window position and size from settings.
/// Guards against invalid values (e.g. Windows minimized sentinel: -32000, -32000).
#[cfg(desktop)]
#[tauri::command]
pub fn restore_window_state(app: tauri::AppHandle) {
    let s = settings::read(&app);
    if let Some(w) = app.get_webview_window("main") {
        use tauri::{LogicalPosition, LogicalSize};
        if let (Some(x), Some(y)) = (s["window_x"].as_f64(), s["window_y"].as_f64()) {
            // Skip if position is the Windows minimized sentinel (-32000)
            if x > -10000.0 && y > -10000.0 {
                let _ = w.set_position(LogicalPosition::new(x, y));
            }
        }
        if let (Some(width), Some(height)) = (s["window_width"].as_f64(), s["window_height"].as_f64()) {
            // Skip if size is unreasonably small (minimized windows report ~192x106)
            if width >= 200.0 && height >= 200.0 {
                let _ = w.set_size(LogicalSize::new(width, height));
            }
        }
        if s["window_maximized"].as_bool() == Some(true) {
            let _ = w.maximize();
        }
    }
}

// ── Custom CSS injection ────────────────────────────────────────────

/// Load custom CSS from the user's config directory (re-sanitized on load).
#[tauri::command]
pub fn load_custom_css(app: tauri::AppHandle) -> Option<String> {
    let dir = app.path().app_config_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    fs::read_to_string(dir.join("custom.css"))
        .ok()
        .map(|raw| sanitize_css(&raw))
}

/// Save custom CSS content (sanitized) to the user's config directory.
#[tauri::command]
pub fn save_custom_css(app: tauri::AppHandle, css: String) -> Result<(), String> {
    let sanitized = sanitize_css(&css);
    let dir = app.path().app_config_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let _ = fs::create_dir_all(&dir);
    fs::write(dir.join("custom.css"), sanitized).map_err(|e| e.to_string())
}

/// Strip @import rules and url() functions to prevent data exfiltration.
fn sanitize_css(css: &str) -> String {
    let mut result = String::with_capacity(css.len());
    for line in css.lines() {
        let trimmed = line.trim().to_lowercase();
        if trimmed.starts_with("@import") {
            result.push_str("/* [blocked: @import removed for security] */");
            result.push('\n');
            continue;
        }
        result.push_str(&remove_url_functions(line));
        result.push('\n');
    }
    result
}

/// Remove url(...) function calls from a CSS line.
fn remove_url_functions(line: &str) -> String {
    let lower = line.to_lowercase();
    if !lower.contains("url(") {
        return line.to_string();
    }
    let mut result = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 4 <= bytes.len() && lower[i..].starts_with("url(") {
            let mut depth = 1;
            let mut j = i + 4;
            while j < bytes.len() && depth > 0 {
                if bytes[j] == b'(' { depth += 1; }
                if bytes[j] == b')' { depth -= 1; }
                j += 1;
            }
            result.push_str("/* [blocked: url() removed] */");
            i = j;
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }
    result
}

// ── Overlay window management (desktop only) ────────────────────────

/// Validate that a string looks like a snowflake ID (digits only, 1–20 chars).
#[cfg(desktop)]
fn is_valid_snowflake(s: &str) -> bool {
    !s.is_empty() && s.len() <= 20 && s.chars().all(|c| c.is_ascii_digit())
}

/// Open a transparent overlay window for streaming chat.
#[cfg(desktop)]
#[tauri::command]
pub fn open_overlay(app: tauri::AppHandle, channel_id: String, server_id: String) -> Result<(), String> {
    if !is_valid_snowflake(&channel_id) {
        return Err("Invalid channel ID".into());
    }
    if !is_valid_snowflake(&server_id) {
        return Err("Invalid server ID".into());
    }

    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }

    let url = format!("/overlay?channelId={}&serverId={}", channel_id, server_id);

    tauri::WebviewWindowBuilder::new(
        &app,
        "overlay",
        tauri::WebviewUrl::App(url.into()),
    )
    .title("Loqa Chat Overlay")
    .inner_size(400.0, 600.0)
    .always_on_top(true)
    .transparent(true)
    .decorations(false)
    .skip_taskbar(true)
    .resizable(true)
    .build()
    .map_err(|e: tauri::Error| e.to_string())?;

    Ok(())
}

/// Close the overlay window.
#[cfg(desktop)]
#[tauri::command]
pub fn close_overlay(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.destroy();
    }
}

/// Toggle whether the overlay window ignores cursor events (click-through).
#[cfg(desktop)]
#[tauri::command]
pub fn set_overlay_interactive(app: tauri::AppHandle, interactive: bool) {
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.set_ignore_cursor_events(!interactive);
    }
}

/// Set the overlay window opacity (0.0–1.0).
#[cfg(desktop)]
#[tauri::command]
pub fn set_overlay_opacity(app: tauri::AppHandle, opacity: f64) {
    if let Some(w) = app.get_webview_window("overlay") {
        let clamped = opacity.max(0.1).min(1.0);
        let _ = w.set_effects(tauri::utils::config::WindowEffectsConfig {
            effects: vec![],
            state: None,
            radius: None,
            color: Some(tauri::utils::config::Color(0, 0, 0, (clamped * 255.0) as u8)),
        });
    }
}
