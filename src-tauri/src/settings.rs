// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2025–2026 Loqa Contributors
//! Persistent JSON settings stored in the app config directory.
//! The frontend syncs relevant toggles here via Tauri commands
//! because Rust cannot read the browser's localStorage directly.

use std::fs;
use std::path::PathBuf;
use tauri::Manager;

/// Resolve the path to the settings file.
pub fn settings_path(app: &tauri::AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    dir.join("settings.json")
}

/// Read the settings JSON from disk, returning `{}` on any error.
pub fn read(app: &tauri::AppHandle) -> serde_json::Value {
    let path = settings_path(app);
    match fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or(serde_json::json!({})),
        Err(_) => serde_json::json!({}),
    }
}

/// Write the settings JSON to disk, creating parent directories as needed.
pub fn write(app: &tauri::AppHandle, settings: &serde_json::Value) {
    let path = settings_path(app);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, serde_json::to_string_pretty(settings).unwrap_or_default());
}
