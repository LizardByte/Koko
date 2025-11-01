//! Configuration for remote desktop capture and streaming.

use serde::{Deserialize, Serialize};

/// Remote desktop configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteDesktopConfig {
    /// Capture settings
    pub capture: CaptureSettings,
    /// Streaming settings
    pub streaming: StreamingSettings,
}

/// Capture settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSettings {
    /// Target framerate (default: 60)
    pub framerate: u32,
    /// Hardware acceleration mode (auto, nvenc, vaapi, none)
    pub hw_accel: String,
    /// Video bitrate in kbps (default: 10000)
    pub bitrate: u32,
    /// Encoder preset (ultrafast, fast, medium, slow)
    pub preset: String,
    /// Monitors to capture (empty = all monitors)
    pub monitors: Vec<usize>,
}

/// Streaming settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingSettings {
    /// Enable clipboard synchronization
    pub enable_clipboard: bool,
    /// Maximum clients allowed
    pub max_clients: usize,
}

impl Default for RemoteDesktopConfig {
    fn default() -> Self {
        Self {
            capture: CaptureSettings {
                framerate: 60,
                hw_accel: "auto".to_string(),
                bitrate: 10000,
                preset: "ultrafast".to_string(),
                monitors: vec![],
            },
            streaming: StreamingSettings {
                enable_clipboard: true,
                max_clients: 10,
            },
        }
    }
}

