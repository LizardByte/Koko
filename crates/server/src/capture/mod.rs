//! Screen capture module for Linux X11/Wayland systems.

pub mod monitor;
pub mod x11_capture;

use std::sync::Arc;
use tokio::sync::broadcast;

/// Frame data captured from the screen
#[derive(Clone)]
pub struct Frame {
    /// Raw frame data (encoded)
    pub data: Vec<u8>,
    /// Width of the frame
    pub width: u32,
    /// Height of the frame
    pub height: u32,
    /// Timestamp in microseconds
    pub timestamp: u64,
    /// Monitor index this frame belongs to
    pub monitor_index: usize,
}

/// Capture configuration
pub struct CaptureConfig {
    /// Target framerate
    pub framerate: u32,
    /// Enable hardware encoding (NVIDIA/AMD)
    pub hw_accel: HardwareAccel,
    /// Quality/bitrate settings
    pub quality: QualitySettings,
    /// Which monitors to capture
    pub monitors: Vec<usize>,
}

/// Hardware acceleration options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareAccel {
    /// No hardware acceleration
    None,
    /// NVIDIA NVENC
    Nvenc,
    /// AMD VCE/AMF
    Vaapi,
    /// Auto-detect
    Auto,
}

/// Quality settings for encoding
#[derive(Debug, Clone)]
pub struct QualitySettings {
    /// Target bitrate in kbps
    pub bitrate: u32,
    /// Encoder preset (ultrafast, fast, medium, slow)
    pub preset: String,
    /// Constant rate factor (lower = better quality)
    pub crf: u32,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            framerate: 60,
            hw_accel: HardwareAccel::Auto,
            quality: QualitySettings {
                bitrate: 10000,
                preset: "ultrafast".to_string(),
                crf: 23,
            },
            monitors: vec![0],
        }
    }
}

/// Screen capture manager
pub struct CaptureManager {
    config: CaptureConfig,
    frame_sender: broadcast::Sender<Frame>,
}

impl CaptureManager {
    /// Create a new capture manager
    pub fn new(config: CaptureConfig) -> Self {
        let (frame_sender, _) = broadcast::channel(100);
        Self {
            config,
            frame_sender,
        }
    }

    /// Get a receiver for frames
    pub fn subscribe(&self) -> broadcast::Receiver<Frame> {
        self.frame_sender.subscribe()
    }

    /// Start capturing screen
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!(
            "Starting screen capture with config: framerate={}, hw_accel={:?}",
            self.config.framerate,
            self.config.hw_accel
        );

        // Initialize capture based on platform
        #[cfg(target_os = "linux")]
        {
            x11_capture::start_capture(self.config.clone(), self.frame_sender.clone()).await?;
        }

        #[cfg(not(target_os = "linux"))]
        {
            log::error!("Screen capture is only supported on Linux");
            return Err("Unsupported platform".into());
        }

        Ok(())
    }

    /// Stop capturing
    pub fn stop(&self) {
        log::info!("Stopping screen capture");
    }
}
