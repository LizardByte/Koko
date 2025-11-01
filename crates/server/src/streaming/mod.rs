//! Streaming module for sending video frames to clients.

use crate::capture::CaptureManager;
use crate::clipboard::ClipboardManager;
use crate::input::{get_input_handler, InputEvent};
use futures::{SinkExt, StreamExt};
use rocket::State;
use rocket_ws as ws;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Message types sent from server to client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    /// Video frame data
    Frame {
        data: Vec<u8>,
        width: u32,
        height: u32,
        timestamp: u64,
        monitor_index: usize,
    },
    /// Clipboard content update
    Clipboard { text: String, timestamp: u64 },
    /// Monitor list
    Monitors { monitors: Vec<MonitorInfo> },
    /// Error message
    Error { message: String },
}

/// Monitor information for client
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

/// Message types sent from client to server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// Input event
    Input { event: InputEvent },
    /// Clipboard content from client
    Clipboard { text: String },
    /// Request monitor list
    GetMonitors,
}

/// Streaming session state
pub struct StreamSession {
    pub capture_manager: Arc<CaptureManager>,
    pub clipboard_manager: Arc<ClipboardManager>,
}

impl StreamSession {
    /// Create a new streaming session
    pub fn new(
        capture_manager: Arc<CaptureManager>,
        clipboard_manager: Arc<ClipboardManager>,
    ) -> Self {
        Self {
            capture_manager,
            clipboard_manager,
        }
    }

    /// Handle WebSocket connection for streaming
    pub async fn handle_connection(
        self: Arc<Self>,
        ws: ws::WebSocket,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (mut sink, mut stream) = ws.split();

        log::info!("New streaming client connected");

        // Subscribe to frame updates
        let mut frame_rx = self.capture_manager.subscribe();

        // Subscribe to clipboard updates
        let mut clipboard_rx = self.clipboard_manager.subscribe();

        // Get input handler
        let input_handler = get_input_handler()?;

        // Send initial monitor list
        let monitors = crate::capture::monitor::get_monitors()?;
        let monitor_infos: Vec<MonitorInfo> = monitors
            .into_iter()
            .map(|m| MonitorInfo {
                index: m.index,
                name: m.name,
                x: m.x,
                y: m.y,
                width: m.width,
                height: m.height,
                primary: m.primary,
            })
            .collect();

        let msg = ServerMessage::Monitors {
            monitors: monitor_infos,
        };
        let json = serde_json::to_string(&msg)?;
        sink.send(ws::Message::Text(json)).await?;

        // Spawn task to send frames
        let sink_clone = Arc::new(tokio::sync::Mutex::new(sink));
        let sink_for_frames = sink_clone.clone();
        let frame_task = tokio::spawn(async move {
            while let Ok(frame) = frame_rx.recv().await {
                let msg = ServerMessage::Frame {
                    data: frame.data,
                    width: frame.width,
                    height: frame.height,
                    timestamp: frame.timestamp,
                    monitor_index: frame.monitor_index,
                };

                if let Ok(json) = serde_json::to_string(&msg) {
                    let mut sink = sink_for_frames.lock().await;
                    if sink.send(ws::Message::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
        });

        // Spawn task to send clipboard updates
        let sink_for_clipboard = sink_clone.clone();
        let clipboard_task = tokio::spawn(async move {
            while let Ok(content) = clipboard_rx.recv().await {
                let msg = ServerMessage::Clipboard {
                    text: content.text,
                    timestamp: content.timestamp,
                };

                if let Ok(json) = serde_json::to_string(&msg) {
                    let mut sink = sink_for_clipboard.lock().await;
                    if sink.send(ws::Message::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
        });

        // Handle incoming client messages
        while let Some(message) = stream.next().await {
            match message {
                Ok(ws::Message::Text(text)) => {
                    if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                        match client_msg {
                            ClientMessage::Input { event } => {
                                if let Err(e) = input_handler.handle_event(event) {
                                    log::error!("Input handling error: {}", e);
                                }
                            }
                            ClientMessage::Clipboard { text } => {
                                if let Err(e) = self.clipboard_manager.set_content(&text) {
                                    log::error!("Clipboard set error: {}", e);
                                }
                            }
                            ClientMessage::GetMonitors => {
                                if let Ok(monitors) = crate::capture::monitor::get_monitors() {
                                    let monitor_infos: Vec<MonitorInfo> = monitors
                                        .into_iter()
                                        .map(|m| MonitorInfo {
                                            index: m.index,
                                            name: m.name,
                                            x: m.x,
                                            y: m.y,
                                            width: m.width,
                                            height: m.height,
                                            primary: m.primary,
                                        })
                                        .collect();

                                    let msg = ServerMessage::Monitors {
                                        monitors: monitor_infos,
                                    };
                                    if let Ok(json) = serde_json::to_string(&msg) {
                                        let mut sink = sink_clone.lock().await;
                                        let _ = sink.send(ws::Message::Text(json)).await;
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(ws::Message::Close(_)) => {
                    log::info!("Client closed connection");
                    break;
                }
                Err(e) => {
                    log::error!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        // Cancel background tasks
        frame_task.abort();
        clipboard_task.abort();

        log::info!("Streaming client disconnected");
        Ok(())
    }
}
