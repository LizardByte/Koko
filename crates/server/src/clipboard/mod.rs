//! Clipboard synchronization module.

use arboard::Clipboard;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

/// Clipboard content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardContent {
    /// Text content
    pub text: String,
    /// Timestamp
    pub timestamp: u64,
}

/// Clipboard manager for synchronization
pub struct ClipboardManager {
    clipboard: Arc<Mutex<Clipboard>>,
    content_sender: broadcast::Sender<ClipboardContent>,
    last_content: Arc<Mutex<String>>,
}

impl ClipboardManager {
    /// Create a new clipboard manager
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let clipboard =
            Clipboard::new().map_err(|e| format!("Failed to initialize clipboard: {}", e))?;

        let (content_sender, _) = broadcast::channel(100);

        Ok(Self {
            clipboard: Arc::new(Mutex::new(clipboard)),
            content_sender,
            last_content: Arc::new(Mutex::new(String::new())),
        })
    }

    /// Get a receiver for clipboard updates
    pub fn subscribe(&self) -> broadcast::Receiver<ClipboardContent> {
        self.content_sender.subscribe()
    }

    /// Set clipboard content (from remote client)
    pub fn set_content(
        &self,
        content: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut clipboard = self.clipboard.lock().unwrap();
        clipboard
            .set_text(content)
            .map_err(|e| format!("Failed to set clipboard: {}", e))?;

        // Update last content to avoid echoing back
        *self.last_content.lock().unwrap() = content.to_string();

        log::debug!("Clipboard set from remote: {} bytes", content.len());
        Ok(())
    }

    /// Get current clipboard content
    pub fn get_content(&self) -> Result<String, Box<dyn std::error::Error>> {
        let mut clipboard = self.clipboard.lock().unwrap();
        clipboard
            .get_text()
            .map_err(|e| format!("Failed to get clipboard: {}", e).into())
    }

    /// Start monitoring clipboard for changes
    pub fn start_monitoring(&self) {
        let clipboard = self.clipboard.clone();
        let sender = self.content_sender.clone();
        let last_content = self.last_content.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                let content = {
                    let mut cb = clipboard.lock().unwrap();
                    match cb.get_text() {
                        Ok(text) => text,
                        Err(_) => continue,
                    }
                };

                // Check if content has changed
                let mut last = last_content.lock().unwrap();
                if content != *last {
                    *last = content.clone();

                    let clipboard_content = ClipboardContent {
                        text: content,
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_micros() as u64,
                    };

                    // Broadcast to subscribers (ignore if no receivers)
                    let _ = sender.send(clipboard_content);
                }
            }
        });

        log::info!("Clipboard monitoring started");
    }
}
