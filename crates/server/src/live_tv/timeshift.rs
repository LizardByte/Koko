//! Temporary, owner-scoped recording buffers, with capture independent of playback and quality.
use super::{
    settings,
    storage,
    stream,
};
use crate::db::{
    DbConn,
    schema::{
        live_tv_programs as p,
        live_tv_recordings as r,
    },
};
use diesel::prelude::*;
use once_cell::sync::Lazy;
use rocket_okapi::okapi::schemars::{
    self,
    JsonSchema,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::PathBuf,
    process::Stdio,
    sync::{
        Arc,
        Mutex,
        atomic::{
            AtomicBool,
            Ordering,
        },
    },
    time::{
        Duration,
        Instant,
    },
};
use tokio::{
    io::AsyncWriteExt,
    process::Command,
};

/// File database used by independent capture tasks; it does not retain a Rocket pool guard.
pub(crate) struct BufferDatabase(pub String);
static SESSIONS: Lazy<Mutex<HashMap<String, Arc<Session>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
const IDLE_SECONDS: u64 = 90;

#[derive(Clone, Debug)]
struct Segment {
    name: String,
    timestamp_ms: i64,
    duration_ms: u64,
}
#[derive(Default)]
struct Stats {
    segments: Vec<Segment>,
    origin_ms: Option<i64>,
    error: Option<String>,
    finished: bool,
}
struct Kept {
    id: i32,
    path: PathBuf,
    deadline: i64,
    last_segment: Option<String>,
    from_ms: i64,
    done: bool,
}
struct Session {
    id: String,
    owner: i32,
    channel: i32,
    title: String,
    directory: PathBuf,
    files: super::buffer_files::FileServer,
    codecs: tokio::sync::OnceCell<stream::BufferCodecs>,
    database: String,
    last_access: Mutex<Instant>,
    stats: Mutex<Stats>,
    kept: Mutex<Option<Kept>>,
    keep_gate: tokio::sync::Mutex<()>,
    closed: AtomicBool,
    finished: AtomicBool,
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct BufferInfo {
    pub session_id: String,
    pub channel_id: i32,
    pub title: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub recording_id: Option<i32>,
    pub finished: bool,
    pub error: Option<String>,
    pub can_keep: bool,
}

fn session(
    id: &str,
    owner: i32,
) -> Result<Arc<Session>, String> {
    SESSIONS
        .lock()
        .map_err(|_| "TV buffers are unavailable")?
        .get(id)
        .filter(|s| s.owner == owner && !s.closed.load(Ordering::Relaxed))
        .cloned()
        .ok_or_else(|| "TV buffer not found".into())
}

fn parse_playlist(value: &str) -> Vec<Segment> {
    let mut timestamp = None;
    let mut duration = None;
    let mut result = Vec::new();
    for line in value.lines().map(str::trim) {
        if let Some(value) = line.strip_prefix("#EXT-X-PROGRAM-DATE-TIME:") {
            timestamp = chrono::DateTime::parse_from_rfc3339(value)
                .or_else(|_| chrono::DateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f%z"))
                .ok()
                .map(|v| v.timestamp_millis());
        } else if let Some(value) = line.strip_prefix("#EXTINF:") {
            duration = value
                .split(',')
                .next()
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v > 0.0)
                .map(|v| (v * 1000.0).round() as u64);
        } else if !line.starts_with('#') && super::buffer_files::valid_segment_name(line) {
            if let (Some(timestamp_ms), Some(duration_ms)) = (timestamp, duration.take()) {
                result.push(Segment {
                    name: line.into(),
                    timestamp_ms,
                    duration_ms,
                });
                timestamp = Some(timestamp_ms.saturating_add(duration_ms as i64));
            }
        }
    }
    result
}

async fn refresh_segments(session: &Session) {
    let Ok(playlist) = tokio::fs::read_to_string(session.directory.join("buffer.m3u8")).await
    else {
        return;
    };
    let segments = parse_playlist(&playlist);
    if let Ok(mut stats) = session.stats.lock() {
        if stats.origin_ms.is_none() {
            stats.origin_ms = segments.first().map(|s| s.timestamp_ms);
        }
        stats.segments = segments;
    }
}

fn info(
    session: &Session,
    can_keep: bool,
) -> BufferInfo {
    let stats = session.stats.lock().unwrap();
    let origin = stats.origin_ms.unwrap_or(0);
    let start_ms = stats
        .segments
        .first()
        .map_or(0, |s| s.timestamp_ms.saturating_sub(origin).max(0) as u64);
    let end_ms = stats.segments.last().map_or(0, |s| {
        s.timestamp_ms.saturating_sub(origin).max(0) as u64 + s.duration_ms
    });
    BufferInfo {
        session_id: session.id.clone(),
        channel_id: session.channel,
        title: session.title.clone(),
        start_ms,
        end_ms,
        recording_id: session
            .kept
            .lock()
            .unwrap()
            .as_ref()
            .filter(|k| !k.done)
            .map(|k| k.id),
        finished: stats.finished,
        error: stats.error.clone(),
        can_keep,
    }
}

fn recording_state(
    database: &str,
    id: i32,
    update: Option<&str>,
) -> Result<String, String> {
    let mut conn =
        SqliteConnection::establish(database).map_err(|_| "Recording database is unavailable")?;
    crate::db::configure_sqlite_connection(&mut conn).map_err(super::db_error)?;
    if let Some(state) = update {
        diesel::update(r::table.find(id))
            .set(r::state.eq(state))
            .execute(&mut conn)
            .map_err(super::db_error)?;
    }
    r::table
        .find(id)
        .select(r::state)
        .first(&mut conn)
        .map_err(super::db_error)
}

async fn copy_kept_segments(session: &Session) -> Result<(), String> {
    let (path, last, from_ms, deadline) = match session.kept.lock().unwrap().as_ref() {
        Some(kept) if !kept.done => (
            kept.path.clone(),
            kept.last_segment.clone(),
            kept.from_ms,
            kept.deadline,
        ),
        _ => return Ok(()),
    };
    let segments = session.stats.lock().unwrap().segments.clone();
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
        .map_err(|_| "Unable to save the buffered recording; check its folder and free space")?;
    for segment in segments.iter().filter(|s| {
        last.as_ref().is_none_or(|last| s.name > *last)
            && s.timestamp_ms.saturating_add(s.duration_ms as i64) > from_ms
            && s.timestamp_ms < deadline * 1000
    }) {
        let mut input = tokio::fs::File::open(session.directory.join(&segment.name))
            .await
            .map_err(|_| "A buffered recording segment is unavailable")?;
        tokio::io::copy(&mut input, &mut file)
            .await
            .map_err(|_| "Unable to write the buffered recording; check free space")?;
        file.flush()
            .await
            .map_err(|_| "Unable to flush the buffered recording")?;
        if let Some(kept) = session.kept.lock().unwrap().as_mut() {
            kept.last_segment = Some(segment.name.clone());
        }
    }
    Ok(())
}

async fn remove_buffer(directory: &std::path::Path) {
    // Only the server-created session directory is accepted here.
    for _ in 0..20 {
        match tokio::fs::remove_dir_all(directory).await {
            Ok(()) => return,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
}

async fn prune_abandoned_buffers(root: &std::path::Path) {
    let Ok(mut entries) = tokio::fs::read_dir(root).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.len() != 32
            || !name.bytes().all(|b| b.is_ascii_hexdigit())
            || SESSIONS.lock().unwrap().contains_key(&name)
        {
            continue;
        }
        let Ok(kind) = entry.file_type().await else {
            continue;
        };
        if !kind.is_dir() || kind.is_symlink() {
            continue;
        }
        let stale = entry
            .metadata()
            .await
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|time| time.elapsed().ok())
            .is_some_and(|age| age > Duration::from_secs(86400));
        if stale {
            remove_buffer(&entry.path()).await;
        }
    }
}

async fn finish_kept(
    session: &Session,
    ending: &'static str,
) -> Result<(), String> {
    let id = session
        .kept
        .lock()
        .unwrap()
        .as_ref()
        .filter(|k| !k.done)
        .map(|k| k.id);
    let Some(id) = id else {
        return Ok(());
    };
    let database = session.database.clone();
    tokio::task::spawn_blocking(move || {
        let current = recording_state(&database, id, None)?;
        if current == "recording" {
            recording_state(&database, id, Some(ending))?;
        } else if current == "stopping" {
            recording_state(&database, id, Some("cancelled"))?;
        }
        Ok::<_, String>(())
    })
    .await
    .map_err(|_| "Recording finalization stopped")??;
    if let Some(kept) = session.kept.lock().unwrap().as_mut() {
        if kept.id == id {
            kept.done = true;
        }
    }
    Ok(())
}

fn recording_end_reached(
    segments: &[Segment],
    now: i64,
    deadline: i64,
) -> bool {
    // A segment containing the end of the show can finish after the wall-clock
    // deadline. Give it time to arrive, without retaining a stalled tuner forever.
    now >= deadline
        && (segments.last().is_some_and(|s| {
            s.timestamp_ms.saturating_add(s.duration_ms as i64) >= deadline.saturating_mul(1000)
        }) || now >= deadline.saturating_add(30))
}

async fn capture(
    session: Arc<Session>,
    mut pipeline: stream::Pipeline,
    lease: stream::Lease,
) {
    let result = capture_inner(&session, &mut pipeline).await;
    let _ = pipeline.child.kill().await;
    drop(pipeline);
    drop(lease);
    {
        let mut stats = session.stats.lock().unwrap();
        stats.finished = true;
        if let Err(message) = result {
            stats.error = Some(message);
        }
    }
    let failed = session.stats.lock().unwrap().error.is_some();
    let _ = finish_kept(&session, if failed { "failed" } else { "completed" }).await;
    session.finished.store(true, Ordering::Release);
    while !session.closed.load(Ordering::Relaxed)
        && session.last_access.lock().unwrap().elapsed() < Duration::from_secs(IDLE_SECONDS)
    {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    SESSIONS.lock().unwrap().remove(&session.id);
    remove_buffer(&session.directory).await;
}

async fn capture_inner(
    session: &Session,
    pipeline: &mut stream::Pipeline,
) -> Result<(), String> {
    let output = pipeline
        .child
        .stdout
        .take()
        .ok_or("TV capture did not open an output")?;
    let mut command = Command::new(crate::config::current_settings().ffmpeg.ffmpeg_path);
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-fflags",
            "+genpts",
            "-i",
            "pipe:0",
            "-map",
            "0:v:0?",
            "-map",
            "0:a:0?",
            "-c",
            "copy",
            "-f",
            "hls",
            "-hls_time",
            "2",
            "-hls_list_size",
            "1800",
            "-hls_delete_threshold",
            "4",
            "-hls_flags",
            "delete_segments+program_date_time+temp_file",
            "-hls_segment_filename",
        ])
        .arg(session.directory.join("segment-%010d.ts"))
        .arg(session.directory.join("buffer.m3u8"))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut segmenter = stream::Pipeline::new(
        command
            .spawn()
            .map_err(|_| "Unable to start the TV buffer; check FFmpeg")?,
    );
    let mut input = segmenter
        .child
        .stdin
        .take()
        .ok_or("TV buffer did not open an input")?;
    let pump = tokio::spawn(async move {
        let mut output = output;
        tokio::io::copy(&mut output, &mut input).await
    });
    let mut ticks = tokio::time::interval(Duration::from_millis(250));
    let mut check = Instant::now();
    let mut failure = None;
    loop {
        ticks.tick().await;
        refresh_segments(session).await;
        if let Err(error) = copy_kept_segments(session).await {
            failure = Some(error);
            break;
        }
        let kept = session
            .kept
            .lock()
            .unwrap()
            .as_ref()
            .filter(|k| !k.done)
            .map(|k| (k.id, k.deadline));
        let closed = session.closed.load(Ordering::Relaxed)
            || session.last_access.lock().unwrap().elapsed() > Duration::from_secs(IDLE_SECONDS);
        if kept.is_none() && closed {
            session.closed.store(true, Ordering::Relaxed);
            break;
        }
        if let Some((id, deadline)) = kept {
            let complete = recording_end_reached(
                &session.stats.lock().unwrap().segments,
                super::now(),
                deadline,
            );
            if complete {
                finish_kept(session, "completed").await?;
                continue;
            }
            if check.elapsed() >= Duration::from_secs(2) {
                check = Instant::now();
                let database = session.database.clone();
                let state =
                    tokio::task::spawn_blocking(move || recording_state(&database, id, None))
                        .await
                        .map_err(|_| "Recording status lookup stopped")??;
                if state != "recording" {
                    finish_kept(session, "cancelled").await?;
                }
            }
        }
        if let Some(status) = segmenter
            .child
            .try_wait()
            .map_err(|_| "Unable to check the TV buffer")?
        {
            if !status.success() {
                tokio::task::yield_now().await;
                pipeline.report_error();
                segmenter.report_error();
                failure =
                    Some("TV buffering stopped; check the source and available disk space".into());
            }
            break;
        }
    }
    let _ = pipeline.child.kill().await;
    if tokio::time::timeout(Duration::from_secs(3), async {
        while !pump.is_finished() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .is_err()
    {
        pump.abort();
    }
    let _ = tokio::time::timeout(Duration::from_secs(3), segmenter.child.wait()).await;
    refresh_segments(session).await;
    copy_kept_segments(session).await?;
    failure.map_or(Ok(()), Err)
}

pub(crate) async fn start(
    db: &DbConn,
    database: &str,
    owner: i32,
    channel_id: i32,
) -> Result<BufferInfo, String> {
    if SESSIONS.lock().unwrap().len() >= 64 {
        return Err("The server has too many TV buffers open".into());
    }
    let (source, channel, lease) = stream::input(db, channel_id).await?;
    let id = format!("{:032x}", rand::random::<u128>());
    let settings = db.run(settings::load).await.map_err(super::db_error)?;
    let root = if settings.recording_dir.is_empty() {
        stream::recording_path(0).parent().unwrap().to_owned()
    } else {
        PathBuf::from(settings.recording_dir)
    };
    let buffers = root.join(".buffers");
    prune_abandoned_buffers(&buffers).await;
    let directory = buffers.join(&id);
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|_| "Unable to create the TV buffer folder; check DVR storage")?;
    let pipeline = match stream::pipeline(&source, &channel, true).await {
        Ok(p) => p,
        Err(e) => {
            let _ = tokio::fs::remove_dir_all(&directory).await;
            return Err(e);
        }
    };
    let files = match super::buffer_files::FileServer::start(directory.clone(), &id).await {
        Ok(files) => files,
        Err(error) => {
            drop(pipeline);
            remove_buffer(&directory).await;
            return Err(error);
        }
    };
    let session = Arc::new(Session {
        id: id.clone(),
        owner,
        channel: channel_id,
        title: channel.name,
        directory,
        files,
        codecs: tokio::sync::OnceCell::new(),
        database: database.into(),
        last_access: Mutex::new(Instant::now()),
        stats: Mutex::new(Stats::default()),
        kept: Mutex::new(None),
        keep_gate: tokio::sync::Mutex::new(()),
        closed: AtomicBool::new(false),
        finished: AtomicBool::new(false),
    });
    SESSIONS.lock().unwrap().insert(id.clone(), session.clone());
    tokio::spawn(capture(session.clone(), pipeline, lease));
    let ready = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let info = info(&session, true);
            if let Some(error) = info.error {
                return Err(error);
            }
            if info.end_ms.saturating_sub(info.start_ms) >= 6000
                || (info.finished && info.end_ms > info.start_ms)
            {
                return Ok(());
            }
            if info.finished {
                return Err("The channel ended before its TV buffer was ready".into());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    match ready {
        Ok(Ok(())) => {
            let can_keep = db
                .run(move |conn| settings::can_record(conn, owner))
                .await
                .map_err(super::db_error)?;
            Ok(info(&session, can_keep))
        }
        value => {
            let _ = close(&id, owner).await;
            Err(value
                .unwrap_or_else(|_| {
                    Err("TV buffering timed out; check the source and FFmpeg".into())
                })
                .unwrap_err())
        }
    }
}

pub(crate) async fn status(
    db: &DbConn,
    id: &str,
    owner: i32,
) -> Result<BufferInfo, String> {
    let session = session(id, owner)?;
    *session.last_access.lock().unwrap() = Instant::now();
    let allowed = db
        .run(move |conn| settings::can_record(conn, owner))
        .await
        .map_err(super::db_error)?;
    Ok(info(&session, allowed))
}

pub(crate) async fn close(
    id: &str,
    owner: i32,
) -> Result<(), String> {
    let session = session(id, owner)?;
    let _keep_gate = session.keep_gate.lock().await;
    session.closed.store(true, Ordering::Relaxed);
    if session
        .kept
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|k| !k.done)
    {
        return Ok(());
    }
    if !session.finished.load(Ordering::Acquire) {
        let _ = tokio::time::timeout(Duration::from_secs(8), async {
            while !session.finished.load(Ordering::Acquire) {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await;
    }
    SESSIONS.lock().unwrap().remove(id);
    remove_buffer(&session.directory).await;
    Ok(())
}

pub(crate) async fn play(
    id: &str,
    owner: i32,
    start_ms: Option<u64>,
    quality: &str,
) -> Result<stream::LiveStream, String> {
    let session = session(id, owner)?;
    *session.last_access.lock().unwrap() = Instant::now();
    let current = info(&session, true);
    let position = start_ms
        .unwrap_or(current.end_ms.saturating_sub(6000))
        .clamp(
            current.start_ms,
            current.end_ms.saturating_sub(250).max(current.start_ms),
        );
    let (index, offset, segment_path) = {
        let stats = session.stats.lock().unwrap();
        let origin = stats.origin_ms.unwrap_or(0);
        let index = stats
            .segments
            .iter()
            .rposition(|s| s.timestamp_ms.saturating_sub(origin).max(0) as u64 <= position)
            .unwrap_or(0);
        let segment = stats.segments.get(index).ok_or("TV buffer is not ready")?;
        (
            index,
            position.saturating_sub(segment.timestamp_ms.saturating_sub(origin).max(0) as u64),
            session.directory.join(&segment.name),
        )
    };
    let codecs = session
        .codecs
        .get_or_init(|| stream::buffer_codecs(&segment_path))
        .await;
    stream::play_buffer(&session.files.url, index, offset, quality, *codecs).await
}

pub(crate) async fn keep(
    db: &DbConn,
    id: &str,
    owner: i32,
    private: bool,
) -> Result<i32, String> {
    let session = session(id, owner)?;
    let _keep_gate = session.keep_gate.lock().await;
    if session.closed.load(Ordering::Relaxed) || session.finished.load(Ordering::Acquire) {
        return Err("This TV buffer has ended".into());
    }
    if let Some(id) = session
        .kept
        .lock()
        .unwrap()
        .as_ref()
        .filter(|k| !k.done)
        .map(|k| k.id)
    {
        return Ok(id);
    }
    let channel_id = session.channel;
    let title = session.title.clone();
    let captured_from = session
        .stats
        .lock()
        .unwrap()
        .origin_ms
        .unwrap_or(super::now() * 1000);
    let (recording, path, deadline, from_ms) = db
        .run(move |conn| {
            conn.transaction::<_, Box<dyn std::error::Error + Send + Sync>, _>(|conn| {
                if !settings::can_record(conn, owner).map_err(super::db_error)? {
                    return Err("Recording is disabled for your account".into());
                }
                let channel = storage::channels(conn, false)
                    .map_err(super::db_error)?
                    .into_iter()
                    .find(|c| c.id == channel_id)
                    .ok_or("Channel not found")?;
                let key = channel.guide_override.unwrap_or(channel.guide_key);
                let program = p::table
                    .filter(p::source_id.eq(channel.source_id))
                    .filter(p::guide_key.eq(key))
                    .filter(p::starts_at.le(super::now()))
                    .filter(p::ends_at.gt(super::now()))
                    .select((p::title, p::ends_at, p::metadata_json, p::starts_at))
                    .first::<(String, i64, String, i64)>(conn)
                    .optional()
                    .map_err(super::db_error)?;
                let (title, deadline, metadata, program_start) = program.unwrap_or((
                    title,
                    super::now() + 3600,
                    "{}".into(),
                    captured_from / 1000,
                ));
                let from_ms = captured_from.max(program_start * 1000);
                diesel::insert_into(r::table)
                    .values((
                        r::user_id.eq(owner),
                        r::channel_id.eq(channel_id),
                        r::title.eq(title),
                        r::starts_at.eq(from_ms / 1000),
                        r::ends_at.eq(deadline),
                        r::padding_before.eq(0),
                        r::padding_after.eq(0),
                        r::state.eq("recording"),
                        r::metadata_json.eq(metadata),
                        r::shared.eq(!private),
                    ))
                    .execute(conn)
                    .map_err(super::db_error)?;
                let id = r::table
                    .order(r::id.desc())
                    .select(r::id)
                    .first::<i32>(conn)
                    .map_err(super::db_error)?;
                let path = settings::capture_path(conn, id)?;
                std::fs::create_dir_all(path.parent().ok_or("Invalid recording folder")?)
                    .map_err(|_| "Unable to create the recording folder")?;
                diesel::update(r::table.find(id))
                    .set(r::file_name.eq(path.to_string_lossy().as_ref()))
                    .execute(conn)
                    .map_err(super::db_error)?;
                Ok((id, path, deadline, from_ms))
            })
        })
        .await
        .map_err(|e| e.to_string())?;
    *session.kept.lock().unwrap() = Some(Kept {
        id: recording,
        path,
        deadline,
        last_segment: None,
        from_ms,
        done: false,
    });
    Ok(recording)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kept_recordings_wait_for_the_last_completed_segment_at_program_end() {
        let mut segments = vec![Segment {
            name: "segment-0000000000.ts".into(),
            timestamp_ms: 1000,
            duration_ms: 2000,
        }];
        assert!(!recording_end_reached(&segments, 4, 4));
        segments.push(Segment {
            name: "segment-0000000001.ts".into(),
            timestamp_ms: 3000,
            duration_ms: 2000,
        });
        assert!(recording_end_reached(&segments, 5, 4));
        assert!(!recording_end_reached(&segments, 3, 4));
        assert!(recording_end_reached(&[], 34, 4));
    }
    #[test]
    fn ffmpeg_program_dates_accept_basic_numeric_timezone_offsets() {
        let value = concat!(
            "#EXTM3U\n#EXTINF:2.000000,\n",
            "#EXT-X-PROGRAM-DATE-TIME:2026-10-30T16:11:24.482-0400\n",
            "segment-0000000000.ts\n"
        );
        let segments = parse_playlist(value);
        assert_eq!(segments.len(), 1);
        assert_eq!(
            segments[0].timestamp_ms,
            chrono::DateTime::parse_from_rfc3339("2026-10-30T20:11:24.482Z")
                .unwrap()
                .timestamp_millis()
        );
    }
    #[test]
    fn playlist_timeline_retains_actual_durations_and_rejects_paths() {
        let value = concat!(
            "#EXTM3U\n#EXTINF:2.4,\n",
            "#EXT-X-PROGRAM-DATE-TIME:2026-10-30T12:00:00.000Z\n",
            "segment-0000000000.ts\n#EXTINF:3.6,\nsegment-0000000001.ts\n",
            "#EXTINF:2,\n../../private.ts\n",
        );
        let segments = parse_playlist(value);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].duration_ms, 2400);
        assert_eq!(segments[1].timestamp_ms - segments[0].timestamp_ms, 2400);
    }
}
