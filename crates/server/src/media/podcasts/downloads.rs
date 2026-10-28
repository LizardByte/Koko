//! Shared, atomic episode downloads. Partial files never become playback sources.
use super::{
    PodcastMetadata,
    http,
};
use crate::{
    config::{
        PodcastSettings,
        current_settings,
    },
    db::{
        DbConn,
        schema::{
            podcast_items as items,
            podcast_subscriptions as subscriptions,
        },
    },
    utils::current_timestamp,
};
use diesel::QueryResult;
use diesel::prelude::*;
use once_cell::sync::Lazy;
use rocket_okapi::okapi::schemars::{
    self,
    JsonSchema,
};
use serde::{
    Deserialize,
    Serialize,
};
use std::{
    collections::HashMap,
    path::{
        Path,
        PathBuf,
    },
    sync::{
        Arc,
        Mutex,
        Weak,
    },
    time::Duration,
};
use tokio::io::AsyncWriteExt;

/// Availability of a shared local episode copy.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadState {
    /// Waiting for a background worker or the next playback request.
    Queued,
    /// The publisher's complete episode is being saved.
    Downloading,
    /// A complete local copy is available.
    Ready,
    /// The last attempt failed and may be retried.
    Failed,
}

/// Account-visible download status without server filesystem paths.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub struct DownloadInfo {
    /// Current download state.
    pub state: DownloadState,
    /// Complete episode size, when available.
    pub size_bytes: Option<u64>,
    /// Concise failure message, when the publisher or storage is unavailable.
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct LocalDownload {
    pub state: DownloadState,
    pub path: Option<PathBuf>,
    pub size_bytes: Option<u64>,
    pub attempted_at: i64,
    pub error: Option<String>,
}
impl LocalDownload {
    fn queued() -> Self {
        Self {
            state: DownloadState::Queued,
            path: None,
            size_bytes: None,
            attempted_at: 0,
            error: None,
        }
    }

    pub(super) fn info(&self) -> DownloadInfo {
        DownloadInfo {
            state: self.state,
            size_bytes: self.size_bytes,
            error: self.error.clone(),
        }
    }
}

/// Publisher JSON and the private local-copy reference share the existing episode record.
#[derive(Deserialize, Serialize)]
pub(super) struct StoredMetadata {
    #[serde(flatten)]
    pub publisher: PodcastMetadata,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_download: Option<LocalDownload>,
}

static LOCKS: Lazy<Mutex<HashMap<i32, Weak<tokio::sync::Mutex<()>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static SLOTS: Lazy<tokio::sync::Semaphore> = Lazy::new(|| tokio::sync::Semaphore::new(2));

fn episode_lock(id: i32) -> Arc<tokio::sync::Mutex<()>> {
    let mut locks = LOCKS.lock().expect("Podcast download lock is poisoned");
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(&id).and_then(Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(tokio::sync::Mutex::new(()));
    locks.insert(id, Arc::downgrade(&lock));
    lock
}

pub(super) fn stored(
    conn: &mut SqliteConnection,
    id: i32,
) -> QueryResult<StoredMetadata> {
    let json = items::table
        .find(id)
        .select(items::metadata_json)
        .first::<String>(conn)?;
    serde_json::from_str(&json)
        .map_err(|error| diesel::result::Error::DeserializationError(Box::new(error)))
}

pub(super) fn set_record(
    conn: &mut SqliteConnection,
    id: i32,
    record: LocalDownload,
) -> QueryResult<()> {
    conn.immediate_transaction(|conn| write_record(conn, id, record))
}

fn write_record(
    conn: &mut SqliteConnection,
    id: i32,
    record: LocalDownload,
) -> QueryResult<()> {
    let mut value = stored(conn, id)?;
    value.local_download = Some(record);
    let json = serde_json::to_string(&value).expect("Podcast download metadata is serializable");
    diesel::update(items::table.find(id))
        .set(items::metadata_json.eq(json))
        .execute(conn)?;
    Ok(())
}

/// Queue within the feed-save transaction without discarding an existing complete copy.
pub(super) fn queue(
    conn: &mut SqliteConnection,
    id: i32,
) -> QueryResult<()> {
    if stored(conn, id)?.local_download.is_none() {
        write_record(conn, id, LocalDownload::queued())?;
    }
    Ok(())
}

fn directory(settings: &PodcastSettings) -> PathBuf {
    if settings.download_dir.trim().is_empty() {
        Path::new(&current_settings().general.data_dir).join("podcasts")
    } else {
        PathBuf::from(settings.download_dir.trim())
    }
}

/// Return a complete shared copy, downloading once when the episode has not been saved yet.
pub async fn ensure(
    db: &DbConn,
    id: i32,
) -> Result<PathBuf, String> {
    let lock = episode_lock(id);
    let _episode = lock.lock().await;
    let source = db
        .run(move |conn| {
            let value = stored(conn, id)?;
            let (url, mime) =
                super::storage::enclosure(conn, id)?.ok_or(diesel::result::Error::NotFound)?;
            Ok::<_, diesel::result::Error>((value, url, mime))
        })
        .await
        .map_err(|_| "This podcast episode is unavailable")?;
    if let Some(record) = &source.0.local_download {
        if let Some(path) = &record.path {
            if let Ok(file) = tokio::fs::metadata(path).await {
                if file.is_file() && record.size_bytes == Some(file.len()) {
                    return Ok(path.clone());
                }
            }
        }
    }
    let _slot = SLOTS
        .acquire()
        .await
        .map_err(|_| "The podcast downloader is unavailable")?;
    let mut record = LocalDownload {
        state: DownloadState::Downloading,
        attempted_at: current_timestamp(),
        ..LocalDownload::queued()
    };
    let update = record.clone();
    db.run(move |conn| set_record(conn, id, update))
        .await
        .map_err(|_| "Unable to save episode download status")?;
    let root = source.0.publisher.podcast_id;
    let folder = directory(&current_settings().podcasts).join(root.to_string());
    let result = tokio::time::timeout(Duration::from_secs(900), async {
        tokio::fs::create_dir_all(&folder)
            .await
            .map_err(|_| "The podcast download folder could not be created")?;
        let folder = tokio::fs::canonicalize(&folder)
            .await
            .map_err(|_| "The podcast download folder is unavailable")?;
        let response = http::request(&source.1, None).await?;
        let extension = extension(source.2.as_deref());
        let nonce = rand::random::<u64>();
        let path = folder.join(format!("{id}-{nonce:016x}.{extension}"));
        let size = save_response(response, &path).await?;
        Ok::<_, String>((path, size))
    })
    .await
    .map_err(|_| "Podcast download timed out".to_string())
    .and_then(|result| result);
    match &result {
        Ok((path, size)) => {
            record.state = DownloadState::Ready;
            record.path = Some(path.clone());
            record.size_bytes = Some(*size);
        }
        Err(error) => {
            record.state = DownloadState::Failed;
            record.error = Some(error.clone());
        }
    }
    db.run(move |conn| set_record(conn, id, record))
        .await
        .map_err(|_| "Unable to save episode download status")?;
    result.map(|(path, _)| path)
}

fn extension(mime: Option<&str>) -> &'static str {
    match mime.unwrap_or("").split(';').next().unwrap_or("").trim() {
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/mp4" | "audio/x-m4a" => "m4a",
        "audio/aac" => "aac",
        "audio/ogg" | "application/ogg" => "ogg",
        "audio/opus" => "opus",
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/flac" => "flac",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        _ => "bin",
    }
}

struct PartialFile(PathBuf);
impl Drop for PartialFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

async fn save_response(
    mut response: reqwest::Response,
    path: &Path,
) -> Result<u64, String> {
    if response.status() != reqwest::StatusCode::OK {
        return Err("Publisher did not return a complete episode".into());
    }
    let mime = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream");
    if !mime.starts_with("audio/")
        && !mime.starts_with("video/")
        && !mime.starts_with("application/octet-stream")
        && !mime.starts_with("application/ogg")
    {
        return Err("Publisher did not return an audio or video episode".into());
    }
    const LIMIT: u64 = 8 * 1024 * 1024 * 1024;
    let expected = response.content_length();
    if expected.is_some_and(|size| size > LIMIT) {
        return Err("Podcast episode exceeds the 8 GiB download limit".into());
    }
    let partial = PartialFile(path.with_extension("part"));
    let mut file = tokio::fs::File::create(&partial.0)
        .await
        .map_err(|_| "The podcast download folder is not writable")?;
    let result = async {
        let mut size = 0_u64;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "Podcast download was interrupted")?
        {
            size += chunk.len() as u64;
            if size > LIMIT {
                return Err("Podcast episode exceeds the 8 GiB download limit");
            }
            file.write_all(&chunk)
                .await
                .map_err(|_| "Unable to save the episode; check available disk space")?;
        }
        if size == 0 || expected.is_some_and(|expected| expected != size) {
            return Err("Publisher returned an incomplete episode");
        }
        file.sync_all()
            .await
            .map_err(|_| "Unable to finish saving the podcast episode")?;
        Ok(size)
    }
    .await;
    // Wait for pending filesystem writes before the partial-file guard tries to remove it.
    drop(file.into_std().await);
    let size = result?;
    if tokio::fs::try_exists(path).await.unwrap_or(false) {
        // A previous process may have completed the atomic rename before saving its database row.
        let previous = tokio::fs::metadata(path)
            .await
            .map_err(|_| "Saved episode is unavailable")?;
        if previous.len() != size {
            return Err("A conflicting saved episode already exists".into());
        }
    } else {
        tokio::fs::rename(&partial.0, path)
            .await
            .map_err(|_| "Unable to finish saving the podcast episode")?;
    }
    Ok(size)
}

/// Work only on subscribed shows, retry failed downloads after five minutes, and bound each batch.
pub(super) async fn download_pending(db: &DbConn) {
    if !current_settings().podcasts.download_new_episodes {
        return;
    }
    let pending = db
        .run(|conn| {
            let rows = items::table
                .inner_join(
                    subscriptions::table.on(subscriptions::podcast_id.eq(items::podcast_id)),
                )
                .filter(items::enclosure_url.is_not_null())
                .filter(items::metadata_json.like("%\"local_download\"%"))
                .select((items::media_item_id, items::metadata_json))
                .distinct()
                .load::<(i32, String)>(conn)?;
            Ok::<_, diesel::result::Error>(
                rows.into_iter()
                    .filter_map(|(id, json)| {
                        let record = serde_json::from_str::<StoredMetadata>(&json)
                            .ok()?
                            .local_download?;
                        (record.state == DownloadState::Queued
                            || record.state != DownloadState::Ready
                                && current_timestamp() - record.attempted_at >= 300)
                            .then_some(id)
                    })
                    .take(4)
                    .collect::<Vec<_>>(),
            )
        })
        .await;
    if let Ok(ids) = pending {
        for id in ids {
            if let Err(error) = ensure(db, id).await {
                log::warn!("Podcast episode {id} download failed: {error}");
            }
        }
    }
}

#[cfg(test)]
mod tests;
