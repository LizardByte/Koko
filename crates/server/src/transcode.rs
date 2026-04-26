//! Transcoding engine for media playback.

// standard imports
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};

// lib imports
use tokio::process::Command;
use tokio::process::Child;
use tokio::fs;

// local imports
use crate::config::FfmpegSettings;

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

/// Create a new unique session ID.
pub fn next_session_id() -> String {
    format!("session-{}", NEXT_SESSION_ID.fetch_add(1, Ordering::SeqCst))
}

/// Details for an active FFmpeg transcoding process.
pub struct TranscodeProcess {
    /// The unique session ID for this process.
    pub session_id: String,
    /// The path to the output directory or file.
    pub output_path: PathBuf,
}

impl TranscodeProcess {
    /// Stop the transcode process and clean up temporary files.
    pub async fn cleanup(&self) {
        if let Some(parent) = self.output_path.parent() {
            let _ = fs::remove_dir_all(parent).await;
        }
    }
}

/// Specification for a transcode job.
#[derive(Debug, Clone)]
pub struct TranscodeSpec {
    /// The path to the source media file.
    pub source_path: PathBuf,
    /// The output path where the transcoded media will be written.
    pub output_path: PathBuf,
    /// The container format to use.
    pub container: String,
    /// The video codec to use, or None to copy.
    pub video_codec: Option<String>,
    /// The audio codec to use, or None to copy.
    pub audio_codec: Option<String>,
    /// The maximum width of the video.
    pub max_width: Option<u32>,
    /// The maximum height of the video.
    pub max_height: Option<u32>,
    /// The maximum total bitrate in kbps.
    pub max_bitrate_kbps: Option<u32>,
    /// The start time in milliseconds to seek to.
    pub start_time_ms: Option<i64>,
}

impl TranscodeSpec {
    /// Generate the FFmpeg command-line arguments for this specification.
    pub fn to_ffmpeg_args(&self) -> Vec<String> {
        self.to_ffmpeg_args_for_output(self.output_path.to_string_lossy().as_ref())
    }

    /// Generate FFmpeg command-line arguments using stdout as the output target.
    pub fn to_ffmpeg_stdout_args(&self) -> Vec<String> {
        self.to_ffmpeg_args_for_output("pipe:1")
    }

    fn to_ffmpeg_args_for_output(&self, output_target: &str) -> Vec<String> {
        let mut args = Vec::new();

        // Avoid writing banner and stats
        args.push("-hide_banner".into());
        args.push("-loglevel".into());
        args.push("warning".into());

        if let Some(start_time) = self.start_time_ms {
            let start_sec = start_time as f64 / 1000.0;
            args.push("-ss".into());
            args.push(format!("{:.3}", start_sec));
        }

        // Input
        args.push("-i".into());
        args.push(self.source_path.to_string_lossy().into_owned());

        // Copy all streams by default if we don't map explicitly, but for transcode we usually map
        args.push("-map".into());
        args.push("0:v:0?".into()); // First video
        args.push("-map".into());
        args.push("0:a:0?".into()); // First audio

        // Video codec
        args.push("-c:v".into());
        if let Some(vc) = &self.video_codec {
            args.push(vc.clone());
            
            // Add scale filter if we need resizing
            if self.max_width.unwrap_or(0) > 0 || self.max_height.unwrap_or(0) > 0 {
                let w = self.max_width.unwrap_or(u32::MAX);
                let h = self.max_height.unwrap_or(u32::MAX);
                // Simple scale filter that preserves aspect ratio and doesn't up-scale
                args.push("-vf".into());
                args.push(format!("scale=w='min({w}\\,iw)':h='min({h}\\,ih)':force_original_aspect_ratio=decrease"));
            }
        } else {
            args.push("copy".into());
        }

        // Audio codec
        args.push("-c:a".into());
        if let Some(ac) = &self.audio_codec {
            args.push(ac.clone());
        } else {
            args.push("copy".into());
        }

        // Bitrate limits (simple CRF/b:v approach)
        if let Some(bitrate) = self.max_bitrate_kbps {
            if self.video_codec.is_some() {
                args.push("-maxrate".into());
                args.push(format!("{}k", bitrate));
                args.push("-bufsize".into());
                args.push(format!("{}k", bitrate * 2));
            }
        }

        // Container / Format
        args.push("-f".into());
        args.push(self.container.clone());

        // Fast start for mp4
        if self.container == "mp4" {
            // Fragmented MP4 can be consumed while FFmpeg is still producing it.
            args.push("-movflags".into());
            args.push("frag_keyframe+empty_moov+default_base_moof".into());
        }

        // Output path or stdout
        args.push("-y".into());
        args.push(output_target.into());

        args
    }
}

/// Spawns a transcode process and returns it.
pub async fn spawn_transcode(
    _session_id: &str,
    spec: &TranscodeSpec,
    settings: &FfmpegSettings,
) -> Result<Child, std::io::Error> {
    if let Some(parent) = spec.output_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let args = spec.to_ffmpeg_args();
    
    log::info!("Starting FFmpeg: {} {}", settings.ffmpeg_path, args.join(" "));

    let child = Command::new(&settings.ffmpeg_path)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;

    Ok(child)
}

/// Spawns a transcode process that writes a fragmented stream to stdout.
pub async fn spawn_transcode_stdout(
    _session_id: &str,
    spec: &TranscodeSpec,
    settings: &FfmpegSettings,
) -> Result<Child, std::io::Error> {
    let args = spec.to_ffmpeg_stdout_args();

    log::info!("Starting FFmpeg stdout stream: {} {}", settings.ffmpeg_path, args.join(" "));

    let child = Command::new(&settings.ffmpeg_path)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    Ok(child)
}
