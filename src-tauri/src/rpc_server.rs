// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2025-2026 Loqa Contributors
//! Discord-compatible Rich Presence IPC server.
//!
//! Listens on the same named pipes (Windows) or Unix domain sockets
//! (macOS/Linux) that Discord uses (`discord-ipc-{0-9}`), allowing
//! games and applications with existing Rich Presence integration to
//! send activity data to Loqa automatically.
//!
//! Protocol: each frame is `[opcode: u32 LE][length: u32 LE][json: utf8]`
//!
//! Opcodes:
//!   0 = HANDSHAKE (client -> server)
//!   1 = FRAME     (bidirectional)
//!   2 = CLOSE     (bidirectional)
//!   3 = PING      (client -> server)
//!   4 = PONG      (server -> client)

use serde_json::{json, Value};
use tauri::Emitter;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_CLOSE: u32 = 2;
const OP_PING: u32 = 3;
const OP_PONG: u32 = 4;

/// Maximum JSON payload size (256 KB).
const MAX_PAYLOAD: u32 = 256 * 1024;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Spawn the IPC server on a background Tokio task.
///
/// Tries to bind `discord-ipc-{0..9}` and accepts connections on the
/// first available index. If all indices are occupied the server
/// silently exits.
pub fn start(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run_server(app).await {
            eprintln!("[RPC] server error: {e}");
        }
    });
}

// ---------------------------------------------------------------------------
// Platform-specific listener
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod platform {
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use tokio::io::{AsyncRead, AsyncWrite};
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

    pub struct Listener {
        pipe_name: String,
    }

    pub struct ClientStream {
        inner: NamedPipeServer,
    }

    impl AsyncRead for ClientStream {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
        }
    }

    impl AsyncWrite for ClientStream {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
        }
        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Pin::new(&mut self.get_mut().inner).poll_flush(cx)
        }
        fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
        }
    }

    impl Listener {
        pub fn bind(index: u32) -> std::io::Result<Self> {
            let pipe_name = format!(r"\\.\pipe\discord-ipc-{index}");
            // Create first instance to verify the name is free.
            let _probe = ServerOptions::new()
                .first_pipe_instance(true)
                .create(&pipe_name)?;
            Ok(Listener { pipe_name })
        }

        pub async fn accept(&self) -> std::io::Result<ClientStream> {
            let server = ServerOptions::new()
                .first_pipe_instance(false)
                .create(&self.pipe_name)?;
            server.connect().await?;
            Ok(ClientStream { inner: server })
        }

        pub fn display_name(&self) -> &str {
            &self.pipe_name
        }
    }

    impl Drop for Listener {
        fn drop(&mut self) {
            // Named pipes are cleaned up by the OS when all handles close.
        }
    }
}

#[cfg(unix)]
mod platform {
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use tokio::io::{AsyncRead, AsyncWrite};
    use tokio::net::{UnixListener, UnixStream};

    pub struct Listener {
        inner: UnixListener,
        path: String,
    }

    pub struct ClientStream {
        inner: UnixStream,
    }

    impl AsyncRead for ClientStream {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
        }
    }

    impl AsyncWrite for ClientStream {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
        }
        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Pin::new(&mut self.get_mut().inner).poll_flush(cx)
        }
        fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
        }
    }

    impl Listener {
        pub fn bind(index: u32) -> std::io::Result<Self> {
            let dir = std::env::var("XDG_RUNTIME_DIR")
                .or_else(|_| std::env::var("TMPDIR"))
                .unwrap_or_else(|_| "/tmp".to_string());
            let path = format!("{dir}/discord-ipc-{index}");
            let _ = std::fs::remove_file(&path); // remove stale socket
            let inner = UnixListener::bind(&path)?;
            Ok(Listener { inner, path })
        }

        pub async fn accept(&self) -> std::io::Result<ClientStream> {
            let (stream, _addr) = self.inner.accept().await?;
            Ok(ClientStream { inner: stream })
        }

        pub fn display_name(&self) -> &str {
            &self.path
        }
    }

    impl Drop for Listener {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

// ---------------------------------------------------------------------------
// Server loop
// ---------------------------------------------------------------------------

async fn run_server(app: tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let listener = (0..10u32)
        .find_map(|i| match platform::Listener::bind(i) {
            Ok(l) => {
                println!("[RPC] listening on {}", l.display_name());
                Some(l)
            }
            Err(_) => None,
        })
        .ok_or("all discord-ipc-{0..9} slots are occupied")?;

    loop {
        match listener.accept().await {
            Ok(stream) => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = handle_connection(stream, app).await {
                        eprintln!("[RPC] connection error: {e}");
                    }
                });
            }
            Err(e) => {
                eprintln!("[RPC] accept error: {e}");
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Per-connection handler
// ---------------------------------------------------------------------------

async fn handle_connection(
    mut stream: platform::ClientStream,
    app: tauri::AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut handshake_done = false;
    let mut client_id = String::new();

    loop {
        // Read 8-byte frame header
        let mut header = [0u8; 8];
        match stream.read_exact(&mut header).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                let _ = app.emit("rpc:activity-clear", json!({}));
                break;
            }
            Err(e) => return Err(e.into()),
        }

        let opcode = u32::from_le_bytes(header[0..4].try_into().unwrap());
        let length = u32::from_le_bytes(header[4..8].try_into().unwrap());

        if length > MAX_PAYLOAD {
            eprintln!("[RPC] payload too large: {length} bytes");
            break;
        }

        let mut payload = vec![0u8; length as usize];
        stream.read_exact(&mut payload).await?;
        let json_str = String::from_utf8_lossy(&payload);

        match opcode {
            OP_HANDSHAKE => {
                let body: Value = serde_json::from_str(&json_str).unwrap_or(json!({}));
                let version = body.get("v").and_then(|v| v.as_u64()).unwrap_or(0);
                client_id = body
                    .get("client_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                if version != 1 {
                    eprintln!("[RPC] unsupported protocol version: {version}");
                    send_frame(
                        &mut stream,
                        OP_CLOSE,
                        &json!({"code": 4000, "message": "unsupported version"}),
                    )
                    .await?;
                    break;
                }

                let ready = json!({
                    "cmd": "DISPATCH",
                    "evt": "READY",
                    "data": {
                        "v": 1,
                        "config": {
                            "cdn_host": "cdn.loqa.chat",
                            "api_endpoint": "https://app.loqa.chat/api",
                            "environment": "production",
                        },
                        "user": {
                            "id": "0",
                            "username": "Loqa",
                            "discriminator": "0000",
                            "avatar": null,
                        }
                    }
                });
                send_frame(&mut stream, OP_FRAME, &ready).await?;
                handshake_done = true;
                println!("[RPC] handshake OK, client_id={client_id}");
            }

            OP_FRAME if handshake_done => {
                let body: Value = serde_json::from_str(&json_str).unwrap_or(json!({}));
                let cmd = body.get("cmd").and_then(|v| v.as_str()).unwrap_or("");
                let nonce = body.get("nonce").cloned().unwrap_or(json!(null));

                match cmd {
                    "SET_ACTIVITY" => {
                        let activity = body
                            .get("args")
                            .and_then(|a| a.get("activity"))
                            .cloned()
                            .unwrap_or(json!(null));

                        if activity.is_null() {
                            let _ = app.emit("rpc:activity-clear", json!({}));
                        } else {
                            let _ = app.emit(
                                "rpc:activity-update",
                                json!({
                                    "client_id": client_id,
                                    "activity": activity,
                                }),
                            );
                        }

                        send_frame(
                            &mut stream,
                            OP_FRAME,
                            &json!({
                                "cmd": "SET_ACTIVITY",
                                "data": body.get("args")
                                    .and_then(|a| a.get("activity"))
                                    .cloned()
                                    .unwrap_or(json!(null)),
                                "evt": null,
                                "nonce": nonce,
                            }),
                        )
                        .await?;
                    }

                    // Acknowledge commands we don't implement so games
                    // don't hang or throw errors.
                    "SUBSCRIBE" | "UNSUBSCRIBE" | "GET_GUILDS" | "GET_GUILD"
                    | "GET_CHANNELS" | "GET_CHANNEL" | "SET_USER_VOICE_SETTINGS"
                    | "SELECT_VOICE_CHANNEL" | "GET_SELECTED_VOICE_CHANNEL"
                    | "SELECT_TEXT_CHANNEL" | "GET_VOICE_SETTINGS"
                    | "SET_VOICE_SETTINGS" | "SET_CERTIFIED_DEVICES"
                    | "SEND_ACTIVITY_JOIN_INVITE" | "CLOSE_ACTIVITY_REQUEST" => {
                        send_frame(
                            &mut stream,
                            OP_FRAME,
                            &json!({
                                "cmd": cmd,
                                "data": null,
                                "evt": null,
                                "nonce": nonce,
                            }),
                        )
                        .await?;
                    }

                    other => {
                        eprintln!("[RPC] unknown command: {other}");
                        send_frame(
                            &mut stream,
                            OP_FRAME,
                            &json!({
                                "cmd": other,
                                "data": null,
                                "evt": "ERROR",
                                "nonce": nonce,
                            }),
                        )
                        .await?;
                    }
                }
            }

            OP_CLOSE => {
                let _ = app.emit("rpc:activity-clear", json!({}));
                println!("[RPC] client disconnected");
                break;
            }

            OP_PING => {
                send_raw(&mut stream, OP_PONG, payload.as_slice()).await?;
            }

            _ => {
                eprintln!(
                    "[RPC] unexpected opcode {opcode} (handshake_done={handshake_done})"
                );
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Frame writing helpers
// ---------------------------------------------------------------------------

async fn send_frame(
    stream: &mut platform::ClientStream,
    opcode: u32,
    data: &Value,
) -> Result<(), std::io::Error> {
    let json = serde_json::to_vec(data).unwrap_or_default();
    send_raw(stream, opcode, &json).await
}

async fn send_raw(
    stream: &mut platform::ClientStream,
    opcode: u32,
    payload: &[u8],
) -> Result<(), std::io::Error> {
    let mut buf = Vec::with_capacity(8 + payload.len());
    buf.extend_from_slice(&opcode.to_le_bytes());
    buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    buf.extend_from_slice(payload);
    stream.write_all(&buf).await?;
    stream.flush().await
}
