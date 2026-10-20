//! Ebook sources, bounded conversion, and independent per-reader positions and bookmarks.

use super::*;
use crate::db::schema::reading_progress as progress;
use diesel::QueryResult;
use serde::Deserialize;
use sha2::{
    Digest,
    Sha256,
};
use std::sync::{
    Arc,
    Weak,
};
use tokio::sync::Mutex;

#[cfg(test)]
mod tests;

/// Account-specific reading state used by item actions without exposing bookmarks.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ReadingStatus {
    /// Last saved fraction through this file.
    pub fraction: f64,
    /// Whether this account has finished the book.
    pub completed: bool,
    /// Whether a reading position has been saved, independently of cached page counts.
    pub started: bool,
    /// Intrinsic/publisher pages or the reader's completed text-layout count, when available.
    pub page_count: Option<u32>,
}

pub(crate) fn status(
    conn: &mut SqliteConnection,
    item: i32,
    user: Option<i32>,
) -> QueryResult<Option<ReadingStatus>> {
    let Some(user) = user else {
        return Ok(None);
    };
    Ok(source(conn, item, user)?
        .and_then(|book| book.document.progress)
        .filter(|state| state.valid() && (!state.locator.is_empty() || state.page_count.is_some()))
        .map(|state| ReadingStatus {
            fraction: state.fraction,
            completed: state.completed,
            started: !state.locator.is_empty(),
            page_count: state.page_count,
        }))
}

/// Restore account-specific card badges in batches, checking the backing file revision.
pub(crate) fn apply_statuses(
    conn: &mut SqliteConnection,
    user: Option<i32>,
    items: &mut [MediaItemSummary],
) -> QueryResult<()> {
    use crate::db::schema::{
        media_file_libraries as membership,
        media_files as files,
    };

    let mut books = items
        .iter_mut()
        .filter(|item| matches!(item.item_type.as_str(), "book" | "comic_issue"))
        .map(|item| {
            item.reading_progress = None;
            (item.id, item)
        })
        .collect::<HashMap<_, _>>();
    let Some(user) = user.filter(|_| !books.is_empty()) else {
        return Ok(());
    };
    let saved = progress::table
        .filter(progress::user_id.eq(user))
        .filter(progress::media_item_id.eq_any(books.keys().copied().collect::<Vec<_>>()))
        .select((progress::media_item_id, progress::payload_json))
        .load::<(i32, String)>(conn)?
        .into_iter()
        .filter_map(|(id, raw)| {
            let state = serde_json::from_str::<ReadingProgress>(&raw).ok()?;
            (state.valid() && (!state.locator.is_empty() || state.page_count.is_some()))
                .then_some((id, state))
        })
        .collect::<HashMap<_, _>>();
    if saved.is_empty() {
        return Ok(());
    }
    let files = membership::table
        .inner_join(files::table)
        .filter(membership::media_item_id.eq_any(saved.keys().copied().collect::<Vec<_>>()))
        .filter(membership::deleted_at.is_null())
        .filter(membership::missing_since.is_null())
        .order(membership::id.asc())
        .select((
            membership::media_item_id,
            membership::relative_path,
            files::file_hash,
            files::file_size,
        ))
        .load::<(Option<i32>, String, String, i64)>(conn)?;
    for (id, path, hash, size) in files {
        let Some(id) = id else { continue };
        let Some(item) = books.remove(&id) else { continue };
        let state = &saved[&id];
        if item.missing_since.is_some()
            || item.relative_path != path
            || !is_book(Path::new(&path))
            || state.revision != file_revision(&hash, size)
        {
            continue;
        }
        item.reading_progress = Some(ReadingStatus {
            fraction: state.fraction,
            completed: state.completed,
            started: !state.locator.is_empty(),
            page_count: state.page_count,
        });
    }
    Ok(())
}

fn file_revision(
    hash: &str,
    size: i64,
) -> String {
    Sha256::digest(format!("{hash}:{size}"))
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// A portable reading bookmark, using a CFI for reflowable books or a page index.
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
pub(crate) struct Bookmark {
    pub title: String,
    pub locator: String,
}

/// Reading state is separate from audio/video playback, and isolated per account.
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
pub(crate) struct ReadingProgress {
    pub revision: String,
    pub locator: String,
    pub fraction: f64,
    pub completed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_count: Option<u32>,
    #[serde(default)]
    pub bookmarks: Vec<Bookmark>,
}

impl ReadingProgress {
    pub(crate) fn valid(&self) -> bool {
        self.revision.len() <= 128
            && !self.revision.is_empty()
            && self.locator.len() <= 4096
            && self.fraction.is_finite()
            && (0.0..=1.0).contains(&self.fraction)
            && self
                .page_count
                .is_none_or(|count| (1..=100_000).contains(&count))
            && self.bookmarks.len() <= 200
            && self
                .bookmarks
                .iter()
                .all(|mark| mark.title.len() <= 400 && mark.locator.len() <= 4096)
    }
}

/// Format information and the current account's saved position.
#[derive(Serialize, JsonSchema)]
pub(crate) struct ReaderDocument {
    pub item_id: i32,
    pub title: String,
    pub file_name: String,
    pub source_format: String,
    pub format: String,
    pub revision: String,
    pub progress: Option<ReadingProgress>,
}

pub(crate) struct BookSource {
    pub path: PathBuf,
    pub document: ReaderDocument,
}

/// Formats decoded by the client, and legacy formats accepted by ebook-convert.
pub(crate) fn is_book(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "epub"
            | "kepub"
            | "pdf"
            | "mobi"
            | "azw"
            | "azw3"
            | "azw4"
            | "fb2"
            | "fbz"
            | "cbz"
            | "cbr"
            | "cb7"
            | "cbt"
            | "cbc"
            | "chm"
            | "djvu"
            | "djv"
            | "docx"
            | "html"
            | "htm"
            | "xhtml"
            | "htmlz"
            | "lit"
            | "lrf"
            | "odt"
            | "prc"
            | "pdb"
            | "pml"
            | "pmlz"
            | "rb"
            | "rtf"
            | "snb"
            | "tcr"
            | "txt"
            | "text"
            | "txtz"
    ) || path
        .to_string_lossy()
        .to_ascii_lowercase()
        .ends_with(".fb2.zip")
}

pub(crate) fn is_comic(path: &str) -> bool {
    matches!(
        Path::new(path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "cbz" | "cbr" | "cb7" | "cbt" | "cbc"
    ) || Path::new(path).components().any(|part| {
        part.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case("comics")
    })
}

fn native_format(extension: &str) -> Option<&'static str> {
    match extension {
        "epub" | "kepub" => Some("epub"),
        "mobi" | "azw" | "azw3" | "prc" => Some("mobi"),
        "fb2" => Some("fb2"),
        "fbz" | "zip" => Some("fbz"),
        "cbz" => Some("cbz"),
        "cbr" | "cb7" | "cbt" => Some("archive"),
        "pdf" => Some("pdf"),
        "txt" | "text" => Some("text"),
        "html" | "htm" | "xhtml" => Some("html"),
        _ => None,
    }
}

pub(crate) fn source(
    conn: &mut SqliteConnection,
    item_id: i32,
    user: i32,
) -> QueryResult<Option<BookSource>> {
    let Some(item) = load_media_item_row(conn, item_id)?.filter(|item| {
        matches!(item.item_type.as_str(), "book" | "comic_issue") && item.deleted_at.is_none()
    }) else {
        return Ok(None);
    };
    let Some(file) = load_backing_media_file(conn, item_id)? else {
        return Ok(None);
    };
    let Some(path) = resolve_media_item_source_path(conn, item_id)? else {
        return Ok(None);
    };
    if !is_book(&path) {
        return Ok(None);
    }
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let revision = file_revision(&file.file_hash, file.file_size);
    let saved = progress::table
        .find((user, item_id))
        .select(progress::payload_json)
        .first::<String>(conn)
        .optional()?
        .and_then(|raw| serde_json::from_str::<ReadingProgress>(&raw).ok())
        .filter(|state| state.revision == revision);
    let format = native_format(&extension).unwrap_or("epub");
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    Ok(Some(BookSource {
        path,
        document: ReaderDocument {
            item_id,
            title: item.display_title,
            file_name: name,
            source_format: extension,
            format: format.into(),
            revision,
            progress: saved,
        },
    }))
}

pub(crate) fn save_progress(
    conn: &mut SqliteConnection,
    user: i32,
    item: i32,
    state: &ReadingProgress,
) -> QueryResult<()> {
    let payload = serde_json::to_string(state).expect("serializable reading state");
    let updated = chrono::Utc::now().timestamp_millis();
    diesel::insert_into(progress::table)
        .values((
            progress::user_id.eq(user),
            progress::media_item_id.eq(item),
            progress::payload_json.eq(&payload),
            progress::updated_at.eq(updated),
        ))
        .on_conflict((progress::user_id, progress::media_item_id))
        .do_update()
        .set((
            progress::payload_json.eq(&payload),
            progress::updated_at.eq(updated),
        ))
        .execute(conn)?;
    Ok(())
}

pub(crate) fn continue_reading(
    conn: &mut SqliteConnection,
    user: Option<i32>,
    visible: &[MediaItemSummary],
) -> QueryResult<Vec<MediaItemSummary>> {
    let Some(user) = user else {
        return Ok(Vec::new());
    };
    let rows = progress::table
        .filter(progress::user_id.eq(user))
        .order(progress::updated_at.desc())
        .select((progress::media_item_id, progress::payload_json))
        .load::<(i32, String)>(conn)?;
    let mut items = Vec::new();
    for (id, raw) in rows {
        let Some(mut item) = visible
            .iter()
            .find(|item| item.id == id && item.missing_since.is_none())
            .cloned()
        else {
            continue;
        };
        let Ok(saved) = serde_json::from_str::<ReadingProgress>(&raw) else {
            continue;
        };
        if !saved.valid() || saved.completed || saved.fraction <= 0.0 {
            continue;
        }
        if source(conn, id, user)?.is_none_or(|book| book.document.revision != saved.revision) {
            continue;
        }
        item.display_subtitle = Some(format!("{:.0}% read", saved.fraction * 100.0));
        item.reading_progress = Some(ReadingStatus {
            fraction: saved.fraction,
            completed: saved.completed,
            started: !saved.locator.is_empty(),
            page_count: saved.page_count,
        });
        items.push(item);
        if items.len() >= 12 {
            break;
        }
    }
    Ok(items)
}

/// Reconcile progress saved on duplicate catalog entries, retaining each reader's latest state.
pub(crate) fn merge_progress(
    conn: &mut SqliteConnection,
    keep: i32,
    duplicate: i32,
) -> QueryResult<()> {
    let rows = progress::table
        .filter(progress::media_item_id.eq(duplicate))
        .select((
            progress::user_id,
            progress::payload_json,
            progress::updated_at,
        ))
        .load::<(i32, String, i64)>(conn)?;
    for (user, raw, updated) in rows {
        let old = progress::table
            .find((user, keep))
            .select(progress::updated_at)
            .first::<i64>(conn)
            .optional()?;
        if old.is_some_and(|old| old >= updated) {
            continue;
        }
        diesel::insert_into(progress::table)
            .values((
                progress::user_id.eq(user),
                progress::media_item_id.eq(keep),
                progress::payload_json.eq(&raw),
                progress::updated_at.eq(updated),
            ))
            .on_conflict((progress::user_id, progress::media_item_id))
            .do_update()
            .set((
                progress::payload_json.eq(&raw),
                progress::updated_at.eq(updated),
            ))
            .execute(conn)?;
    }
    Ok(())
}

static CONVERSIONS: once_cell::sync::Lazy<Mutex<HashMap<String, Weak<Mutex<()>>>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));
static CONVERSION_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

/// Convert only when needed, inside an isolated cache directory. Never edit the library file.
pub(crate) async fn content(
    source: BookSource,
    settings: &crate::config::Settings,
) -> Result<PathBuf, String> {
    if native_format(&source.document.source_format).is_some() {
        return Ok(source.path);
    }
    let key = format!(
        "v1-{}-{}",
        source.document.revision, source.document.source_format
    );
    let lock = {
        let mut locks = CONVERSIONS.lock().await;
        locks.retain(|_, lock| lock.strong_count() > 0);
        locks.get(&key).and_then(Weak::upgrade).unwrap_or_else(|| {
            let lock = Arc::new(Mutex::new(()));
            locks.insert(key.clone(), Arc::downgrade(&lock));
            lock
        })
    };
    let _guard = lock.lock().await;
    let cache = Path::new(&settings.general.data_dir)
        .join("cache/reader")
        .join(&key);
    let output = cache.join("book.epub");
    if output.is_file() {
        return Ok(output);
    }
    let _slot = CONVERSION_SLOTS
        .acquire()
        .await
        .map_err(|error| error.to_string())?;
    tokio::fs::create_dir_all(&cache)
        .await
        .map_err(|error| error.to_string())?;
    let cache = tokio::fs::canonicalize(&cache)
        .await
        .map_err(|error| error.to_string())?;
    let output = cache.join("book.epub");
    let input = cache.join(format!("source.{}", source.document.source_format));
    tokio::fs::copy(&source.path, &input)
        .await
        .map_err(|error| error.to_string())?;
    let pending = cache.join("pending.epub");
    let _ = tokio::fs::remove_file(&pending).await;
    let mut command = tokio::process::Command::new(&settings.reader.converter_path);
    command
        .arg(&input)
        .arg(&pending)
        .current_dir(&cache)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(settings.reader.conversion_timeout_seconds.clamp(1, 600)),
        command.status(),
    )
    .await;
    let _ = tokio::fs::remove_file(&input).await;
    match result {
        Ok(Ok(status)) if status.success() && pending.is_file() => {
            tokio::fs::rename(&pending, &output)
                .await
                .map_err(|error| error.to_string())?;
            Ok(output)
        }
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(
                "This format needs Calibre. Configure the server's ebook-convert path in Settings \
                 → General."
                    .into(),
            )
        }
        Err(_) => {
            let _ = tokio::fs::remove_file(&pending).await;
            Err("Book conversion timed out. Try a DRM-free EPUB or PDF copy.".into())
        }
        _ => {
            let _ = tokio::fs::remove_file(&pending).await;
            Err(
                "This book could not be converted. Check that the file is readable and DRM-free."
                    .into(),
            )
        }
    }
}
