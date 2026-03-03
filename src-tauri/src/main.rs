// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2025–2026 Loqa Contributors
//! Loqa Desktop — Tauri application entry point (desktop only).

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::path::PathBuf;

fn main() {
    // ── Crash reporter ──────────────────────────────────────────
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let crash_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("com.loqa.desktop");
        let _ = fs::create_dir_all(&crash_dir);
        let crash_path = crash_dir.join("crash.log");
        let timestamp = chrono::Local::now().to_rfc3339();
        let msg = format!("[{}] PANIC: {}\n", timestamp, info);
        let _ = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&crash_path)
            .and_then(|mut f| {
                use std::io::Write;
                f.write_all(msg.as_bytes())
            });
        default_hook(info);
    }));

    loqa_desktop::build_app()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
