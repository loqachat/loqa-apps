# Loqa Desktop

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![GitHub Release](https://img.shields.io/github/v/release/loqachat/loqa?label=Download&color=brightgreen)](https://github.com/loqachat/loqa/releases/latest)

Native desktop client for [Loqa](https://loqa.chat) — a federated, encrypted chat platform. Built with [Tauri v2](https://v2.tauri.app/).

The desktop app is a lightweight native shell (~4 MB) that loads the Loqa web app from `app.loqa.chat`. All platform-specific features — system tray, notifications, taskbar badges, push-to-talk, deep links, and auto-updates — are implemented in Rust via Tauri's IPC bridge.

## Download

> **[📥 Download Latest Release](https://github.com/loqachat/loqa/releases/latest)**

| Platform | Architecture | Format | Size |
|----------|-------------|--------|------|
| **Windows** | x64 | `.exe` (NSIS installer) | ~10 MB |
| **macOS** | Apple Silicon (M1/M2/M3+) | `.dmg` | ~7 MB |
| **macOS** | Intel | `.dmg` | ~7 MB |
| **Linux** | x64 | `.deb` · `.AppImage` | ~9 MB |

All builds are compiled automatically via GitHub Actions and update-signed.

## Architecture

```
┌──────────────────────────────────────┐
│           Tauri Shell (Rust)         │
│  ┌──────────┐  ┌──────────────────┐  │
│  │ System   │  │ IPC Commands     │  │
│  │ Tray     │  │ • Badge count    │  │
│  │          │  │ • Flash taskbar  │  │
│  └──────────┘  │ • Notifications  │  │
│                │ • Sleep prevent  │  │
│                │ • Window state   │  │
│                └──────────────────┘  │
│  ┌──────────────────────────────────┐│
│  │ WebView2 → https://app.loqa.chat ││
│  └──────────────────────────────────┘│
└──────────────────────────────────────┘
```

The frontend is **not bundled** — it's loaded remotely. This means:
- Desktop updates only ship when native features change
- Frontend updates deploy instantly without a new installer
- The desktop repo is fully self-contained (no frontend dependency at build time)

## Prerequisites

- **Rust** ≥ 1.77 — `rustup update stable`
- **Tauri CLI v2** — `cargo install tauri-cli --version "^2"`
- **WebView2** — pre-installed on Windows 10/11

## Development

```bash
# Install the Tauri CLI (one-time)
cargo install tauri-cli --version "^2"

# Launch the desktop app (loads app.loqa.chat)
cargo tauri dev
```

## Production Build

```bash
cargo tauri build
```

Installers are output to `src-tauri/target/release/bundle/`:

| Platform | Formats |
|----------|---------|
| **Windows** | `.exe` (NSIS), `.msi` (WiX) |
| **macOS** | `.dmg`, `.app` |
| **Linux** | `.deb`, `.rpm`, `.AppImage` |

### Linux Build Dependencies

On Debian/Ubuntu:
```bash
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev \
  librsvg2-dev patchelf libssl-dev libgtk-3-dev libayatana-appindicator3-dev
```

On Arch Linux:
```bash
sudo pacman -S webkit2gtk-4.1 libappindicator-gtk3 librsvg patchelf openssl gtk3
```

On Fedora:
```bash
sudo dnf install webkit2gtk4.1-devel libappindicator-gtk3-devel \
  librsvg2-devel openssl-devel gtk3-devel
```

## CI/CD

GitHub Actions builds for all platforms on every version tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

This triggers `.github/workflows/desktop-release.yml` which:
1. Builds on **Windows x64**, **macOS Intel**, **macOS ARM**, and **Linux x64**
2. Uploads all installers as artifacts
3. Creates a GitHub Release with all binaries

## Project Structure

```
loqa-desktop/
├── .gitignore
├── LICENSE                       # AGPL-3.0
├── README.md
├── package.json                  # npm scripts wrapping cargo tauri
├── keys/
│   └── loqa-update.key.pub      # Tauri updater public key
└── src-tauri/
    ├── Cargo.toml                # Rust deps + Tauri plugins
    ├── Cargo.lock
    ├── build.rs                  # Tauri build script
    ├── tauri.conf.json           # App config (remote URL, window, tray, bundle)
    ├── capabilities/
    │   └── default.json          # Plugin permissions
    ├── data/
    │   └── games.json            # Rich presence game database
    ├── icons/                    # App icons (generated via cargo tauri icon)
    └── src/
        ├── main.rs               # Entry point — plugins, commands, tray, close-to-tray
        ├── commands.rs            # IPC commands (badge, flash, sleep, window state)
        ├── tray.rs                # System tray setup (icon, menu, push-to-talk)
        └── settings.rs            # Persistent JSON settings (app config dir)
```

## Tauri Plugins

| Plugin | Purpose |
|--------|---------|
| `tauri-plugin-notification` | Native OS notifications (Windows Action Center) |
| `tauri-plugin-deep-link` | Handle `loqa://` protocol URLs |
| `tauri-plugin-global-shortcut` | Push-to-talk, keyboard shortcuts |
| `tauri-plugin-dialog` | Native file open/save dialogs |
| `tauri-plugin-clipboard-manager` | Rich clipboard (text, images, HTML) |
| `tauri-plugin-updater` | Automatic update mechanism |
| `tauri-plugin-process` | Process management (restart, exit) |
| `tauri-plugin-shell` | Open URLs in default browser |
| `tauri-plugin-fs` | File system access (settings, downloads) |
| `tauri-plugin-single-instance` | Prevent duplicate app instances |

## IPC Commands

The Rust backend exposes these commands to the frontend via Tauri's invoke system:

| Command | Description |
|---------|-------------|
| `set_badge_count` | Update taskbar title with unread count + flash + tray tooltip |
| `flash_taskbar` | Blink the taskbar icon without focusing the window |
| `open_external` | Open a URL in the default browser |
| `set_process_priority` | Set process priority (used during voice calls) |
| `get/set_close_to_tray` | Toggle minimize-to-tray behavior |
| `prevent/allow_sleep` | Prevent system sleep during voice calls |
| `detect_activity` | Detect user input for auto-away status |
| `save/restore_window_state` | Persist window size and position |
| `load/save_custom_css` | Custom CSS injection support |
| `open/close_overlay` | Overlay window management |

## Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes
4. Run `cargo build` to verify compilation
5. Submit a pull request

## License

This project is licensed under the [GNU Affero General Public License v3.0](LICENSE).
