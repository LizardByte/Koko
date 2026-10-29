//! Local audiobook recordings, ordered parts, chapter metadata, and catalog storage.

#[cfg(test)]
mod tests;

use super::{
    CatalogMediaFile,
    MediaItem,
    MediaItemDetail,
    MediaItemSummary,
    PlannedMediaItem,
    PlayableContainerTarget,
    fallback_title_from_relative_path,
    load_media_item_row,
    prioritized_metadata_links_for_item,
    to_media_item_summary,
};
use crate::config::MetadataProviderId;
use crate::db::schema::audiobook_metadata as store;
use diesel::{
    QueryResult,
    prelude::*,
};
use rocket_okapi::okapi::schemars::{
    self,
    JsonSchema,
};
use serde::{
    Deserialize,
    Serialize,
};
use serde_json::Value;
use std::{
    collections::{
        HashMap,
        HashSet,
    },
    path::{
        Path,
        PathBuf,
    },
};

/// One specific narration of a written work, never inferred from its title alone.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AudiobookRecording {
    /// Provider that identifies this recording.
    pub provider_id: MetadataProviderId,
    /// Recording identifier, including region for Libex.
    pub external_id: String,
    /// Book title supplied by the recording provider.
    pub title: String,
    /// Narrator credits for this version.
    pub narrator: String,
    /// Recording language.
    pub language: Option<String>,
    /// Recording publication year.
    pub release_year: Option<i32>,
    /// Whether the narration is abridged, when known.
    pub abridged: Option<bool>,
    /// Expected runtime; provider minute totals may be rounded.
    pub runtime_ms: Option<i64>,
    /// Cover for this specific narration; older saved identities may not include one.
    #[serde(default)]
    pub artwork_url: Option<String>,
    /// Audible recording identifiers for exact cross-provider enrichment.
    pub asins: Vec<String>,
}

/// Another local recording of the same work, with its own files and listening progress.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct AudiobookVersion {
    /// Local audiobook item identifier.
    pub item_id: i32,
    /// Version label including narrator when known.
    pub label: String,
    /// Duration of this local recording.
    pub duration_ms: Option<i64>,
}

/// A chapter within one audio file, using the original recording's timeline.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AudiobookChapter {
    /// Human-readable chapter title.
    pub title: String,
    /// Inclusive start position in milliseconds.
    pub start_ms: i64,
    /// Exclusive end position in milliseconds.
    pub end_ms: i64,
}

/// Local recording metadata, kept separate from music and ebook editions.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(default)]
pub struct AudiobookMetadata {
    /// Optional local version folder label, independent of a confirmed provider narration.
    pub version_label: String,
    /// Confirmed narration identity; absent until a recording is selected.
    pub recording: Option<AudiobookRecording>,
    /// Independent folder title hint retained even when embedded tags disagree.
    pub folder_title: String,
    /// Independent folder author hint retained even when embedded tags disagree.
    pub folder_author: String,
    /// Explicit recording identifier from an embedded ASIN tag.
    pub asin: Option<String>,
    /// Book title, from the album tag or containing directory.
    pub title: String,
    /// Author credit from author, composer, or artist tags.
    pub author: String,
    /// Narrator credit when explicitly tagged.
    pub narrator: String,
    /// Recording language when tagged.
    pub language: Option<String>,
    /// Series title when tagged.
    pub series: Option<String>,
    /// Position in the series, including fractional volume labels.
    pub series_position: Option<String>,
    /// Canonical written-work identity, independent of its narration or file layout.
    pub openlibrary_work_id: Option<String>,
    /// Disc number for multipart recordings.
    pub disc_number: Option<i32>,
    /// Part number within the disc.
    pub part_number: Option<i32>,
    /// File-relative chapters on parts, or recording-relative chapters on matched books.
    pub chapters: Vec<AudiobookChapter>,
    /// Provider section filenames, aligned with chapters, for matching real local file starts.
    pub chapter_files: Vec<String>,
}

pub(super) fn asset_source_path(path: PathBuf) -> PathBuf {
    if let Some(directory) = path.parent() {
        if directory
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(disc_number)
            .is_some()
        {
            if let (Some(parent), Some(file)) = (directory.parent(), path.file_name()) {
                return parent.join(file);
            }
        }
    }
    path
}

fn positive_number(value: Option<String>) -> Option<i32> {
    value?
        .split('/')
        .next()?
        .trim()
        .parse()
        .ok()
        .filter(|n| *n > 0)
}

fn disc_number(name: &str) -> Option<i32> {
    let lower = name.to_ascii_lowercase();
    ["disc", "disk", "cd"].into_iter().find_map(|prefix| {
        positive_number(
            lower
                .strip_prefix(prefix)
                .map(|value| value.trim_matches([' ', '-', '_']).to_string()),
        )
    })
}

fn type_directory(name: &str) -> bool {
    [
        "audiobooks",
        "books",
        "comics",
    ]
    .contains(&name.to_ascii_lowercase().as_str())
}

fn book_directory(path: &str) -> (String, Option<i32>) {
    let directory = path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
    let (parent, name) = directory.rsplit_once('/').unwrap_or(("", directory));
    let disc = disc_number(name);
    (
        if disc.is_some() { parent } else { directory }.to_string(),
        disc,
    )
}

fn milliseconds(value: &Value) -> Option<i64> {
    let seconds = value
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .or_else(|| value.as_f64())?;
    (seconds.is_finite() && seconds >= 0.0 && seconds < i64::MAX as f64 / 1000.0)
        .then(|| (seconds * 1000.0).round() as i64)
}

impl AudiobookMetadata {
    fn from_file(file: &CatalogMediaFile) -> (Self, String, String) {
        let raw = file
            .metadata_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
            .unwrap_or_default();
        let tags = audiobook_probe_tags(&raw);
        let tag = |keys: &[&str]| keys.iter().find_map(|key| tags.get(*key).cloned());
        let (directory, inferred_disc) = book_directory(&file.relative_path);
        let fallback = fallback_title_from_relative_path(&file.relative_path);
        let mut directories = directory
            .split('/')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        let type_grouped = directories.first().is_some_and(|name| type_directory(name));
        while directories.first().is_some_and(|name| type_directory(name)) {
            directories.remove(0);
        }
        let version_without_author = if directories.len() == 2
            && directories.last().is_some_and(|name| {
                (name.starts_with('[') && name.ends_with(']'))
                    || name.to_ascii_lowercase().starts_with("version - ")
                    || tag(&[
                        "narrator",
                        "readby",
                        "performer",
                    ])
                    .is_some_and(|narrator| {
                        normalized_identity(name) == normalized_identity(&narrator)
                    })
            }) {
            directories
                .pop()
                .map(|name| name.trim_matches(['[', ']']).to_string())
        } else {
            None
        };
        let folder = if directories.len() >= 2 {
            directories[1]
        } else if directory.is_empty() && inferred_disc.is_some() {
            Path::new(&file.source_root_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
        } else {
            directories.last().copied().unwrap_or_default()
        };
        let title = tag(&["album"]).unwrap_or_else(|| {
            let folder_title = if directory.is_empty() && inferred_disc.is_some() {
                Path::new(&file.source_root_path)
                    .file_name()
                    .and_then(|name| name.to_str())
            } else {
                Some(folder)
            };
            folder_title
                .filter(|s| !s.is_empty())
                .unwrap_or(&fallback)
                .to_string()
        });
        let root = Path::new(&file.source_root_path);
        let author_directory = if directories.len() >= 2 {
            Some(directories[0])
        } else if directories.is_empty() && inferred_disc.is_some() {
            root.parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                .filter(|name| !type_directory(name))
        } else if type_grouped {
            None
        } else {
            root.file_name()
                .and_then(|name| name.to_str())
                .filter(|name| {
                    !["audiobooks", "books", "media"].contains(&name.to_ascii_lowercase().as_str())
                })
        };
        let author = tag(&[
            "author",
            "composer",
            "albumartist",
            "artist",
        ])
        .or_else(|| author_directory.map(str::to_string))
        .unwrap_or_default();
        let mut chapters = raw["chapters"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|chapter| {
                let start = milliseconds(&chapter["start_time"])?;
                let end = milliseconds(&chapter["end_time"])?;
                let end = file.duration_ms.map_or(end, |duration| end.min(duration));
                (end > start).then(|| AudiobookChapter {
                    title: chapter
                        .pointer("/tags/title")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .unwrap_or("Chapter")
                        .to_string(),
                    start_ms: start,
                    end_ms: end,
                })
            })
            .collect::<Vec<_>>();
        chapters.sort_by_key(|chapter| (chapter.start_ms, chapter.end_ms));
        chapters.dedup_by_key(|chapter| chapter.start_ms);
        let metadata = Self {
            version_label: version_without_author.unwrap_or_else(|| {
                directories
                    .iter()
                    .skip(2)
                    .copied()
                    .collect::<Vec<_>>()
                    .join(" / ")
            }),
            recording: None,
            chapter_files: Vec::new(),
            folder_title: folder.into(),
            folder_author: author_directory.unwrap_or_default().into(),
            asin: tag(&["asin", "audibleasin"]),
            title,
            author,
            narrator: tag(&[
                "narrator",
                "readby",
                "performer",
            ])
            .unwrap_or_default(),
            language: tag(&["language"]),
            series: tag(&["series", "mvnm"]),
            series_position: tag(&[
                "seriespart",
                "seriesposition",
                "mvin",
            ]),
            openlibrary_work_id: None,
            disc_number: positive_number(tag(&["disc", "discnumber"])).or(inferred_disc),
            part_number: tag(&["track", "tracknumber"])
                .and_then(|value| value.split('/').next()?.trim().parse::<i32>().ok())
                .filter(|number| *number >= 0),
            chapters,
        };
        // A library root can itself be the book folder containing the CD directories.
        let location = if directory.is_empty() && inferred_disc.is_some() {
            ".".to_string()
        } else {
            directory
        };
        (metadata, tag(&["title"]).unwrap_or(fallback), location)
    }
}

/// Persist the user's recording choice separately from provider priority and work matching.
pub(crate) fn select_recording_source(
    conn: &mut SqliteConnection,
    id: i32,
    provider: &MetadataProviderId,
    external: &str,
) -> QueryResult<()> {
    use crate::db::schema::audiobook_recording_selections as selected;
    let recording = match provider {
        MetadataProviderId::Libex => external.ends_with("/recording"),
        MetadataProviderId::LibriVox => external.ends_with("/recording"),
        MetadataProviderId::AudioSilo => external.split('/').count() == 2,
        _ => return Ok(()),
    };
    if recording {
        diesel::insert_into(selected::table)
            .values((
                selected::media_item_id.eq(id),
                selected::provider_id.eq(provider.as_storage_value()),
                selected::external_id.eq(external),
            ))
            .on_conflict(selected::media_item_id)
            .do_update()
            .set((
                selected::provider_id.eq(provider.as_storage_value()),
                selected::external_id.eq(external),
            ))
            .execute(conn)?;
    } else {
        diesel::delete(
            selected::table
                .filter(selected::media_item_id.eq(id))
                .filter(selected::provider_id.eq(provider.as_storage_value())),
        )
        .execute(conn)?;
    }
    Ok(())
}

pub(super) fn plan_file(
    file: &CatalogMediaFile,
    library_id: i32,
) -> Vec<PlannedMediaItem> {
    let (metadata, part_title, directory) = AudiobookMetadata::from_file(file);
    // Folder identity preserves separate recordings even when their book tags match.
    // Files directly in a library root are independent single-file books.
    let location = if directory.is_empty() { file.relative_path.clone() } else { directory };
    let root = &file.source_root_path;
    let key = format!(
        "audiobook:{library_id}:{}:{root}:{}:{location}",
        root.len(),
        location.len()
    );
    let mut book_metadata = metadata.clone();
    book_metadata.chapters.clear();
    book_metadata.disc_number = None;
    book_metadata.part_number = None;
    let mut plans = [
        (
            key.clone(),
            None,
            "audiobook",
            metadata.title.clone(),
            book_metadata,
        ),
        (
            format!("file:{}", file.library_file_id),
            Some(key),
            "audiobook_part",
            part_title,
            metadata,
        ),
    ]
    .into_iter()
    .map(
        |(identity_key, parent_identity_key, item_type, display_title, metadata)| {
            PlannedMediaItem {
                identity_key,
                parent_identity_key,
                item_type: item_type.into(),
                display_title,
                relative_path: (item_type == "audiobook_part").then(|| file.relative_path.clone()),
                media_kind: Some("audio".into()),
                season_number: None,
                episode_number: None,
                playable: item_type == "audiobook_part",
                child_count: 0,
                file_size: (item_type == "audiobook_part").then_some(file.file_size),
                duration_ms: file.duration_ms,
                modified_at: file.modified_at,
                explicit_id: (item_type == "audiobook_part").then_some(file.library_file_id),
                missing_since: file.missing_since,
                available_leaf_count: i32::from(file.missing_since.is_none()),
                missing_leaf_count: i32::from(file.missing_since.is_some()),
                music: None,
                audiobook: Some(metadata),
            }
        },
    )
    .collect::<Vec<_>>();
    let local = plans[0].audiobook.as_ref().unwrap();
    let author_hint = if local.folder_author.is_empty() {
        metadata_author(&plans[0])
    } else {
        &local.folder_author
    };
    let work_title =
        if local.folder_title.is_empty() { &local.title } else { &local.folder_title }.clone();
    let author_name =
        if author_hint.is_empty() { "Unknown author".into() } else { author_hint.to_string() };
    let author_key = format!(
        "book-author:{library_id}:{}",
        normalized_identity(&author_name)
    );
    let work_key = format!(
        "book-work:{library_id}:{}:{}",
        normalized_identity(&author_name),
        normalized_identity(&work_title)
    );
    let mut author = plans[0].clone();
    author.identity_key = author_key.clone();
    author.parent_identity_key = None;
    author.item_type = "book_author".into();
    author.display_title = author_name;
    author.media_kind = Some("book".into());
    author.duration_ms = None;
    author.audiobook = None;
    let mut work = plans[0].clone();
    work.identity_key = work_key.clone();
    work.parent_identity_key = Some(author_key);
    work.item_type = "book_work".into();
    work.display_title = work_title.clone();
    work.media_kind = Some("book".into());
    work.duration_ms = None;
    if let Some(metadata) = work.audiobook.as_mut() {
        metadata.title = work_title;
        metadata.author = author.display_title.clone();
        metadata.version_label.clear();
        metadata.asin = None;
        metadata.narrator.clear();
        metadata.language = None;
    }
    plans[0].parent_identity_key = Some(work_key);
    let mut hierarchy = vec![author, work];
    hierarchy.extend(plans);
    hierarchy
}

fn metadata_author(item: &PlannedMediaItem) -> &str {
    item.audiobook
        .as_ref()
        .map(|metadata| metadata.author.as_str())
        .unwrap_or_default()
}
fn normalized_identity(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

pub(super) fn plan_ebook(
    file: &CatalogMediaFile,
    library_id: i32,
) -> Vec<PlannedMediaItem> {
    let mut plans = plan_file(file, library_id);
    let parent = plans[1].identity_key.clone();
    plans.remove(2);
    let leaf = plans.last_mut().unwrap();
    leaf.parent_identity_key = Some(parent);
    leaf.item_type = "book".into();
    leaf.display_title = fallback_title_from_relative_path(&file.relative_path);
    leaf.media_kind = Some("book".into());
    leaf.playable = false;
    leaf.audiobook = None;
    if super::ebooks::is_comic(&file.path) {
        let mut issue = plans.pop().unwrap();
        let mut series = plans.pop().unwrap();
        series.identity_key = format!("comic-series:{}", series.identity_key);
        series.parent_identity_key = None;
        series.item_type = "comic_series".into();
        series.audiobook = None;
        issue.parent_identity_key = Some(series.identity_key.clone());
        issue.item_type = "comic_issue".into();
        return vec![series, issue];
    }
    plans
}

pub(super) fn save(
    conn: &mut SqliteConnection,
    id: i32,
    metadata: &AudiobookMetadata,
) -> QueryResult<usize> {
    diesel::insert_into(store::table)
        .values((
            store::media_item_id.eq(id),
            store::metadata_json
                .eq(serde_json::to_string(metadata).expect("serializable audiobook metadata")),
        ))
        .on_conflict(store::media_item_id)
        .do_update()
        .set(
            store::metadata_json
                .eq(serde_json::to_string(metadata).expect("serializable audiobook metadata")),
        )
        .execute(conn)
}

pub(crate) fn load_local(
    conn: &mut SqliteConnection,
    ids: &[i32],
) -> QueryResult<HashMap<i32, AudiobookMetadata>> {
    if ids.len() > 500 {
        let mut result = HashMap::new();
        for chunk in ids.chunks(500) {
            result.extend(load_local(conn, chunk)?);
        }
        return Ok(result);
    }
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let local = store::table
        .filter(store::media_item_id.eq_any(ids))
        .select((store::media_item_id, store::metadata_json))
        .load::<(i32, String)>(conn)?
        .into_iter()
        .filter_map(|(id, raw)| {
            serde_json::from_str::<AudiobookMetadata>(&raw)
                .ok()
                .map(|value| (id, value))
        })
        .collect::<HashMap<_, _>>();
    Ok(local)
}

pub(super) fn load(
    conn: &mut SqliteConnection,
    ids: &[i32],
) -> QueryResult<HashMap<i32, AudiobookMetadata>> {
    if ids.len() > 500 {
        let mut result = HashMap::new();
        for chunk in ids.chunks(500) {
            result.extend(load(conn, chunk)?);
        }
        return Ok(result);
    }
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let mut local = load_local(conn, ids)?;
    use crate::db::schema::audiobook_recording_selections as selected;
    let selections = selected::table
        .filter(selected::media_item_id.eq_any(ids))
        .select((
            selected::media_item_id,
            selected::provider_id,
            selected::external_id,
        ))
        .load::<(i32, String, String)>(conn)?
        .into_iter()
        .map(|(id, provider, external)| (id, (provider, external)))
        .collect::<HashMap<_, _>>();
    use crate::db::schema::audiobook_provider_metadata as providers;
    let rows = providers::table
        .filter(providers::media_item_id.eq_any(ids))
        .select((
            providers::media_item_id,
            providers::provider_id,
            providers::external_id,
            providers::metadata_json,
        ))
        .load::<(i32, String, String, String)>(conn)?;
    let rows = rows
        .into_iter()
        .map(|(id, provider, external, raw)| ((id, provider, external), raw))
        .collect::<HashMap<_, _>>();
    let provider_item_ids = rows.keys().map(|(id, _, _)| *id).collect::<HashSet<_>>();
    for id in ids
        .iter()
        .copied()
        .filter(|id| provider_item_ids.contains(id))
    {
        let Some(item) = load_media_item_row(conn, id)? else {
            continue;
        };
        let summary = to_media_item_summary(item);
        let links = prioritized_metadata_links_for_item(conn, id, summary.library_id, &[])?;
        let mut recording_sources = Vec::new();
        // Lower-priority primary sources fill gaps; secondary sources only supplement them.
        for secondary in [false, true] {
            for link in links.iter().rev().filter(|link| {
                link.match_state == "linked" && (link.relation_kind == "secondary") == secondary
            }) {
                let Some(raw) = rows.get(&(id, link.provider_id.clone(), link.external_id.clone()))
                else {
                    continue;
                };
                let Some(mut provider) = serde_json::from_str::<AudiobookMetadata>(raw).ok() else {
                    continue;
                };
                if provider.recording.is_some() {
                    recording_sources.push(provider.clone());
                }
                provider.narrator.clear();
                provider.chapters.clear();
                provider.chapter_files.clear();
                provider.recording = None;
                if let Some(value) = local.get_mut(&id) {
                    merge_metadata(value, provider, !secondary);
                }
            }
        }
        if let (Some(value), Some((source, external))) = (local.get_mut(&id), selections.get(&id)) {
            apply_selected_recording(value, source, external, &recording_sources);
        }
    }
    Ok(local)
}

fn merge_metadata(
    value: &mut AudiobookMetadata,
    provider: AudiobookMetadata,
    overwrite: bool,
) {
    if !provider.title.is_empty() && (overwrite || value.title.is_empty()) {
        value.title = provider.title;
    }
    if !provider.author.is_empty() && (overwrite || value.author.is_empty()) {
        value.author = provider.author;
    }
    let narrator_agrees = value.narrator.is_empty()
        || provider.narrator.is_empty()
        || value.narrator.eq_ignore_ascii_case(&provider.narrator);
    if !provider.chapters.is_empty()
        && (overwrite || (value.chapters.is_empty() && narrator_agrees))
    {
        value.chapters = provider.chapters;
    }
    if !provider.narrator.is_empty() && (overwrite || value.narrator.is_empty()) {
        value.narrator = provider.narrator;
    }
    if overwrite || value.language.is_none() {
        value.language = provider.language.or(value.language.take());
    }
    if overwrite || value.series.is_none() {
        value.series = provider.series.or(value.series.take());
    }
    if overwrite || value.series_position.is_none() {
        value.series_position = provider.series_position.or(value.series_position.take());
    }
    value.openlibrary_work_id = provider
        .openlibrary_work_id
        .or(value.openlibrary_work_id.take());
}

pub(crate) fn save_provider(
    conn: &mut SqliteConnection,
    id: i32,
    snapshot: &crate::metadata::StoredMetadataSnapshot,
) -> QueryResult<()> {
    use crate::db::schema::audiobook_provider_metadata as providers;
    let metadata =
        snapshot
            .provider_payload_json
            .as_deref()
            .and_then(|raw| match snapshot.provider_id {
                MetadataProviderId::AudioSilo => {
                    crate::metadata::providers::audiosilo::recording_metadata(raw)
                }
                MetadataProviderId::Libex => {
                    crate::metadata::providers::libex::recording_metadata(raw)
                }
                MetadataProviderId::LibriVox => {
                    crate::metadata::providers::librivox::recording_metadata(raw)
                }
                MetadataProviderId::OpenLibrary => {
                    crate::metadata::providers::openlibrary::work_metadata(raw)
                }
                _ => None,
            });
    if let Some(metadata) = metadata {
        if snapshot.provider_id == MetadataProviderId::LibriVox && metadata.recording.is_some() {
            // Strong automatic matches can select a narration once; explicit user choices win.
            use crate::db::schema::audiobook_recording_selections as selected;
            diesel::insert_into(selected::table)
                .values((
                    selected::media_item_id.eq(id),
                    selected::provider_id.eq("librivox"),
                    selected::external_id.eq(&snapshot.external_id),
                ))
                .on_conflict(selected::media_item_id)
                .do_nothing()
                .execute(conn)?;
        }
        let raw = serde_json::to_string(&metadata).expect("serializable audiobook metadata");
        diesel::insert_into(providers::table)
            .values((
                providers::media_item_id.eq(id),
                providers::provider_id.eq(snapshot.provider_id.as_storage_value()),
                providers::external_id.eq(&snapshot.external_id),
                providers::metadata_json.eq(&raw),
            ))
            .on_conflict((providers::media_item_id, providers::provider_id))
            .do_update()
            .set((
                providers::external_id.eq(&snapshot.external_id),
                providers::metadata_json.eq(&raw),
            ))
            .execute(conn)?;
    }
    Ok(())
}

pub(super) fn apply_summaries(
    conn: &mut SqliteConnection,
    items: &mut [MediaItemSummary],
) -> QueryResult<()> {
    let mut metadata = load(
        conn,
        &items
            .iter()
            .filter(|item| {
                matches!(
                    item.item_type.as_str(),
                    "audiobook" | "audiobook_part" | "book_work"
                )
            })
            .map(|item| item.id)
            .collect::<Vec<_>>(),
    )?;
    for item in items {
        if let Some(value) = metadata.remove(&item.id) {
            if item.item_type == "audiobook" {
                item.display_title = value.title.clone();
                if value
                    .recording
                    .as_ref()
                    .is_none_or(|recording| recording.provider_id != MetadataProviderId::Libex)
                {
                    item.overview = None;
                    item.release_year = None;
                }
            }
            item.display_subtitle = Some(
                [
                    Some(value.author.clone()),
                    value
                        .recording
                        .as_ref()
                        .map(|recording| format!("Read by {}", recording.narrator)),
                ]
                .into_iter()
                .flatten()
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join(" · "),
            );
            item.audiobook = Some(Box::new(value));
        }
    }
    Ok(())
}

pub(super) fn apply_detail(
    conn: &mut SqliteConnection,
    item: &mut MediaItemDetail,
) -> QueryResult<()> {
    item.audiobook = load(conn, &[item.id])?.remove(&item.id).map(Box::new);
    apply_summaries(conn, &mut item.hierarchy)?;
    apply_summaries(conn, &mut item.children)?;
    if item.item_type == "audiobook_part" {
        if let (Some(local), Some(book)) = (
            item.audiobook.as_mut(),
            item.hierarchy
                .iter()
                .rev()
                .find_map(|ancestor| ancestor.audiobook.as_ref()),
        ) {
            local.title = book.title.clone();
            if local.author.is_empty() {
                local.author = book.author.clone();
            }
            if local.narrator.is_empty() {
                local.narrator = book.narrator.clone();
            }
            if local.language.is_none() {
                local.language = book.language.clone();
            }
        }
    }
    if item.item_type == "audiobook" {
        if let Some(work) = item
            .hierarchy
            .iter()
            .find(|ancestor| ancestor.item_type == "book_work")
            .and_then(|ancestor| ancestor.audiobook.as_ref())
        {
            if let Some(metadata) = item.audiobook.as_mut() {
                metadata.openlibrary_work_id = work
                    .openlibrary_work_id
                    .clone()
                    .or(metadata.openlibrary_work_id.take());
            }
        }
        if let Some(metadata) = item.audiobook.as_ref() {
            item.release_year = metadata
                .recording
                .as_ref()
                .and_then(|recording| recording.release_year);
            if metadata
                .recording
                .as_ref()
                .is_none_or(|recording| recording.provider_id != MetadataProviderId::Libex)
            {
                item.overview = item
                    .hierarchy
                    .iter()
                    .find(|ancestor| ancestor.item_type == "book_work")
                    .and_then(|work| work.overview.clone());
            }
        }
        sort_parts(&mut item.children);
        if let Some(parent) = item.parent_id {
            item.audiobook_versions = audiobook_versions(conn, parent)?;
        }
    }
    Ok(())
}

/// Numeric filename runs sort naturally and identically on the server and client.
fn natural_key(path: &str) -> String {
    let mut key = String::new();
    let mut digits = String::new();
    for c in path.to_lowercase().chars().chain(std::iter::once('\0')) {
        if c.is_ascii_digit() {
            digits.push(c);
            continue;
        }
        if !digits.is_empty() {
            let trimmed = digits.trim_start_matches('0');
            key.push_str(&format!("{:010}:{trimmed}", trimmed.len()));
            digits.clear();
        }
        key.push(c);
    }
    key
}

fn part_sort_key(
    item: &MediaItemSummary,
    tagged_discs: &HashSet<i32>,
) -> (i32, i32, String, i32) {
    let disc = item
        .audiobook
        .as_ref()
        .and_then(|m| m.disc_number)
        .unwrap_or(1);
    (
        disc,
        if tagged_discs.contains(&disc) {
            item.audiobook
                .as_ref()
                .and_then(|m| m.part_number)
                .unwrap_or(0)
        } else {
            0
        },
        natural_key(&item.relative_path),
        item.id,
    )
}

// A partially tagged disc must not move its few tagged files ahead of its untagged files.
fn sort_parts(items: &mut [MediaItemSummary]) {
    let mut discs: HashMap<i32, (bool, HashSet<i32>)> = HashMap::new();
    for item in items.iter() {
        let metadata = item.audiobook.as_deref();
        let disc = metadata.and_then(|m| m.disc_number).unwrap_or(1);
        let entry = discs.entry(disc).or_insert_with(|| (true, HashSet::new()));
        entry.0 &= metadata
            .and_then(|m| m.part_number)
            .is_some_and(|number| entry.1.insert(number));
    }
    let tagged = discs
        .into_iter()
        .filter_map(|(disc, (complete, _))| complete.then_some(disc))
        .collect();
    items.sort_by_key(|item| part_sort_key(item, &tagged));
}

pub(super) fn playable_parts(
    conn: &mut SqliteConnection,
    book: &MediaItem,
) -> QueryResult<Vec<PlayableContainerTarget>> {
    use crate::db::schema::media_items as items;
    let rows = items::table
        .filter(items::parent_id.eq(book.id))
        .filter(items::library_id.eq(book.library_id))
        .filter(items::item_type.eq("audiobook_part"))
        .filter(items::playable.eq(true))
        .filter(items::deleted_at.is_null())
        .select(MediaItem::as_select())
        .load::<MediaItem>(conn)?;
    let mut summaries = rows
        .iter()
        .cloned()
        .map(to_media_item_summary)
        .collect::<Vec<_>>();
    apply_summaries(conn, &mut summaries)?;
    sort_parts(&mut summaries);
    let mut rows = rows
        .into_iter()
        .filter(|item| item.missing_since.is_none())
        .map(|row| (row.id, row))
        .collect::<HashMap<_, _>>();
    Ok(summaries
        .into_iter()
        .filter_map(|item| rows.remove(&item.id))
        .map(|item| PlayableContainerTarget {
            item,
            season_number: None,
            episode_number: None,
        })
        .collect())
}

fn audiobook_probe_tags(raw: &Value) -> HashMap<String, String> {
    let mut tags = HashMap::new();
    let audio = raw["streams"].as_array().and_then(|streams| {
        streams
            .iter()
            .find(|stream| stream["codec_type"] == "audio")
    });
    for source in [
        audio.and_then(|stream| stream.get("tags")),
        raw.pointer("/format/tags"),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(source) = source.as_object() {
            for (key, value) in source {
                if let Some(value) = value.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                    let key = key
                        .chars()
                        .filter(char::is_ascii_alphanumeric)
                        .flat_map(char::to_lowercase)
                        .collect::<String>();
                    tags.insert(key, value.to_string());
                }
            }
        }
    }
    tags
}

fn apply_selected_recording(
    value: &mut AudiobookMetadata,
    source: &str,
    external: &str,
    recording_sources: &[AudiobookMetadata],
) {
    if let Some(chosen) = recording_sources.iter().find(|metadata| {
        metadata.recording.as_ref().is_some_and(|recording| {
            recording.provider_id.as_storage_value() == source && recording.external_id == external
        })
    }) {
        let recording = chosen.recording.as_ref().unwrap();
        value.narrator = recording.narrator.clone();
        value.language = recording.language.clone().or(value.language.take());
        value.chapters = chosen.chapters.clone();
        value.chapter_files = chosen.chapter_files.clone();
        if value.chapters.is_empty() && recording.provider_id == MetadataProviderId::Libex {
            if let Some(enrichment) = recording_sources.iter().find(|metadata| {
                metadata.recording.as_ref().is_some_and(|other| {
                    other.provider_id == MetadataProviderId::AudioSilo
                        && other
                            .asins
                            .iter()
                            .any(|asin| recording.asins.contains(asin))
                })
            }) {
                value.chapters = enrichment.chapters.clone();
            }
        }
        value.recording = Some(recording.clone());
    }
}

fn audiobook_versions(
    conn: &mut SqliteConnection,
    parent: i32,
) -> QueryResult<Vec<AudiobookVersion>> {
    use crate::db::schema::media_items as items;
    let mut siblings = items::table
        .filter(items::parent_id.eq(parent))
        .filter(items::item_type.eq("audiobook"))
        .filter(items::deleted_at.is_null())
        .select(MediaItem::as_select())
        .load::<MediaItem>(conn)?
        .into_iter()
        .map(to_media_item_summary)
        .collect::<Vec<_>>();
    apply_summaries(conn, &mut siblings)?;
    Ok(siblings
        .into_iter()
        .map(|sibling| {
            let label = sibling
                .audiobook
                .as_ref()
                .map(|metadata| {
                    metadata
                        .recording
                        .as_ref()
                        .map(|recording| recording.narrator.clone())
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| {
                            if metadata.version_label.is_empty() {
                                "Unconfirmed local version".into()
                            } else {
                                metadata.version_label.clone()
                            }
                        })
                })
                .unwrap_or_else(|| sibling.display_title.clone());
            AudiobookVersion {
                item_id: sibling.id,
                label,
                duration_ms: sibling.duration_ms,
            }
        })
        .collect())
}
