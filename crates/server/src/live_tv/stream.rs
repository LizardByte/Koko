//! Browser-compatible live streams and bounded tuner leases released on disconnect.

use super::{
    Channel,
    Source,
    sources,
    storage,
};
use crate::db::DbConn;
use once_cell::sync::Lazy;
use rocket::{
    Request,
    http::ContentType,
    response::{
        self,
        Responder,
        Response,
    },
};
use std::{
    collections::HashMap,
    io,
    pin::Pin,
    process::Stdio,
    sync::Mutex,
    task::{
        Context,
        Poll,
    },
};
use tokio::{
    io::{
        AsyncRead,
        AsyncReadExt,
        ReadBuf,
    },
    process::{
        Child,
        ChildStdout,
        Command,
    },
};

static LEASES: Lazy<Mutex<HashMap<(i32, bool), i32>>> = Lazy::new(|| Mutex::new(HashMap::new()));
/// High-bitrate TV may not reveal audio parameters within FFmpeg's default probe budget.
pub(super) const PROBE_ARGS: [&str; 4] = [
    "-analyzeduration",
    "15000000",
    "-probesize",
    "20000000",
];
/// Serialize capture admission with removal without waiting for long-running guide refreshes.
pub(super) static ADMISSIONS: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub(super) struct Lease(Vec<(i32, bool)>);
/// Source removal shares the admission lock with new captures and waits for existing leases.
pub(super) fn source_in_use(id: i32) -> bool {
    LEASES.lock().map_or(true, |leases| {
        leases.get(&(id, false)).copied().unwrap_or(0) > 0
    })
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Ok(mut leases) = LEASES.lock() {
            for key in &self.0 {
                if let Some(count) = leases.get_mut(key) {
                    *count -= 1;
                    if *count == 0 {
                        leases.remove(key);
                    }
                }
            }
        }
    }
}

pub(super) fn lease(source: &Source) -> Result<Lease, String> {
    reserve(source, false)
}

fn reserve(
    source: &Source,
    bridge: bool,
) -> Result<Lease, String> {
    let mut limits = vec![((source.id, false), source.capacity)];
    if bridge {
        limits.push((
            (source.id, true),
            sources::config(source)?.bridge_capacity.max(1) as i32,
        ));
    }
    let mut leases = LEASES.lock().map_err(|_| "Unable to reserve a tuner")?;
    if limits
        .iter()
        .any(|(key, limit)| leases.get(key).copied().unwrap_or(0) >= *limit)
    {
        return Err(
            "All tuners or allowed IPTV streams are busy. Stop another stream or recording and \
             try again"
                .into(),
        );
    }
    for (key, _) in &limits {
        *leases.entry(*key).or_default() += 1;
    }
    Ok(Lease(limits.into_iter().map(|(key, _)| key).collect()))
}

pub(super) async fn input(
    db: &DbConn,
    id: i32,
) -> Result<(Source, Channel, Lease), String> {
    let _admission = ADMISSIONS.lock().await;
    let (source, channel, bridge, provider_enabled) = db
        .run(move |conn| {
            let channel = storage::channel(conn, id)?;
            let source = storage::source(conn, channel.source_id)?;
            let metadata = super::providers::load(conn, source.id)?;
            let provider_enabled = super::providers::allowed(&source, &channel, &metadata);
            let bridge = metadata
                .channels
                .get(&channel.source_key)
                .is_some_and(|c| c.bridge);
            Ok::<_, diesel::result::Error>((source, channel, bridge, provider_enabled))
        })
        .await
        .map_err(super::db_error)?;
    if !source.enabled || !channel.enabled || !channel.available || !provider_enabled {
        return Err("This channel is disabled or unavailable".into());
    }
    let lease = match reserve(&source, bridge) {
        Ok(lease) => lease,
        Err(error) => {
            if !super::broadcast::preempt(source.id) {
                return Err(error);
            }
            let wait = async {
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    if let Ok(lease) = reserve(&source, bridge) {
                        break lease;
                    }
                }
            };
            tokio::time::timeout(std::time::Duration::from_secs(5), wait)
                .await
                .map_err(|_| error)?
        }
    };
    Ok((source, channel, lease))
}

pub(super) fn command(
    url: &str,
    record: bool,
    local: bool,
) -> Command {
    command_for_service(url, record, local, None)
}

fn command_for_service(
    url: &str,
    record: bool,
    local: bool,
    service: Option<&str>,
) -> Command {
    let mut command = Command::new(crate::config::current_settings().ffmpeg.ffmpeg_path);
    command.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
    ]);
    if !local {
        command.args([
            "-rw_timeout",
            "15000000",
            "-protocol_whitelist",
            "http,https,tcp,tls,crypto,udp,rtp",
        ]);
    }
    command.args(PROBE_ARGS);
    command.args(["-i", url]);
    if let Some(service) = service {
        command.args([
            "-map",
            &format!("0:p:{service}:v:0?"),
            "-map",
            &format!("0:p:{service}:a:0?"),
        ]);
    } else {
        command.args([
            "-map", "0:v:0?", "-map", "0:a:0?",
        ]);
    }
    if record {
        command.args([
            "-c", "copy", "-f", "mpegts", "pipe:1",
        ]);
    } else {
        command.args([
            "-vf",
            "yadif,scale=w='min(1920,iw)':h=-2",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-tune",
            "zerolatency",
            "-crf",
            "23",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-ac",
            "2",
            "-b:a",
            "192k",
            "-movflags",
            "frag_keyframe+empty_moov+default_base_moof",
            "-frag_duration",
            "1000000",
            "-f",
            "mp4",
            "pipe:1",
        ]);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    command
}

struct Reader {
    inner: tokio::io::Chain<std::io::Cursor<Vec<u8>>, ChildStdout>,
    _pipeline: Pipeline,
    _lease: Option<Lease>,
}
impl AsyncRead for Reader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

pub(crate) struct LiveStream(Reader);
impl<'r> Responder<'r, 'static> for LiveStream {
    fn respond_to(
        self,
        _: &'r Request<'_>,
    ) -> response::Result<'static> {
        Response::build()
            .header(ContentType::MP4)
            .raw_header("Cache-Control", "no-store")
            .raw_header("X-Content-Type-Options", "nosniff")
            .streamed_body(self.0)
            .ok()
    }
}

async fn open(
    url: &str,
    lease: Option<Lease>,
    local: bool,
) -> Result<LiveStream, String> {
    let child = command(url, false, local)
        .spawn()
        .map_err(|_| "Unable to start FFmpeg; check the FFmpeg path in Settings")?;
    open_pipeline(make_pipeline(child, None, None, None), lease).await
}

async fn open_pipeline(
    mut pipeline: Pipeline,
    lease: Option<Lease>,
) -> Result<LiveStream, String> {
    let mut stdout = pipeline
        .child
        .stdout
        .take()
        .ok_or("FFmpeg did not open a video stream")?;
    let mut prefix = vec![0; 4096];
    let length = tokio::time::timeout(std::time::Duration::from_secs(25), stdout.read(&mut prefix))
        .await
        .map_err(|_| {
            "The channel did not start in time; check tuner reception and source availability"
        })?
        .map_err(|_| "The video stream could not start")?;
    if length == 0 {
        tokio::task::yield_now().await;
        pipeline.report_error();
        return Err(pipeline.failure_message().unwrap_or_else(|| {
            "The source could not be played; check the channel, tuner, and FFmpeg installation"
                .into()
        }));
    }
    prefix.truncate(length);
    Ok(LiveStream(Reader {
        inner: std::io::Cursor::new(prefix).chain(stdout),
        _pipeline: pipeline,
        _lease: lease,
    }))
}

pub(crate) async fn play_channel(
    db: &DbConn,
    id: i32,
) -> Result<LiveStream, String> {
    let (source, channel, lease) = input(db, id).await?;
    open_pipeline(pipeline(&source, &channel, false).await?, Some(lease)).await
}

#[derive(Default, Clone, Copy)]
pub(super) struct BufferCodecs {
    video_copy: bool,
}

fn buffer_codecs_from_json(value: &serde_json::Value) -> BufferCodecs {
    let streams = value["streams"].as_array();
    let video = streams.and_then(|s| s.iter().find(|s| s["codec_type"] == "video"));
    BufferCodecs {
        video_copy: video.is_some_and(|s| {
            s["codec_name"] == "h264"
                && s["pix_fmt"] == "yuv420p"
                && s["field_order"] == "progressive"
        }),
    }
}

pub(super) async fn buffer_codecs(path: &std::path::Path) -> BufferCodecs {
    let mut command = Command::new(crate::config::current_settings().ffmpeg.ffprobe_path);
    command
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type,codec_name,pix_fmt,field_order,channels",
            "-of",
            "json",
        ])
        .arg(path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    match tokio::time::timeout(std::time::Duration::from_secs(5), command.output()).await {
        Ok(Ok(output)) if output.status.success() => serde_json::from_slice(&output.stdout)
            .map(|value| buffer_codecs_from_json(&value))
            .unwrap_or_default(),
        _ => BufferCodecs::default(),
    }
}

pub(super) async fn play_buffer(
    url: &str,
    index: usize,
    offset_ms: u64,
    quality: &str,
    codecs: BufferCodecs,
) -> Result<LiveStream, String> {
    log::debug!(
        "TV buffer playback: segment={index}, offset_ms={offset_ms}, quality={quality}, \
         compatible_video={}",
        codecs.video_copy
    );
    let target = match quality {
        "original" => None,
        "4k" => Some((3840, 2160, 24808)),
        "1080p" => Some((1920, 1080, 7808)),
        "720p" => Some((1280, 720, 3808)),
        "480p" => Some((854, 480, 1308)),
        _ => return Err("Choose a supported TV playback quality".into()),
    };
    let base = command(url, false, false);
    let mut arguments = base
        .as_std()
        .get_args()
        .map(|s| s.to_os_string())
        .collect::<Vec<_>>();
    let input = arguments
        .iter()
        .position(|s| s == "-i")
        .expect("FFmpeg input");
    arguments.splice(
        input..input,
        [
            "-live_start_index".into(),
            index.to_string().into(),
        ],
    );
    let output = arguments.len() - 1;
    if offset_ms > 0 {
        arguments.splice(
            output..output,
            [
                "-ss".into(),
                format!("{:.3}", offset_ms as f64 / 1000.0).into(),
            ],
        );
    }
    if let Some(filter) = arguments.iter().position(|s| s == "-vf") {
        arguments[filter + 1] = target
            .map_or_else(
                || "yadif=deint=interlaced,scale=trunc(iw/2)*2:trunc(ih/2)*2".into(),
                |(width, height, _)| {
                    format!(
                        "yadif=deint=interlaced,scale=w='min({width},iw)':h='min({height},ih)':\
                         force_original_aspect_ratio=decrease:force_divisible_by=2"
                    )
                },
            )
            .into();
    }
    // Keep audio conversion so fragmented MP4 has AAC decoder configuration in
    // its initialization segment, even when the transport stream has no extradata.
    if quality == "original" && codecs.video_copy && offset_ms == 0 {
        for option in [
            "-vf", "-preset", "-tune", "-crf", "-pix_fmt",
        ] {
            if let Some(index) = arguments.iter().position(|s| s == option) {
                arguments.drain(index..index + 2);
            }
        }
        if let Some(index) = arguments.iter().position(|s| s == "-c:v") {
            arguments[index + 1] = "copy".into();
        }
    }
    if let Some((_, _, bitrate)) = target {
        let output = arguments.len() - 1;
        arguments.splice(
            output..output,
            [
                "-maxrate".into(),
                format!("{bitrate}k").into(),
                "-bufsize".into(),
                format!("{}k", bitrate * 2).into(),
            ],
        );
    }
    let mut command = Command::new(base.as_std().get_program());
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let child = command
        .spawn()
        .map_err(|_| "Unable to start buffered TV playback; check FFmpeg")?;
    open_pipeline(make_pipeline(child, None, None, None), None).await
}

pub(super) struct Pipeline {
    pub child: Child,
    _tuner: Option<Child>,
    _bridge: Option<tokio::task::JoinHandle<()>>,
    _proxy: Option<super::proxy::Proxy>,
    log: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    log_reader: tokio::task::JoinHandle<()>,
}
impl Drop for Pipeline {
    fn drop(&mut self) {
        if let Some(bridge) = &self._bridge {
            bridge.abort();
        }
        self.log_reader.abort();
    }
}

impl Pipeline {
    pub(super) fn new(child: Child) -> Self {
        make_pipeline(child, None, None, None)
    }

    pub(super) fn report_error(&self) {
        if let Ok(data) = self.log.lock() {
            let message = String::from_utf8_lossy(&data);
            static URL: Lazy<regex::Regex> =
                Lazy::new(|| regex::Regex::new(r"https?://[^\s]+").unwrap());
            log::warn!(
                "Live TV FFmpeg failed: {}",
                URL.replace_all(message.trim(), "[stream]")
            );
        }
    }

    /// Report actionable causes without exposing upstream addresses, credentials, or raw logs.
    pub(super) fn failure_message(&self) -> Option<String> {
        self.log
            .lock()
            .ok()
            .and_then(|data| playback_failure(&data))
    }
}

fn playback_failure(data: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(data);
    static HTTP_ERROR: Lazy<regex::Regex> = Lazy::new(|| {
        regex::Regex::new(r"(?i)(?:HTTP error|Server returned)\s+([45]\d{2})\b").unwrap()
    });
    if let Some(code) = HTTP_ERROR
        .captures(&text)
        .and_then(|capture| capture.get(1))
    {
        let code = code.as_str();
        let action = match code {
            "401" | "403" => {
                "Check the provider sign-in and access to this channel in the source app"
            }
            "404" | "410" => "Check that this channel plays in the source app",
            "429" => "Wait before trying this channel again",
            _ => "Check the source app and try this channel again",
        };
        return Some(format!("The TV source returned HTTP {code}. {action}."));
    }
    if text.contains("No space left on device") {
        return Some(
            "TV buffer storage is full. Free space in the DVR save folder and retry.".into(),
        );
    }
    if text.contains("Permission denied") || text.contains("Read-only file system") {
        return Some(
            "Koko could not access the TV files. Check the DVR save folder permissions.".into(),
        );
    }
    None
}

fn make_pipeline(
    mut child: Child,
    tuner: Option<Child>,
    bridge: Option<tokio::task::JoinHandle<()>>,
    proxy: Option<super::proxy::Proxy>,
) -> Pipeline {
    let mut stderr = child.stderr.take();
    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let output = log.clone();
    let log_reader = tokio::spawn(async move {
        let Some(ref mut stderr) = stderr else {
            return;
        };
        let mut chunk = [0; 1024];
        while let Ok(size) = stderr.read(&mut chunk).await {
            if size == 0 {
                break;
            }
            if let Ok(mut data) = output.lock() {
                data.extend_from_slice(&chunk[..size]);
                let excess = data.len().saturating_sub(8192);
                data.drain(..excess);
            }
        }
    });
    Pipeline {
        child,
        _tuner: tuner,
        _bridge: bridge,
        _proxy: proxy,
        log,
        log_reader,
    }
}

pub(super) async fn pipeline(
    source: &Source,
    channel: &Channel,
    record: bool,
) -> Result<Pipeline, String> {
    if source.kind == "native" {
        let config = sources::config(source)?;
        let mut input = super::tsduck::tune(&config, channel).await?;
        let service = channel
            .source_key
            .rsplit(':')
            .next()
            .ok_or("Invalid tuner service")?;
        let mut child = command_for_service("pipe:0", record, true, Some(service))
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|_| "Unable to start FFmpeg")?;
        let mut sink = child
            .stdin
            .take()
            .ok_or("Unable to open the tuner video stream")?;
        let bridge = tokio::spawn(async move {
            let _ = tokio::io::copy(&mut input, &mut sink).await;
        });
        return Ok(make_pipeline(child, None, Some(bridge), None));
    }
    if source.kind == "dvb" {
        let config = sources::config(source)?;
        let mut tuner = super::native::tune(&config, channel, false)?;
        let mut input = tuner.stdout.take().ok_or("Unable to read the DVB tuner")?;
        let mut child = command("pipe:0", record, true)
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|_| "Unable to start FFmpeg")?;
        let mut sink = child
            .stdin
            .take()
            .ok_or("Unable to open the tuner video stream")?;
        let bridge = tokio::spawn(async move {
            let _ = tokio::io::copy(&mut input, &mut sink).await;
        });
        Ok(make_pipeline(child, Some(tuner), Some(bridge), None))
    } else {
        let upstream = sources::stream_url(source, channel).await?;
        let (proxy, url) = super::proxy::start(&upstream).await?;
        let child = command(&url, record, false)
            .spawn()
            .map_err(|_| "Unable to start FFmpeg; check its path in Settings")?;
        let keepalive = sources::keepalive(source, &upstream)?;
        Ok(make_pipeline(child, None, keepalive, Some(proxy)))
    }
}

pub(super) fn recording_path(id: i32) -> std::path::PathBuf {
    std::path::Path::new(&crate::config::current_settings().general.data_dir)
        .join("live-tv")
        .join("recordings")
        .join(format!("recording-{id}.ts"))
}

pub(crate) async fn play_recording(
    db: &DbConn,
    id: i32,
    user: i32,
) -> Result<LiveStream, String> {
    use crate::db::schema::live_tv_recordings as r;
    use diesel::prelude::*;
    let allowed = db
        .run(move |conn| {
            r::table
                .find(id)
                .filter(r::user_id.eq(user).or(r::shared.eq(true)))
                .filter(r::state.eq_any([
                    "completed",
                    "interrupted",
                    "failed",
                    "cancelled",
                ]))
                .select(r::file_name)
                .first::<Option<String>>(conn)
        })
        .await
        .map_err(|_| "Recording not found")?;
    let Some(saved) = allowed else {
        return Err("This recording has no saved video".into());
    };
    let path = super::settings::saved_path(id, &saved);
    if !path.is_file() {
        return Err("The recording file is missing".into());
    }
    open(&path.to_string_lossy(), None, true).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn playback_failures_distinguish_source_http_errors_from_storage_errors() {
        for code in [
            401, 403, 404, 410, 429, 500, 503,
        ] {
            for error in [
                format!("Server returned {code} Not Found"),
                format!("HTTP error {code} Upstream"),
            ] {
                let private = "https://private.example/?token=private-token";
                let data = format!("{error}\nError opening input file {private}");
                let message = playback_failure(data.as_bytes()).unwrap();
                assert!(message.contains(&format!("HTTP {code}")));
                assert!(!message.contains("private") && !message.contains("disk"));
            }
        }
        assert!(
            playback_failure(b"No space left on device")
                .unwrap()
                .contains("storage is full")
        );
        assert!(
            playback_failure(b"Read-only file system")
                .unwrap()
                .contains("permissions")
        );
        assert!(
            playback_failure(b"Permission denied")
                .unwrap()
                .contains("permissions")
        );
        assert!(playback_failure(b"Invalid data found when processing input").is_none());
        assert!(playback_failure(b"frame 404, HTTP connection closed").is_none());
    }
    #[test]
    fn all_bridged_providers_share_one_capture_limit_and_release_it_on_disconnect() {
        let source = Source {
            id: 910001,
            name: "IPTV".into(),
            kind: "m3u".into(),
            config_json: serde_json::json!({"bridge_capacity":1}).to_string(),
            enabled: true,
            capacity: 3,
            next_refresh: 0,
            last_error: None,
        };
        let first = reserve(&source, true).unwrap();
        assert!(reserve(&source, true).is_err());
        let direct = lease(&source).unwrap();
        drop(first);
        let second = reserve(&source, true).unwrap();
        drop(second);
        drop(direct);
        assert!(
            !LEASES
                .lock()
                .unwrap()
                .keys()
                .any(|(id, _)| *id == source.id)
        );
    }
    #[test]
    fn only_browser_compatible_progressive_video_uses_original_passthrough() {
        let compatible = serde_json::json!({"streams":[
            {"codec_type":"video", "codec_name":"h264", "pix_fmt":"yuv420p",
             "field_order":"progressive"},
            {"codec_type":"audio", "codec_name":"aac", "channels":2}
        ]});
        let result = buffer_codecs_from_json(&compatible);
        assert!(result.video_copy);
        for (field, value) in [
            ("codec_name", "mpeg2video"),
            ("field_order", "tt"),
            ("pix_fmt", "yuv420p10le"),
        ] {
            let mut value_json = compatible.clone();
            value_json["streams"][0][field] = value.into();
            assert!(!buffer_codecs_from_json(&value_json).video_copy);
        }
        assert!(!buffer_codecs_from_json(&serde_json::json!({})).video_copy);
    }
}
