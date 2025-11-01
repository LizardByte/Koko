//! Connection management for WebSocket communication with server.

use anyhow::Result;
use futures::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Message types from server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    Frame {
        data: Vec<u8>,
        width: u32,
        height: u32,
        timestamp: u64,
        monitor_index: usize,
    },
    Clipboard {
        text: String,
        timestamp: u64,
    },
    Monitors {
        monitors: Vec<MonitorInfo>,
    },
    Error {
        message: String,
    },
}

/// Monitor information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub index: usize,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

/// Message types to server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    Input { event: InputEvent },
    Clipboard { text: String },
    GetMonitors,
}

/// Input event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InputEvent {
    Mouse(MouseEvent),
    Keyboard(KeyEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MouseEvent {
    Move { x: i32, y: i32 },
    ButtonDown { button: MouseButton },
    ButtonUp { button: MouseButton },
    Scroll { delta_x: i32, delta_y: i32 },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    Button4,
    Button5,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyEvent {
    KeyDown { code: u32, key: String },
    KeyUp { code: u32, key: String },
}

/// Connection state
pub struct Connection {
    sender: mpsc::UnboundedSender<ClientMessage>,
    pub frame_rx: Arc<RwLock<mpsc::UnboundedReceiver<Vec<u8>>>>,
    pub monitors: Arc<RwLock<Vec<MonitorInfo>>>,
    pub clipboard_rx: Arc<RwLock<mpsc::UnboundedReceiver<String>>>,
}

impl Connection {
    /// Connect to the server
    pub async fn connect(
        url: &str,
        token: &str,
    ) -> Result<Self> {
        let url_with_auth = format!("{}?token={}", url, token);
        info!("Connecting to server: {}", url);

        let (ws_stream, _) = connect_async(&url_with_auth).await?;
        info!("WebSocket connection established");

        let (write, mut read) = ws_stream.split();

        // Channels for communication
        let (client_tx, mut client_rx) = mpsc::unbounded_channel::<ClientMessage>();
        let (frame_tx, frame_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (clipboard_tx, clipboard_rx) = mpsc::unbounded_channel::<String>();

        let monitors = Arc::new(RwLock::new(Vec::new()));
        let monitors_clone = monitors.clone();

        // Spawn task to send messages to server
        tokio::spawn(async move {
            let mut write = write;
            while let Some(msg) = client_rx.recv().await {
                if let Ok(json) = serde_json::to_string(&msg) {
                    if write.send(Message::Text(json)).await.is_err() {
                        error!("Failed to send message to server");
                        break;
                    }
                }
            }
        });

        // Spawn task to receive messages from server
        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(server_msg) = serde_json::from_str::<ServerMessage>(&text) {
                            match server_msg {
                                ServerMessage::Frame { data, .. } => {
                                    let _ = frame_tx.send(data);
                                }
                                ServerMessage::Clipboard { text, .. } => {
                                    let _ = clipboard_tx.send(text);
                                }
                                ServerMessage::Monitors { monitors } => {
                                    *monitors_clone.write().await = monitors;
                                    info!(
                                        "Received monitor list: {} monitors",
                                        monitors_clone.read().await.len()
                                    );
                                }
                                ServerMessage::Error { message } => {
                                    error!("Server error: {}", message);
                                }
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        info!("Server closed connection");
                        break;
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(Self {
            sender: client_tx,
            frame_rx: Arc::new(RwLock::new(frame_rx)),
            monitors,
            clipboard_rx: Arc::new(RwLock::new(clipboard_rx)),
        })
    }

    /// Send input event to server
    pub fn send_input(
        &self,
        event: InputEvent,
    ) -> Result<()> {
        self.sender.send(ClientMessage::Input { event })?;
        Ok(())
    }

    /// Send clipboard content to server
    pub fn send_clipboard(
        &self,
        text: String,
    ) -> Result<()> {
        self.sender.send(ClientMessage::Clipboard { text })?;
        Ok(())
    }

    /// Request monitor list from server
    pub fn request_monitors(&self) -> Result<()> {
        self.sender.send(ClientMessage::GetMonitors)?;
        Ok(())
    }
}
