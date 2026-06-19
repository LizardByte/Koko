//! Transcoding engine for media playback.

// standard imports
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};

// lib imports
use tokio::fs;
use tokio::process::Child;
use tokio::process::Command;

// local imports
use crate::config::FfmpegSettings;

use rocket::http::Status;

/// A structured error from attempting to spawn a transcode.
#[derive(Debug)]
pub enum SpawnTranscodeError {
    /// ffmpeg could not be resolved/executed. `checked_paths` lists where we looked.
    ExecutableMissing {
        /// Every location the resolver inspected, for diagnostics.
        checked_paths: Vec<PathBuf>,
    },
    /// ffmpeg started but reported its input was unusable. Produced by the
    /// deferred lifecycle phase; declared here so the shared mapper is complete.
    BadInput {
        /// The captured stderr from ffmpeg explaining the input failure.
        ffmpeg_stderr: String,
    },
    /// Any other spawn-time I/O error.
    Io(std::io::Error),
}

impl From<std::io::Error> for SpawnTranscodeError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl std::fmt::Display for SpawnTranscodeError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            SpawnTranscodeError::ExecutableMissing { checked_paths } => {
                let checked = checked_paths
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "FFmpeg could not be found. Checked: [{checked}]")
            }
            SpawnTranscodeError::BadInput { ffmpeg_stderr } => {
                write!(
                    f,
                    "FFmpeg could not read the source media. {}",
                    ffmpeg_stderr.trim()
                )
            }
            SpawnTranscodeError::Io(error) => {
                write!(f, "Transcode failed to start: {error}")
            }
        }
    }
}

impl std::error::Error for SpawnTranscodeError {}

/// The JSON body returned to clients when a transcode fails. The `action`
/// field lets the UI decide whether to show an actionable control (e.g.
/// "Open settings") for this kind of failure.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TranscodeErrorBody {
    /// Stable machine code: `transcode_executable_missing` | `transcode_input_error` | `transcode_failed`.
    pub code: &'static str,
    /// Human-readable explanation.
    pub message: String,
    /// Optional UI action hint, e.g. `Some("open_settings")`.
    pub action: Option<&'static str>,
}

/// Map a [`SpawnTranscodeError`] to an HTTP status + body. This is the single
/// error-shaping function reused by the route handler today and the lifecycle
/// watcher in a later phase.
pub fn map_transcode_error(error: SpawnTranscodeError) -> (Status, TranscodeErrorBody) {
    match error {
        SpawnTranscodeError::ExecutableMissing { checked_paths } => {
            let checked = checked_paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            (
                Status::ServiceUnavailable,
                TranscodeErrorBody {
                    code: "transcode_executable_missing",
                    message: format!(
                        "FFmpeg could not be found. Install it or set its path in Settings. \
                         Checked: [{checked}]"
                    ),
                    action: Some("open_settings"),
                },
            )
        }
        SpawnTranscodeError::BadInput { ffmpeg_stderr } => (
            Status::UnprocessableEntity,
            TranscodeErrorBody {
                code: "transcode_input_error",
                message: format!(
                    "FFmpeg could not read the source media. {}",
                    ffmpeg_stderr.trim()
                ),
                action: None,
            },
        ),
        SpawnTranscodeError::Io(error) => (
            Status::InternalServerError,
            TranscodeErrorBody {
                code: "transcode_failed",
                message: format!("Transcode failed to start: {error}"),
                action: None,
            },
        ),
    }
}

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
    /// Zero-based audio stream index among audio streams.
    pub audio_stream_index: Option<usize>,
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

    fn to_ffmpeg_args_for_output(
        &self,
        output_target: &str,
    ) -> Vec<String> {
        // Avoid writing banner and stats
        let mut args = vec![
            "-hide_banner".into(),
            "-loglevel".into(),
            "warning".into(),
            "-fflags".into(),
            "+genpts".into(),
        ];

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
        args.push(format!("0:a:{}?", self.audio_stream_index.unwrap_or(0)));

        // Video codec
        args.push("-c:v".into());
        if let Some(vc) = &self.video_codec {
            args.push(vc.clone());
            if vc == "libx264" {
                args.push("-preset".into());
                args.push("veryfast".into());
                args.push("-pix_fmt".into());
                args.push("yuv420p".into());
            }

            // Add scale filter if we need resizing
            if self.max_width.unwrap_or(0) > 0 || self.max_height.unwrap_or(0) > 0 {
                let w = self.max_width.unwrap_or(u32::MAX);
                let h = self.max_height.unwrap_or(u32::MAX);
                // Simple scale filter that preserves aspect ratio and doesn't up-scale
                args.push("-vf".into());
                args.push(format!(
                    "scale=w='min({w}\\,iw)':h='min({h}\\,ih)':\
                     force_original_aspect_ratio=decrease"
                ));
            }
        } else {
            args.push("copy".into());
        }

        // Audio codec
        args.push("-c:a".into());
        if let Some(ac) = &self.audio_codec {
            args.push(ac.clone());
            if ac == "aac" {
                args.push("-ac".into());
                args.push("2".into());
                args.push("-b:a".into());
                args.push("192k".into());
            }
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
            args.push("-avoid_negative_ts".into());
            args.push("make_zero".into());
            args.push("-muxdelay".into());
            args.push("0".into());
            args.push("-muxpreload".into());
            args.push("0".into());
        }

        // Output path or stdout
        args.push("-y".into());
        args.push(output_target.into());

        args
    }
}

/// Resolve the configured ffmpeg path to an absolute, verified executable,
/// logging the resolution. Returns [`SpawnTranscodeError::ExecutableMissing`]
/// when it cannot be found — this is the actual fix for issue 1 (a bare
/// `ffmpeg` lookup fails in GUI-launched processes that don't inherit the
/// user's shell PATH).
fn resolve_ffmpeg_for_spawn(
    settings: &FfmpegSettings,
    args_label: &str,
) -> Result<PathBuf, SpawnTranscodeError> {
    match crate::ffmpeg_resolve::resolve_ffmpeg(&settings.ffmpeg_path) {
        crate::ffmpeg_resolve::ResolvedBinary::Found { resolved_path, .. } => {
            log::info!("Starting FFmpeg {args_label}: {}", resolved_path.display());
            Ok(resolved_path)
        }
        crate::ffmpeg_resolve::ResolvedBinary::Missing { checked_paths, .. } => {
            Err(SpawnTranscodeError::ExecutableMissing { checked_paths })
        }
    }
}

/// Spawns a transcode process and returns it.
pub async fn spawn_transcode(
    _session_id: &str,
    spec: &TranscodeSpec,
    settings: &FfmpegSettings,
) -> Result<Child, SpawnTranscodeError> {
    if let Some(parent) = spec.output_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let args = spec.to_ffmpeg_args();
    let ffmpeg_path = resolve_ffmpeg_for_spawn(settings, &args.join(" "))?;

    let mut command = Command::new(&ffmpeg_path);
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let child = command.spawn()?;

    Ok(child)
}

/// Spawns a transcode process that writes a fragmented stream to stdout.
pub async fn spawn_transcode_stdout(
    _session_id: &str,
    spec: &TranscodeSpec,
    settings: &FfmpegSettings,
) -> Result<Child, SpawnTranscodeError> {
    let args = spec.to_ffmpeg_stdout_args();
    let ffmpeg_path = resolve_ffmpeg_for_spawn(settings, "stdout stream")?;

    log::info!("FFmpeg stdout args: {}", args.join(" "));

    let mut command = Command::new(&ffmpeg_path);
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = command.spawn()?;

    Ok(child)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn executable_missing_maps_to_open_settings_action() {
        let err = SpawnTranscodeError::ExecutableMissing {
            checked_paths: vec![PathBuf::from(
                "/usr/bin/ffmpeg",
            )],
        };
        let (status, body) = map_transcode_error(err);
        assert_eq!(status, Status::ServiceUnavailable);
        assert_eq!(body.code, "transcode_executable_missing");
        assert_eq!(body.action, Some("open_settings"));
        assert!(body.message.contains("ffmpeg"), "{}", body.message);
    }

    #[test]
    fn io_error_maps_to_failed_with_no_action() {
        let err = SpawnTranscodeError::Io(std::io::Error::other("boom"));
        let (status, body) = map_transcode_error(err);
        assert_eq!(status, Status::InternalServerError);
        assert_eq!(body.code, "transcode_failed");
        assert_eq!(body.action, None);
    }

    #[test]
    fn bad_input_maps_to_input_error() {
        let err = SpawnTranscodeError::BadInput {
            ffmpeg_stderr: "No such file".into(),
        };
        let (status, body) = map_transcode_error(err);
        assert_eq!(status, Status::UnprocessableEntity);
        assert_eq!(body.code, "transcode_input_error");
        assert_eq!(body.action, None);
        assert!(body.message.contains("No such file"));
    }

    #[test]
    fn io_error_conversion_wraps_in_variant() {
        let io_err = std::io::Error::other("disk full");
        let wrapped: SpawnTranscodeError = io_err.into();
        assert!(matches!(wrapped, SpawnTranscodeError::Io(_)));
    }
}
