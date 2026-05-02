//! Media-library inspection, persistence, and transcoding capability utilities.

// standard imports
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

// lib imports
use diesel::{
    ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl, SelectableHelper,
    SqliteConnection, sql_types,
};
use once_cell::sync::Lazy;
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// local imports
use crate::config::{
    FfmpegSettings, MediaLibraryKind, MediaLibraryMetadataLanguageMode, MediaLibrarySettings,
    MetadataProviderId,
};
use crate::db::models::{
    ItemMetadataLink, MediaFile, MediaItem, MediaLibrary, MetadataCollection,
    MetadataCollectionItem, NewMediaFile, NewMediaFileLibrary, NewMediaItem, NewMediaLibrary,
    NewPlaybackProgress, NewScanState, PlaybackProgress, ScanState, User,
};
use crate::metadata::{
    ArtworkKind, DEFAULT_METADATA_LOCALE, MetadataCollectionSummary, MetadataRegistry,
    list_metadata_collection_summaries_with_preferred_languages, normalize_locale_key,
    presentation_from_metadata_links,
};
use crate::utils::current_timestamp;

#[derive(Debug, Clone)]
struct SummaryMetadataLink {
    media_item_id: i32,
    title: Option<String>,
    overview: Option<String>,
    genres_json: Option<String>,
    logo_url: Option<String>,
    cached_logo_path: Option<String>,
    backdrop_url: Option<String>,
    cached_backdrop_path: Option<String>,
    refresh_state: String,
    refresh_error: Option<String>,
    updated_at: Option<i64>,
    locale_key: String,
}

/// Scan status for a configured media library.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LibraryScanStatus {
    /// The library exists in configuration but has not been scanned yet.
    NeverScanned,
    /// The library path exists and was scanned successfully.
    Available,
    /// The library path was empty.
    EmptyPath,
    /// The library path does not exist.
    MissingPath,
    /// The configured path exists but is not a directory.
    NotDirectory,
    /// The library path could not be read completely.
    Unreadable,
}

/// Summary of one configured media library.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct LibraryScanSummary {
    /// Human-friendly library name.
    pub name: String,
    /// Configured filesystem path.
    pub path: String,
    /// Configured filesystem paths for this logical library.
    pub paths: Vec<String>,
    /// Whether the scan is recursive.
    pub recursive: bool,
    /// Intended media category for the library.
    pub kind: MediaLibraryKind,
    /// Scan status for this library.
    pub status: LibraryScanStatus,
    /// Total number of files discovered.
    pub total_files: u64,
    /// Number of video files discovered.
    pub video_files: u64,
    /// Number of audio files discovered.
    pub audio_files: u64,
    /// Number of image files discovered.
    pub image_files: u64,
    /// Number of book or document files discovered.
    pub book_files: u64,
    /// Number of files that do not match known media extensions.
    pub other_files: u64,
    /// The last scan error, if any.
    pub error: Option<String>,
}

/// Details about a discovered executable used for media processing.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct BinaryCapability {
    /// Configured command or path.
    pub configured_path: String,
    /// Whether the executable could be launched successfully.
    pub available: bool,
    /// First line of the version output, when available.
    pub version: Option<String>,
    /// Error details when the executable is unavailable.
    pub error: Option<String>,
}

/// Current FFmpeg tooling availability.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct TranscodingCapability {
    /// FFmpeg executable capability.
    pub ffmpeg: BinaryCapability,
    /// ffprobe executable capability.
    pub ffprobe: BinaryCapability,
}

/// Persisted media library summary with a stable database identity.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct PersistedLibrarySummary {
    /// Stable database identifier for the library.
    pub id: i32,
    /// Human-friendly library name.
    pub name: String,
    /// Configured filesystem path.
    pub path: String,
    /// Configured filesystem paths for this logical library.
    pub paths: Vec<String>,
    /// Whether the scan is recursive.
    pub recursive: bool,
    /// Intended media category for the library.
    pub kind: MediaLibraryKind,
    /// Ordered metadata providers configured for this library.
    pub metadata_providers: Vec<MetadataProviderId>,
    /// Whether metadata languages are inferred from users or set manually.
    pub metadata_language_mode: MediaLibraryMetadataLanguageMode,
    /// Ordered metadata languages configured for this library.
    pub metadata_languages: Vec<String>,
    /// Scan status for this library.
    pub status: LibraryScanStatus,
    /// Monotonically increasing scan revision.
    pub scan_revision: i64,
    /// Last completed scan time as Unix seconds, when available.
    pub last_scanned_at: Option<i64>,
    /// Total number of files discovered.
    pub total_files: i64,
    /// Number of video files discovered.
    pub video_files: i64,
    /// Number of audio files discovered.
    pub audio_files: i64,
    /// Number of image files discovered.
    pub image_files: i64,
    /// Number of book or document files discovered.
    pub book_files: i64,
    /// Number of files that do not match known media extensions.
    pub other_files: i64,
    /// The last scan error, if any.
    pub error: Option<String>,
    /// Number of linked metadata items tracked for refresh progress.
    pub metadata_refresh_total: i64,
    /// Number of linked metadata items still pending refresh.
    pub metadata_refresh_pending: i64,
    /// Number of linked metadata items already processed in the active refresh run.
    pub metadata_refresh_completed: i64,
    /// Number of linked metadata items whose latest refresh failed.
    pub metadata_refresh_failed: i64,
    /// Number of file rows currently marked missing.
    pub missing_files: i64,
    /// Number of item rows currently marked missing.
    pub missing_items: i64,
}

#[derive(Debug, Clone, Default)]
struct LibraryMetadataRefreshCounts {
    total_items: i64,
    pending_items: i64,
    completed_items: i64,
    failed_items: i64,
}

const CATALOG_MEDIA_FILE_COLUMNS: &str = "\
    files.id AS id,\
    memberships.id AS library_file_id,\
    memberships.library_id AS library_id,\
    memberships.source_root_path AS source_root_path,\
    memberships.relative_path AS relative_path,\
    files.file_size AS file_size,\
    files.modified_at AS modified_at,\
    files.media_kind AS media_kind,\
    files.fingerprint_seed AS fingerprint_seed,\
    memberships.display_title AS display_title,\
    files.container AS container,\
    files.duration_ms AS duration_ms,\
    files.bit_rate AS bit_rate,\
    files.width AS width,\
    files.height AS height,\
    files.video_codec AS video_codec,\
    files.audio_codec AS audio_codec,\
    files.metadata_json AS metadata_json,\
    files.metadata_updated_at AS metadata_updated_at,\
    memberships.metadata_match_attempted_at AS metadata_match_attempted_at,\
    memberships.media_item_id AS media_item_id,\
    memberships.missing_since AS missing_since,\
    memberships.deleted_at AS deleted_at";

/// Persisted media file summary for a library.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct PersistedMediaFileSummary {
    /// Stable database identifier for the file row.
    pub id: i32,
    /// Owning media-library identifier.
    pub library_id: i32,
    /// Library-relative file path.
    pub relative_path: String,
    /// File size in bytes.
    pub file_size: i64,
    /// Last modified timestamp as Unix seconds, when available.
    pub modified_at: Option<i64>,
    /// Classified media type for the file.
    pub media_kind: String,
    /// Basic fingerprint seed for future change detection.
    pub fingerprint_seed: String,
    /// Browser-friendly title for the item.
    pub display_title: String,
    /// Container format reported by ffprobe when available.
    pub container: Option<String>,
    /// Duration in milliseconds when available.
    pub duration_ms: Option<i64>,
    /// Video width when available.
    pub width: Option<i32>,
    /// Video height when available.
    pub height: Option<i32>,
    /// Video codec name when available.
    pub video_codec: Option<String>,
    /// Audio codec name when available.
    pub audio_codec: Option<String>,
    /// When this file was first observed as missing from disk.
    pub missing_since: Option<i64>,
}

/// Summary of a browser-visible media item.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct MediaItemSummary {
    /// Stable database identifier for the item.
    pub id: i32,
    /// Owning media-library identifier.
    pub library_id: i32,
    /// Parent item identifier when this item belongs to a hierarchy.
    pub parent_id: Option<i32>,
    /// Logical item type such as movie, show, season, or episode.
    pub item_type: String,
    /// Display title for the item.
    pub display_title: String,
    /// Library-relative file path.
    pub relative_path: String,
    /// Classified media kind.
    pub media_kind: String,
    /// Whether the item can be played directly as a leaf item.
    pub playable: bool,
    /// Number of direct child items.
    pub child_count: i32,
    /// Season number when the item is a season or episode.
    pub season_number: Option<i32>,
    /// Episode number when the item is an episode.
    pub episode_number: Option<i32>,
    /// Duration in milliseconds when available.
    pub duration_ms: Option<i64>,
    /// Video width when available.
    pub width: Option<i32>,
    /// Video height when available.
    pub height: Option<i32>,
    /// Genre labels from linked metadata when available.
    pub genres: Vec<String>,
    /// Description or overview from linked metadata, when available.
    pub overview: Option<String>,
    /// Local or managed backdrop artwork URL, when available.
    pub backdrop_url: Option<String>,
    /// Local or managed title logo URL, when available.
    pub logo_url: Option<String>,
    /// Whether the item currently has linked metadata.
    pub has_metadata: bool,
    /// Current metadata refresh state when metadata exists.
    pub metadata_refresh_state: Option<String>,
    /// Last metadata refresh error, when available.
    pub metadata_refresh_error: Option<String>,
    /// Revision timestamp for artwork cache-busting when linked metadata changes.
    pub artwork_updated_at: Option<i64>,
    /// Last modified timestamp as Unix seconds, when available.
    pub modified_at: Option<i64>,
    /// Last saved playback position for the current user.
    pub playback_position_ms: Option<i64>,
    /// Last saved playback duration for the current user.
    pub playback_duration_ms: Option<i64>,
    /// When this item was first observed as missing from disk.
    pub missing_since: Option<i64>,
}

/// Detailed browser-facing media item response.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct MediaItemDetail {
    /// Stable database identifier for the item.
    pub id: i32,
    /// Owning media-library identifier.
    pub library_id: i32,
    /// Parent item identifier when this item belongs to a hierarchy.
    pub parent_id: Option<i32>,
    /// Logical item type such as movie, show, season, or episode.
    pub item_type: String,
    /// Display title for the item.
    pub display_title: String,
    /// Library-relative file path.
    pub relative_path: String,
    /// File size in bytes.
    pub file_size: Option<i64>,
    /// Last modified timestamp as Unix seconds, when available.
    pub modified_at: Option<i64>,
    /// Classified media kind.
    pub media_kind: String,
    /// Whether the item can be played directly as a leaf item.
    pub playable: bool,
    /// Number of direct child items.
    pub child_count: i32,
    /// Season number when the item is a season or episode.
    pub season_number: Option<i32>,
    /// Episode number when the item is an episode.
    pub episode_number: Option<i32>,
    /// Container format reported by ffprobe when available.
    pub container: Option<String>,
    /// Duration in milliseconds when available.
    pub duration_ms: Option<i64>,
    /// Bit rate when available.
    pub bit_rate: Option<i64>,
    /// Video width when available.
    pub width: Option<i32>,
    /// Video height when available.
    pub height: Option<i32>,
    /// Video codec name when available.
    pub video_codec: Option<String>,
    /// Audio codec name when available.
    pub audio_codec: Option<String>,
    /// Raw ffprobe JSON payload, when available.
    pub metadata_json: Option<String>,
    /// Metadata update timestamp as Unix seconds, when available.
    pub metadata_updated_at: Option<i64>,
    /// Local or managed poster artwork URL, when available.
    pub poster_url: Option<String>,
    /// Local or managed backdrop artwork URL, when available.
    pub backdrop_url: Option<String>,
    /// Theme-song URL, when available.
    pub theme_song_url: Option<String>,
    /// Tagline from linked metadata, when available.
    pub tagline: Option<String>,
    /// Description or overview from linked metadata, when available.
    pub overview: Option<String>,
    /// Genre labels from linked metadata.
    pub genres: Vec<String>,
    /// Release year from linked metadata, when available.
    pub release_year: Option<i32>,
    /// Provider-supplied title logo URL, when available.
    pub logo_url: Option<String>,
    /// Provider-supplied user/community rating, when available.
    pub rating: Option<f32>,
    /// Provider-supplied content rating such as PG-13 or TV-MA, when available.
    pub content_rating: Option<String>,
    /// Linked metadata media type such as movie or tv.
    pub linked_media_type: Option<String>,
    /// Whether the item currently has linked metadata.
    pub has_metadata: bool,
    /// Current metadata refresh state when metadata exists.
    pub metadata_refresh_state: Option<String>,
    /// Last metadata refresh error, when available.
    pub metadata_refresh_error: Option<String>,
    /// Revision timestamp for artwork cache-busting when linked metadata changes.
    pub artwork_updated_at: Option<i64>,
    /// Trailer title, when available.
    pub trailer_title: Option<String>,
    /// Trailer URL, when available.
    pub trailer_url: Option<String>,
    /// Audio streams discovered in the source container.
    pub audio_tracks: Vec<MediaAudioTrack>,
    /// Discovered subtitle sidecars for this item.
    pub subtitle_tracks: Vec<MediaSubtitleTrack>,
    /// Breadcrumb-like hierarchy for this item.
    pub hierarchy: Vec<MediaItemSummary>,
    /// Direct child items for hierarchical browsing.
    pub children: Vec<MediaItemSummary>,
    /// Last saved playback position for the current user.
    pub playback_position_ms: Option<i64>,
    /// Last saved playback duration for the current user.
    pub playback_duration_ms: Option<i64>,
    /// When this item was first observed as missing from disk.
    pub missing_since: Option<i64>,
}

/// Result of removing missing items from the active catalog.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct MissingItemsCleanupSummary {
    /// Library scoped by the cleanup request, when applicable.
    pub library_id: Option<i32>,
    /// File rows removed from the active catalog.
    pub deleted_files: i64,
    /// Item rows removed from the active catalog.
    pub deleted_items: i64,
    /// Collection membership rows removed from active collection/list views.
    pub removed_collection_items: i64,
}

/// One audio stream discovered inside a media file.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct MediaAudioTrack {
    /// Zero-based audio stream index among audio streams, suitable for `0:a:<index>`.
    pub index: usize,
    /// Human-friendly label.
    pub label: String,
    /// Codec name when available.
    pub codec: Option<String>,
    /// Stream language when available.
    pub language: Option<String>,
    /// Whether the stream is marked as default.
    pub default: bool,
}

/// Subtitle track discovered for one media item.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct MediaSubtitleTrack {
    /// Stable index used to request the subtitle asset.
    pub index: usize,
    /// Human-friendly track label.
    pub label: String,
    /// Subtitle container or format.
    pub format: String,
    /// Browser-facing asset URL.
    pub url: String,
}

/// One media shelf on the browser home screen.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct MediaShelf {
    /// Stable shelf identifier.
    pub id: String,
    /// Shelf title.
    pub title: String,
    /// Items shown in the shelf.
    pub items: Vec<MediaItemSummary>,
}

/// Browser-facing home response.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct MediaHome {
    /// Library filter currently applied.
    pub library_id: Option<i32>,
    /// Kodi/Plex-style shelves for the main page.
    pub shelves: Vec<MediaShelf>,
    /// Real collection groupings derived from linked metadata.
    pub collections: Vec<MetadataCollectionSummary>,
}

/// Codec/container/format capabilities that a client declares to the server.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ClientProfile {
    /// Unique client type identifier (e.g. "web", "android", "desktop-win").
    pub client_type: String,
    /// Human-readable client name for logging/UI.
    pub client_name: String,
    /// Containers the client can play natively (e.g. ["mp4", "webm", "matroska"]).
    pub supported_containers: Vec<String>,
    /// Video codecs the client can decode (e.g. ["h264", "av1", "vp9", "hevc"]).
    pub supported_video_codecs: Vec<String>,
    /// Audio codecs the client can decode (e.g. ["aac", "opus", "mp3", "flac"]).
    pub supported_audio_codecs: Vec<String>,
    /// Subtitle formats the client can render (e.g. ["srt", "vtt", "ass"]).
    pub supported_subtitle_formats: Vec<String>,
    /// Maximum video width the client wants to receive (0 = no limit).
    pub max_video_width: u32,
    /// Maximum video height the client wants to receive (0 = no limit).
    pub max_video_height: u32,
    /// Maximum total bitrate in kbps the client wants to receive (0 = no limit).
    pub max_bitrate_kbps: u32,
    /// Whether the client can handle adaptive bitrate streams (HLS/DASH).
    pub supports_adaptive_streaming: bool,
    /// Whether the client prefers HLS over raw progressive download.
    pub prefer_hls: bool,
}

/// Direct-play versus transcode decision for one media item.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct PlaybackDecision {
    /// Stable database identifier for the item.
    pub item_id: i32,
    /// Whether the item can be played directly in the browser.
    pub can_direct_play: bool,
    /// Whether transcoding would be required for ideal playback.
    pub transcode_required: bool,
    /// Human-readable reason for the current decision.
    pub reason: String,
    /// Direct stream URL when direct play is supported.
    pub stream_url: Option<String>,
    /// Browser media MIME type when known.
    pub mime_type: Option<String>,
    /// When transcode is required, the target container.
    pub transcode_container: Option<String>,
    /// When transcode is required, the target video codec.
    pub transcode_video_codec: Option<String>,
    /// When transcode is required, the target audio codec.
    pub transcode_audio_codec: Option<String>,
    /// Whether only the video track needs transcoding.
    pub video_transcode_required: bool,
    /// Whether only the audio track needs transcoding.
    pub audio_transcode_required: bool,
    /// Source media info for display.
    pub source_video_codec: Option<String>,
    /// Source audio codec.
    pub source_audio_codec: Option<String>,
    /// Source container.
    pub source_container: Option<String>,
}

/// Server-managed playback session tracking one active transcode or direct-play stream.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct PlaybackSession {
    /// The unique identifier for this session.
    pub session_id: String,
    /// The ID of the item being played.
    pub item_id: i32,
    /// The user requesting playback, if any.
    pub user_id: Option<i32>,
    /// The client profile that initiated this session.
    pub client_profile: ClientProfile,
    /// The playback decision rendered for this session.
    pub decision: PlaybackDecision,
    /// Unix timestamp when the session was created.
    pub created_at: i64,
    /// Selected zero-based audio stream index among audio streams.
    pub audio_stream_index: Option<usize>,
}

/// One unmatched media item that is eligible for automatic metadata linking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomaticMetadataCandidate {
    /// Stable item identifier.
    pub item_id: i32,
    /// Library-relative media path.
    pub relative_path: String,
    /// Current display title derived from scan data.
    pub display_title: String,
    /// Last modification timestamp used to prioritize recent items.
    pub modified_at: Option<i64>,
    /// Owning library kind.
    pub library_kind: MediaLibraryKind,
    /// Metadata providers enabled for the library.
    pub metadata_providers: Vec<MetadataProviderId>,
}

#[derive(Debug, Default)]
struct FileCounters {
    total_files: u64,
    video_files: u64,
    audio_files: u64,
    image_files: u64,
    book_files: u64,
    other_files: u64,
}

#[derive(Debug, Clone)]
struct LibraryInspection {
    summary: LibraryScanSummary,
    files: Vec<DiscoveredMediaFile>,
    scanned_root_paths: HashSet<String>,
}

#[derive(Debug, Clone)]
struct DiscoveredMediaFile {
    full_path: PathBuf,
    source_root_path: String,
    relative_path: String,
    file_size: i64,
    modified_at: Option<i64>,
    media_kind: String,
    fingerprint_seed: String,
    default_title: String,
}

#[derive(Debug, Clone, diesel::QueryableByName)]
#[diesel(table_name = crate::db::schema::media_files)]
struct CatalogMediaFile {
    #[diesel(sql_type = sql_types::Integer)]
    id: i32,
    #[diesel(sql_type = sql_types::Integer)]
    library_file_id: i32,
    #[diesel(sql_type = sql_types::Integer)]
    library_id: i32,
    #[diesel(sql_type = sql_types::Text)]
    source_root_path: String,
    #[diesel(sql_type = sql_types::Text)]
    relative_path: String,
    #[diesel(sql_type = sql_types::BigInt)]
    file_size: i64,
    #[diesel(sql_type = sql_types::Nullable<sql_types::BigInt>)]
    modified_at: Option<i64>,
    #[diesel(sql_type = sql_types::Text)]
    media_kind: String,
    #[diesel(sql_type = sql_types::Text)]
    fingerprint_seed: String,
    #[diesel(sql_type = sql_types::Nullable<sql_types::Text>)]
    display_title: Option<String>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::Text>)]
    container: Option<String>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::BigInt>)]
    duration_ms: Option<i64>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::BigInt>)]
    bit_rate: Option<i64>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::Integer>)]
    width: Option<i32>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::Integer>)]
    height: Option<i32>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::Text>)]
    video_codec: Option<String>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::Text>)]
    audio_codec: Option<String>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::Text>)]
    metadata_json: Option<String>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::BigInt>)]
    metadata_updated_at: Option<i64>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::BigInt>)]
    metadata_match_attempted_at: Option<i64>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::Integer>)]
    media_item_id: Option<i32>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::BigInt>)]
    missing_since: Option<i64>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::BigInt>)]
    deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Default)]
struct ExtractedMetadata {
    container: Option<String>,
    duration_ms: Option<i64>,
    bit_rate: Option<i64>,
    width: Option<i32>,
    height: Option<i32>,
    video_codec: Option<String>,
    audio_codec: Option<String>,
    metadata_json: Option<String>,
    metadata_updated_at: Option<i64>,
}

#[derive(Debug, Clone)]
struct PlannedMediaItem {
    identity_key: String,
    parent_identity_key: Option<String>,
    item_type: String,
    display_title: String,
    relative_path: Option<String>,
    media_kind: Option<String>,
    season_number: Option<i32>,
    episode_number: Option<i32>,
    playable: bool,
    child_count: i32,
    file_size: Option<i64>,
    duration_ms: Option<i64>,
    modified_at: Option<i64>,
    explicit_id: Option<i32>,
    missing_since: Option<i64>,
    available_leaf_count: i32,
    missing_leaf_count: i32,
}

#[derive(Debug, Clone)]
struct PlannedLibraryItems {
    items: Vec<PlannedMediaItem>,
    leaf_identity_by_file_id: HashMap<i32, String>,
}

#[derive(Debug, Clone)]
struct ParsedShowPath {
    show_title: String,
    show_key: String,
    season_title: String,
    season_key: String,
    season_number: Option<i32>,
    episode_title: String,
    episode_key: String,
    episode_number: Option<i32>,
}

#[derive(Debug, Clone, Copy)]
struct ProbeContext<'a> {
    ffprobe_path: &'a str,
    enabled: bool,
}

/// Inspect configured media libraries and return lightweight scan summaries.
pub fn inspect_libraries(libraries: &[MediaLibrarySettings]) -> Vec<LibraryScanSummary> {
    libraries
        .iter()
        .map(inspect_library_with_inventory)
        .map(|inspection| inspection.summary)
        .collect()
}

/// Detect FFmpeg and ffprobe availability from the configured settings.
pub fn inspect_transcoding_capability(settings: &FfmpegSettings) -> TranscodingCapability {
    TranscodingCapability {
        ffmpeg: detect_binary(&settings.ffmpeg_path),
        ffprobe: detect_binary(&settings.ffprobe_path),
    }
}

/// Return the number of persisted media libraries.
pub fn count_persisted_libraries(
    conn: &mut SqliteConnection
) -> Result<i64, diesel::result::Error> {
    use crate::db::schema::media_libraries::dsl as media_libraries_dsl;

    media_libraries_dsl::media_libraries
        .count()
        .get_result(conn)
}

/// Ensure legacy library settings are imported into the database when needed.
pub fn migrate_legacy_library_settings(
    conn: &mut SqliteConnection,
    legacy_libraries: &[MediaLibrarySettings],
) -> Result<bool, diesel::result::Error> {
    if count_persisted_libraries(conn)? > 0 || legacy_libraries.is_empty() {
        return Ok(false);
    }

    for library in legacy_libraries {
        insert_media_library(conn, library)?;
    }

    Ok(true)
}

/// Return the persisted media-library settings stored in the database.
pub fn list_library_settings(
    conn: &mut SqliteConnection,
    legacy_libraries: &[MediaLibrarySettings],
) -> Result<Vec<MediaLibrarySettings>, diesel::result::Error> {
    use crate::db::schema::media_libraries::dsl as media_libraries_dsl;

    migrate_legacy_library_settings(conn, legacy_libraries)?;

    let rows = media_libraries_dsl::media_libraries
        .order(media_libraries_dsl::id.asc())
        .select(MediaLibrary::as_select())
        .load::<MediaLibrary>(conn)?;

    Ok(rows
        .into_iter()
        .map(media_library_settings_from_row)
        .collect())
}

/// Return metadata providers configured for a persisted library id.
pub fn get_library_metadata_providers(
    conn: &mut SqliteConnection,
    library_id: i32,
    legacy_libraries: &[MediaLibrarySettings],
) -> Result<Option<Vec<MetadataProviderId>>, diesel::result::Error> {
    use crate::db::schema::media_libraries::dsl as media_libraries_dsl;

    migrate_legacy_library_settings(conn, legacy_libraries)?;

    let library = media_libraries_dsl::media_libraries
        .filter(media_libraries_dsl::id.eq(library_id))
        .select(MediaLibrary::as_select())
        .first::<MediaLibrary>(conn)
        .optional()?;

    Ok(library.map(|row| media_library_settings_from_row(row).metadata_providers))
}

/// Return metadata languages configured for a persisted library id.
pub fn get_library_metadata_languages(
    conn: &mut SqliteConnection,
    library_id: i32,
    legacy_libraries: &[MediaLibrarySettings],
) -> Result<Option<Vec<String>>, diesel::result::Error> {
    use crate::db::schema::media_libraries::dsl as media_libraries_dsl;

    migrate_legacy_library_settings(conn, legacy_libraries)?;

    let library = media_libraries_dsl::media_libraries
        .filter(media_libraries_dsl::id.eq(library_id))
        .select(MediaLibrary::as_select())
        .first::<MediaLibrary>(conn)
        .optional()?;

    let Some(row) = library else {
        return Ok(None);
    };
    let settings = media_library_settings_from_row(row);
    if settings.metadata_language_mode == MediaLibraryMetadataLanguageMode::Manual {
        return Ok(Some(settings.metadata_languages));
    }

    Ok(Some(user_metadata_languages_for_library(
        conn,
        &settings.allowed_user_ids,
    )?))
}

fn user_metadata_languages_for_library(
    conn: &mut SqliteConnection,
    allowed_user_ids: &[i32],
) -> Result<Vec<String>, diesel::result::Error> {
    use crate::db::schema::users::dsl as users_dsl;

    let rows = users_dsl::users
        .select(User::as_select())
        .load::<User>(conn)?;
    let mut languages = Vec::new();
    for user in rows {
        let has_access =
            allowed_user_ids.is_empty() || user.admin || allowed_user_ids.contains(&user.id);
        if !has_access {
            continue;
        }
        let preferred =
            serde_json::from_str::<Vec<String>>(&user.preferred_metadata_languages_json)
                .unwrap_or_default();
        for language in preferred {
            let language = normalize_locale_key(&language);
            if !language.is_empty() && !languages.contains(&language) {
                languages.push(language);
            }
        }
    }
    if languages.is_empty() {
        languages.push(crate::metadata::DEFAULT_METADATA_LOCALE.to_string());
    }
    Ok(languages)
}

/// Return whether a user can view a library. Empty access lists are public.
pub fn user_can_access_library(
    conn: &mut SqliteConnection,
    library_id: i32,
    user_id: Option<i32>,
) -> Result<bool, diesel::result::Error> {
    use crate::db::schema::media_libraries::dsl as media_libraries_dsl;
    use crate::db::schema::users::dsl as users_dsl;

    let library = media_libraries_dsl::media_libraries
        .filter(media_libraries_dsl::id.eq(library_id))
        .select(MediaLibrary::as_select())
        .first::<MediaLibrary>(conn)
        .optional()?;
    let Some(library) = library else {
        return Ok(false);
    };
    let settings = media_library_settings_from_row(library);
    if settings.allowed_user_ids.is_empty() {
        return Ok(true);
    }
    let Some(user_id) = user_id else {
        return Ok(false);
    };
    let is_admin = users_dsl::users
        .filter(users_dsl::id.eq(user_id))
        .select(users_dsl::admin)
        .first::<bool>(conn)
        .optional()?
        .unwrap_or(false);
    Ok(is_admin || settings.allowed_user_ids.contains(&user_id))
}

/// Replace the persisted media-library settings stored in the database.
pub fn replace_library_settings(
    conn: &mut SqliteConnection,
    libraries: &[MediaLibrarySettings],
) -> Result<Vec<MediaLibrarySettings>, diesel::result::Error> {
    use crate::db::schema::media_libraries::dsl as media_libraries_dsl;

    let existing = media_libraries_dsl::media_libraries
        .order(media_libraries_dsl::id.asc())
        .select(MediaLibrary::as_select())
        .load::<MediaLibrary>(conn)?;

    for (index, library) in libraries.iter().enumerate() {
        if let Some(existing_row) = existing.get(index) {
            update_media_library(conn, existing_row.id, library)?;
        } else {
            insert_media_library(conn, library)?;
        }
    }

    for stale_library in existing.into_iter().skip(libraries.len()) {
        diesel::delete(
            media_libraries_dsl::media_libraries
                .filter(media_libraries_dsl::id.eq(stale_library.id)),
        )
        .execute(conn)?;
    }

    list_library_settings(conn, &[])
}

/// Insert one persisted media library.
pub fn add_library_setting(
    conn: &mut SqliteConnection,
    library: &MediaLibrarySettings,
) -> Result<Vec<MediaLibrarySettings>, diesel::result::Error> {
    insert_media_library(conn, library)?;
    list_library_settings(conn, &[])
}

/// Remove one persisted media library by its database identifier.
pub fn remove_library_setting(
    conn: &mut SqliteConnection,
    library_index: usize,
) -> Result<bool, diesel::result::Error> {
    use crate::db::schema::media_libraries::dsl as media_libraries_dsl;

    let existing = media_libraries_dsl::media_libraries
        .order(media_libraries_dsl::id.asc())
        .select(MediaLibrary::as_select())
        .load::<MediaLibrary>(conn)?;
    let Some(library_id) = existing.get(library_index).map(|library| library.id) else {
        return Ok(false);
    };

    let deleted = diesel::delete(
        media_libraries_dsl::media_libraries.filter(media_libraries_dsl::id.eq(library_id)),
    )
    .execute(conn)?;

    Ok(deleted > 0)
}

/// Sync persisted libraries from the database into the media catalog.
pub fn sync_persisted_library_catalog(
    conn: &mut SqliteConnection,
    legacy_libraries: &[MediaLibrarySettings],
    ffmpeg_settings: &FfmpegSettings,
) -> Result<Vec<PersistedLibrarySummary>, diesel::result::Error> {
    let libraries = list_library_settings(conn, legacy_libraries)?;
    sync_library_catalog(conn, &libraries, ffmpeg_settings)
}

/// Return persisted media-library summaries without triggering a foreground rescan.
pub fn get_persisted_library_summaries(
    conn: &mut SqliteConnection,
    legacy_libraries: &[MediaLibrarySettings],
) -> Result<Vec<PersistedLibrarySummary>, diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as item_metadata_links_dsl;
    use crate::db::schema::media_file_libraries::dsl as media_file_libraries_dsl;
    use crate::db::schema::media_items::dsl as media_items_dsl;
    use crate::db::schema::media_libraries::dsl as media_libraries_dsl;
    use crate::db::schema::scan_state::dsl as scan_state_dsl;

    migrate_legacy_library_settings(conn, legacy_libraries)?;

    let libraries = media_libraries_dsl::media_libraries
        .order(media_libraries_dsl::id.asc())
        .select(MediaLibrary::as_select())
        .load::<MediaLibrary>(conn)?;
    let states = scan_state_dsl::scan_state
        .select(ScanState::as_select())
        .load::<ScanState>(conn)?
        .into_iter()
        .map(|state| (state.library_id, state))
        .collect::<HashMap<_, _>>();
    let item_library_ids = if libraries.is_empty() {
        HashMap::new()
    } else {
        media_items_dsl::media_items
            .filter(
                media_items_dsl::library_id.eq_any(
                    libraries
                        .iter()
                        .map(|library| library.id)
                        .collect::<Vec<_>>(),
                ),
            )
            .filter(media_items_dsl::deleted_at.is_null())
            .select(MediaItem::as_select())
            .load::<MediaItem>(conn)?
            .into_iter()
            .map(|item| (item.id, item.library_id))
            .collect::<HashMap<_, _>>()
    };
    let refresh_counts = if item_library_ids.is_empty() {
        HashMap::new()
    } else {
        item_metadata_links_dsl::item_metadata_links
            .filter(
                item_metadata_links_dsl::media_item_id
                    .eq_any(item_library_ids.keys().copied().collect::<Vec<_>>()),
            )
            .filter(item_metadata_links_dsl::relation_kind.eq("primary"))
            .select(ItemMetadataLink::as_select())
            .load::<ItemMetadataLink>(conn)?
            .into_iter()
            .fold(
                HashMap::<i32, LibraryMetadataRefreshCounts>::new(),
                |mut grouped, link| {
                    let Some(library_id) = item_library_ids.get(&link.media_item_id).copied()
                    else {
                        return grouped;
                    };

                    let counts = grouped.entry(library_id).or_default();
                    counts.total_items += 1;
                    if link.refresh_state == "pending" {
                        counts.pending_items += 1;
                    } else {
                        counts.completed_items += 1;
                        if link.refresh_state == "error" {
                            counts.failed_items += 1;
                        }
                    }

                    grouped
                },
            )
    };
    let library_ids = libraries
        .iter()
        .map(|library| library.id)
        .collect::<Vec<_>>();
    let missing_files_by_library = if library_ids.is_empty() {
        HashMap::new()
    } else {
        media_file_libraries_dsl::media_file_libraries
            .filter(media_file_libraries_dsl::library_id.eq_any(&library_ids))
            .filter(media_file_libraries_dsl::deleted_at.is_null())
            .filter(media_file_libraries_dsl::missing_since.is_not_null())
            .select(media_file_libraries_dsl::library_id)
            .load::<i32>(conn)?
            .into_iter()
            .fold(HashMap::<i32, i64>::new(), |mut counts, library_id| {
                *counts.entry(library_id).or_default() += 1;
                counts
            })
    };
    let missing_items_by_library = if library_ids.is_empty() {
        HashMap::new()
    } else {
        media_items_dsl::media_items
            .filter(media_items_dsl::library_id.eq_any(&library_ids))
            .filter(media_items_dsl::deleted_at.is_null())
            .filter(media_items_dsl::missing_since.is_not_null())
            .select(media_items_dsl::library_id)
            .load::<i32>(conn)?
            .into_iter()
            .fold(HashMap::<i32, i64>::new(), |mut counts, library_id| {
                *counts.entry(library_id).or_default() += 1;
                counts
            })
    };

    Ok(libraries
        .into_iter()
        .map(|library| {
            let settings = media_library_settings_from_row(library.clone());
            let state = states.get(&library.id);
            let metadata_counts = refresh_counts.get(&library.id).cloned().unwrap_or_default();
            PersistedLibrarySummary {
                id: library.id,
                name: settings.name,
                path: settings.path,
                paths: settings.paths,
                recursive: settings.recursive,
                kind: settings.kind,
                metadata_providers: settings.metadata_providers,
                metadata_language_mode: settings.metadata_language_mode,
                metadata_languages: settings.metadata_languages,
                status: state
                    .map(|state| LibraryScanStatus::from_storage_value(&state.last_status))
                    .unwrap_or(LibraryScanStatus::NeverScanned),
                scan_revision: state.map(|state| state.scan_revision).unwrap_or_default(),
                last_scanned_at: state.and_then(|state| state.last_scanned_at),
                total_files: state.map(|state| state.total_files).unwrap_or_default(),
                video_files: state.map(|state| state.video_files).unwrap_or_default(),
                audio_files: state.map(|state| state.audio_files).unwrap_or_default(),
                image_files: state.map(|state| state.image_files).unwrap_or_default(),
                book_files: state.map(|state| state.book_files).unwrap_or_default(),
                other_files: state.map(|state| state.other_files).unwrap_or_default(),
                error: state.and_then(|state| state.last_error.clone()),
                metadata_refresh_total: metadata_counts.total_items,
                metadata_refresh_pending: metadata_counts.pending_items,
                metadata_refresh_completed: metadata_counts.completed_items,
                metadata_refresh_failed: metadata_counts.failed_items,
                missing_files: missing_files_by_library
                    .get(&library.id)
                    .copied()
                    .unwrap_or_default(),
                missing_items: missing_items_by_library
                    .get(&library.id)
                    .copied()
                    .unwrap_or_default(),
            }
        })
        .collect())
}

fn load_catalog_files_for_library(
    conn: &mut SqliteConnection,
    library_id: i32,
    include_deleted: bool,
) -> Result<Vec<CatalogMediaFile>, diesel::result::Error> {
    let deleted_filter = if include_deleted { "" } else { " AND memberships.deleted_at IS NULL" };
    let sql = format!(
        "SELECT {CATALOG_MEDIA_FILE_COLUMNS} \
         FROM media_file_libraries AS memberships \
         INNER JOIN media_files AS files ON files.id = memberships.media_file_id \
         WHERE memberships.library_id = ?{deleted_filter} \
         ORDER BY memberships.relative_path ASC"
    );
    diesel::sql_query(sql)
        .bind::<sql_types::Integer, _>(library_id)
        .load::<CatalogMediaFile>(conn)
}

fn load_active_catalog_files(
    conn: &mut SqliteConnection
) -> Result<Vec<CatalogMediaFile>, diesel::result::Error> {
    let sql = format!(
        "SELECT {CATALOG_MEDIA_FILE_COLUMNS} \
         FROM media_file_libraries AS memberships \
         INNER JOIN media_files AS files ON files.id = memberships.media_file_id \
         WHERE memberships.deleted_at IS NULL \
           AND memberships.missing_since IS NULL \
         ORDER BY files.modified_at DESC, files.id ASC"
    );
    diesel::sql_query(sql).load::<CatalogMediaFile>(conn)
}

fn load_catalog_file_for_item(
    conn: &mut SqliteConnection,
    item_id: i32,
) -> Result<Option<CatalogMediaFile>, diesel::result::Error> {
    let sql = format!(
        "SELECT {CATALOG_MEDIA_FILE_COLUMNS} \
         FROM media_file_libraries AS memberships \
         INNER JOIN media_files AS files ON files.id = memberships.media_file_id \
         WHERE memberships.media_item_id = ? \
           AND memberships.deleted_at IS NULL \
         ORDER BY memberships.id ASC \
         LIMIT 1"
    );
    diesel::sql_query(sql)
        .bind::<sql_types::Integer, _>(item_id)
        .load::<CatalogMediaFile>(conn)
        .map(|mut rows| rows.pop())
}

fn load_media_file_by_path(
    conn: &mut SqliteConnection,
    path: &str,
) -> Result<Option<MediaFile>, diesel::result::Error> {
    use crate::db::schema::media_files::dsl as media_files_dsl;

    media_files_dsl::media_files
        .filter(media_files_dsl::path.eq(path))
        .select(MediaFile::as_select())
        .first(conn)
        .optional()
}

/// Sync configured libraries into the persistent catalog and refresh their inventory.
pub fn sync_library_catalog(
    conn: &mut SqliteConnection,
    libraries: &[MediaLibrarySettings],
    ffmpeg_settings: &FfmpegSettings,
) -> Result<Vec<PersistedLibrarySummary>, diesel::result::Error> {
    use crate::db::schema::media_file_libraries::dsl as media_file_libraries_dsl;
    use crate::db::schema::media_files::dsl as media_files_dsl;
    use crate::db::schema::media_libraries::dsl as media_libraries_dsl;
    use crate::db::schema::scan_state::dsl as scan_state_dsl;

    let probe_context = ProbeContext {
        ffprobe_path: &ffmpeg_settings.ffprobe_path,
        enabled: detect_binary(&ffmpeg_settings.ffprobe_path).available,
    };
    let inspections: Vec<LibraryInspection> = libraries
        .iter()
        .map(inspect_library_with_inventory)
        .collect();
    let existing_library_rows = media_libraries_dsl::media_libraries
        .order(media_libraries_dsl::id.asc())
        .select(MediaLibrary::as_select())
        .load::<MediaLibrary>(conn)?;
    let mut persisted = Vec::with_capacity(inspections.len());

    for (index, (library, inspection)) in libraries.iter().zip(inspections.into_iter()).enumerate()
    {
        let library_values = NewMediaLibrary {
            name: inspection.summary.name.clone(),
            path: inspection.summary.path.clone(),
            paths_json: serde_json::to_string(&inspection.summary.paths)
                .unwrap_or_else(|_| "[]".into()),
            kind: inspection.summary.kind.as_storage_value(),
            recursive: inspection.summary.recursive,
            metadata_providers_json: serde_json::to_string(
                &library
                    .metadata_providers
                    .iter()
                    .map(|provider| provider.as_storage_value())
                    .collect::<Vec<_>>(),
            )
            .unwrap_or_else(|_| "[\"tmdb\"]".into()),
            metadata_language_mode: match library.metadata_language_mode {
                MediaLibraryMetadataLanguageMode::Auto => "auto",
                MediaLibraryMetadataLanguageMode::Manual => "manual",
            }
            .into(),
            metadata_languages_json: serde_json::to_string(&library.metadata_languages)
                .unwrap_or_else(|_| "[\"en-US\"]".into()),
            allowed_user_ids_json: serde_json::to_string(&library.allowed_user_ids)
                .unwrap_or_else(|_| "[]".into()),
        };

        let existing_library = existing_library_rows.get(index).cloned();

        let library_row = if let Some(existing_library) = existing_library {
            diesel::update(
                media_libraries_dsl::media_libraries
                    .filter(media_libraries_dsl::id.eq(existing_library.id)),
            )
            .set(&library_values)
            .execute(conn)?;

            media_libraries_dsl::media_libraries
                .filter(media_libraries_dsl::id.eq(existing_library.id))
                .select(MediaLibrary::as_select())
                .first(conn)?
        } else {
            diesel::insert_into(media_libraries_dsl::media_libraries)
                .values(&library_values)
                .execute(conn)?;

            media_libraries_dsl::media_libraries
                .order(media_libraries_dsl::id.desc())
                .select(MediaLibrary::as_select())
                .first(conn)?
        };

        let existing_state = scan_state_dsl::scan_state
            .filter(scan_state_dsl::library_id.eq(library_row.id))
            .select(ScanState::as_select())
            .first(conn)
            .optional()?;
        let next_scan_revision = existing_state
            .as_ref()
            .map(|state| state.scan_revision + 1)
            .unwrap_or(1);
        let last_scanned_at = Some(current_timestamp());

        let existing_files = load_catalog_files_for_library(conn, library_row.id, true)?;
        let mut existing_files_by_path: HashMap<String, CatalogMediaFile> = existing_files
            .into_iter()
            .map(|file| (media_file_inventory_key(&file), file))
            .collect();
        let mut seen_paths = HashSet::new();

        for discovered_file in &inspection.files {
            let inventory_key = discovered_media_file_inventory_key(discovered_file);
            if !seen_paths.insert(inventory_key.clone()) {
                continue;
            }

            let existing_file = existing_files_by_path.remove(&inventory_key);
            let physical_path = discovered_file.physical_path();
            let existing_physical_file = load_media_file_by_path(conn, &physical_path)?;
            let should_refresh_title = existing_file
                .as_ref()
                .map(|file| {
                    file.display_title.as_deref() != Some(discovered_file.default_title.as_str())
                })
                .unwrap_or(true);
            let should_refresh_metadata = existing_file
                .as_ref()
                .map(|file| file.fingerprint_seed != discovered_file.fingerprint_seed)
                .or_else(|| {
                    existing_physical_file
                        .as_ref()
                        .map(|file| file.fingerprint_seed != discovered_file.fingerprint_seed)
                })
                .unwrap_or(true);
            let should_restore_file = existing_file
                .as_ref()
                .map(|file| file.missing_since.is_some() || file.deleted_at.is_some())
                .unwrap_or(false);

            let metadata = if should_refresh_metadata {
                extract_metadata(discovered_file, probe_context)
            } else {
                existing_file
                    .as_ref()
                    .map(extracted_metadata_from_existing)
                    .or_else(|| {
                        existing_physical_file
                            .as_ref()
                            .map(extracted_metadata_from_file_row)
                    })
                    .unwrap_or_else(|| default_metadata(discovered_file))
            };
            let display_title = Some(discovered_file.default_title.clone());
            let metadata_match_attempted_at = if should_refresh_metadata || should_refresh_title {
                None
            } else {
                existing_file
                    .as_ref()
                    .and_then(|file| file.metadata_match_attempted_at)
            };
            let file_values = discovered_file.to_new_media_file(metadata);
            let media_file_id = if let Some(existing_physical_file) = existing_physical_file {
                if existing_physical_file.fingerprint_seed != discovered_file.fingerprint_seed {
                    diesel::update(
                        media_files_dsl::media_files
                            .filter(media_files_dsl::id.eq(existing_physical_file.id)),
                    )
                    .set(&file_values)
                    .execute(conn)?;
                    diesel::update(media_file_libraries_dsl::media_file_libraries.filter(
                        media_file_libraries_dsl::media_file_id.eq(existing_physical_file.id),
                    ))
                    .set(
                        media_file_libraries_dsl::metadata_match_attempted_at
                            .eq::<Option<i64>>(None),
                    )
                    .execute(conn)?;
                }
                existing_physical_file.id
            } else {
                diesel::insert_into(media_files_dsl::media_files)
                    .values(&file_values)
                    .execute(conn)?;
                media_files_dsl::media_files
                    .filter(media_files_dsl::path.eq(&physical_path))
                    .select(media_files_dsl::id)
                    .first::<i32>(conn)?
            };
            let membership_values = discovered_file.to_new_media_file_library(
                media_file_id,
                library_row.id,
                display_title,
                metadata_match_attempted_at,
            );

            if let Some(existing_file) = existing_file {
                if existing_file.fingerprint_seed != discovered_file.fingerprint_seed
                    || should_refresh_title
                    || should_restore_file
                    || existing_file.id != media_file_id
                {
                    diesel::update(
                        media_file_libraries_dsl::media_file_libraries
                            .filter(media_file_libraries_dsl::id.eq(existing_file.library_file_id)),
                    )
                    .set(&membership_values)
                    .execute(conn)?;
                }
            } else {
                diesel::insert_into(media_file_libraries_dsl::media_file_libraries)
                    .values(&membership_values)
                    .execute(conn)?;
            };
        }

        let missing_file_ids: Vec<i32> = existing_files_by_path
            .values()
            .filter(|file| file.deleted_at.is_none())
            .map(|file| file.library_file_id)
            .collect();
        let missing_file_count = missing_file_ids.len();
        if !missing_file_ids.is_empty() {
            diesel::update(
                media_file_libraries_dsl::media_file_libraries
                    .filter(media_file_libraries_dsl::id.eq_any(missing_file_ids))
                    .filter(media_file_libraries_dsl::missing_since.is_null()),
            )
            .set(media_file_libraries_dsl::missing_since.eq(current_timestamp()))
            .execute(conn)?;
        }

        if inspection.scanned_root_paths.is_empty() {
            if missing_file_count > 0 {
                log::warn!(
                    "No configured roots were scanned successfully for library {} ({}); marked {} existing file rows as missing",
                    library_row.id,
                    library_row.name,
                    missing_file_count
                );
            }
        } else {
            let missing_unscanned_files = existing_files_by_path
                .values()
                .filter(|file| {
                    !inspection
                        .scanned_root_paths
                        .contains(&file.source_root_path)
                })
                .count();
            if missing_unscanned_files > 0 {
                log::warn!(
                    "Marked {} existing file rows as missing in library {} ({}) because their source roots were not scanned successfully",
                    missing_unscanned_files,
                    library_row.id,
                    library_row.name
                );
            }
        }
        sync_logical_media_items_for_library(conn, &library_row, &inspection.summary.kind)?;

        let state_values = NewScanState {
            library_id: library_row.id,
            last_status: inspection.summary.status.as_storage_value().to_string(),
            last_error: inspection.summary.error.clone(),
            scan_revision: next_scan_revision,
            last_scanned_at,
            total_files: inspection.summary.total_files as i64,
            video_files: inspection.summary.video_files as i64,
            audio_files: inspection.summary.audio_files as i64,
            image_files: inspection.summary.image_files as i64,
            book_files: inspection.summary.book_files as i64,
            other_files: inspection.summary.other_files as i64,
        };

        if let Some(existing_state) = existing_state {
            diesel::update(
                scan_state_dsl::scan_state.filter(scan_state_dsl::id.eq(existing_state.id)),
            )
            .set(&state_values)
            .execute(conn)?;
        } else {
            diesel::insert_into(scan_state_dsl::scan_state)
                .values(&state_values)
                .execute(conn)?;
        }

        persisted.push(PersistedLibrarySummary {
            id: library_row.id,
            name: inspection.summary.name,
            path: inspection.summary.path,
            paths: inspection.summary.paths,
            recursive: inspection.summary.recursive,
            kind: inspection.summary.kind,
            metadata_providers: library.metadata_providers.clone(),
            metadata_language_mode: library.metadata_language_mode.clone(),
            metadata_languages: library.metadata_languages.clone(),
            status: inspection.summary.status,
            scan_revision: next_scan_revision,
            last_scanned_at,
            total_files: state_values.total_files,
            video_files: state_values.video_files,
            audio_files: state_values.audio_files,
            image_files: state_values.image_files,
            book_files: state_values.book_files,
            other_files: state_values.other_files,
            error: state_values.last_error,
            metadata_refresh_total: 0,
            metadata_refresh_pending: 0,
            metadata_refresh_completed: 0,
            metadata_refresh_failed: 0,
            missing_files: 0,
            missing_items: 0,
        });
    }

    Ok(persisted)
}

fn sync_logical_media_items_for_library(
    conn: &mut SqliteConnection,
    library: &MediaLibrary,
    library_kind: &MediaLibraryKind,
) -> Result<(), diesel::result::Error> {
    use crate::db::schema::media_file_libraries::dsl as media_file_libraries_dsl;
    use crate::db::schema::media_items::dsl as media_items_dsl;

    let files = load_catalog_files_for_library(conn, library.id, false)?;
    let existing_items = media_items_dsl::media_items
        .filter(media_items_dsl::library_id.eq(library.id))
        .select(MediaItem::as_select())
        .load::<MediaItem>(conn)?;

    let planned = plan_library_media_items(&files, library_kind, library.id);
    let planned_keys = planned
        .items
        .iter()
        .map(|item| item.identity_key.clone())
        .collect::<HashSet<_>>();
    let mut existing_by_key = existing_items
        .iter()
        .cloned()
        .map(|item| (item.identity_key.clone(), item))
        .collect::<HashMap<_, _>>();
    let existing_by_id = existing_items
        .iter()
        .cloned()
        .map(|item| (item.id, item))
        .collect::<HashMap<_, _>>();
    let mut item_ids_by_key = HashMap::new();

    for plan in planned
        .items
        .iter()
        .filter(|item| item.parent_identity_key.is_none())
    {
        upsert_planned_media_item(
            conn,
            library.id,
            plan,
            None,
            &mut existing_by_key,
            &existing_by_id,
            &mut item_ids_by_key,
        )?;
    }

    for plan in planned
        .items
        .iter()
        .filter(|item| item.parent_identity_key.is_some())
    {
        let parent_id = plan
            .parent_identity_key
            .as_ref()
            .and_then(|identity_key| item_ids_by_key.get(identity_key))
            .copied();
        upsert_planned_media_item(
            conn,
            library.id,
            plan,
            parent_id,
            &mut existing_by_key,
            &existing_by_id,
            &mut item_ids_by_key,
        )?;
    }

    for file in &files {
        let Some(identity_key) = planned.leaf_identity_by_file_id.get(&file.library_file_id) else {
            continue;
        };
        let Some(item_id) = item_ids_by_key.get(identity_key).copied() else {
            continue;
        };
        diesel::update(
            media_file_libraries_dsl::media_file_libraries
                .filter(media_file_libraries_dsl::id.eq(file.library_file_id)),
        )
        .set(media_file_libraries_dsl::media_item_id.eq(item_id))
        .execute(conn)?;
    }

    let stale_ids = existing_items
        .into_iter()
        .filter(|item| !planned_keys.contains(&item.identity_key))
        .filter(|item| item.deleted_at.is_none())
        .map(|item| item.id)
        .collect::<Vec<_>>();
    if !stale_ids.is_empty() {
        mark_media_items_deleted(conn, &stale_ids, current_timestamp())?;
    }

    Ok(())
}

/// Mark missing media files and items as deleted from the active catalog.
pub fn delete_missing_media_items(
    conn: &mut SqliteConnection,
    library_id: Option<i32>,
    missing_before_or_at: Option<i64>,
) -> Result<MissingItemsCleanupSummary, diesel::result::Error> {
    use crate::db::schema::media_file_libraries::dsl as media_file_libraries_dsl;
    use crate::db::schema::media_items::dsl as media_items_dsl;
    use crate::db::schema::metadata_collection_items::dsl as collection_items_dsl;

    let mut file_query = media_file_libraries_dsl::media_file_libraries
        .filter(media_file_libraries_dsl::deleted_at.is_null())
        .filter(media_file_libraries_dsl::missing_since.is_not_null())
        .into_boxed();
    let mut item_query = media_items_dsl::media_items
        .filter(media_items_dsl::deleted_at.is_null())
        .filter(media_items_dsl::missing_since.is_not_null())
        .into_boxed();

    if let Some(library_id) = library_id {
        file_query = file_query.filter(media_file_libraries_dsl::library_id.eq(library_id));
        item_query = item_query.filter(media_items_dsl::library_id.eq(library_id));
    }
    if let Some(cutoff) = missing_before_or_at {
        file_query = file_query.filter(media_file_libraries_dsl::missing_since.le(cutoff));
        item_query = item_query.filter(media_items_dsl::missing_since.le(cutoff));
    }

    let file_ids = file_query
        .select(media_file_libraries_dsl::id)
        .load::<i32>(conn)?;
    let item_ids = item_query.select(media_items_dsl::id).load::<i32>(conn)?;

    let deleted_at = current_timestamp();
    let deleted_files = if file_ids.is_empty() {
        0
    } else {
        diesel::update(
            media_file_libraries_dsl::media_file_libraries
                .filter(media_file_libraries_dsl::id.eq_any(&file_ids)),
        )
        .set(media_file_libraries_dsl::deleted_at.eq(deleted_at))
        .execute(conn)? as i64
    };

    let removed_collection_items = if item_ids.is_empty() {
        0
    } else {
        diesel::delete(
            collection_items_dsl::metadata_collection_items
                .filter(collection_items_dsl::media_item_id.eq_any(&item_ids)),
        )
        .execute(conn)? as i64
    };

    let deleted_items = if item_ids.is_empty() {
        0
    } else {
        diesel::update(media_items_dsl::media_items.filter(media_items_dsl::id.eq_any(&item_ids)))
            .set(media_items_dsl::deleted_at.eq(deleted_at))
            .execute(conn)? as i64
    };

    Ok(MissingItemsCleanupSummary {
        library_id,
        deleted_files,
        deleted_items,
        removed_collection_items,
    })
}

fn mark_media_items_deleted(
    conn: &mut SqliteConnection,
    item_ids: &[i32],
    deleted_at: i64,
) -> Result<usize, diesel::result::Error> {
    use crate::db::schema::media_items::dsl as media_items_dsl;
    use crate::db::schema::metadata_collection_items::dsl as collection_items_dsl;

    if item_ids.is_empty() {
        return Ok(0);
    }

    diesel::delete(
        collection_items_dsl::metadata_collection_items
            .filter(collection_items_dsl::media_item_id.eq_any(item_ids)),
    )
    .execute(conn)?;

    diesel::update(media_items_dsl::media_items.filter(media_items_dsl::id.eq_any(item_ids)))
        .set(media_items_dsl::deleted_at.eq(deleted_at))
        .execute(conn)
}

fn upsert_planned_media_item(
    conn: &mut SqliteConnection,
    library_id: i32,
    plan: &PlannedMediaItem,
    parent_id: Option<i32>,
    existing_by_key: &mut HashMap<String, MediaItem>,
    existing_by_id: &HashMap<i32, MediaItem>,
    item_ids_by_key: &mut HashMap<String, i32>,
) -> Result<(), diesel::result::Error> {
    use crate::db::schema::media_items::dsl as media_items_dsl;

    let values = NewMediaItem {
        library_id,
        parent_id,
        identity_key: plan.identity_key.clone(),
        item_type: plan.item_type.clone(),
        display_title: plan.display_title.clone(),
        relative_path: plan.relative_path.clone(),
        media_kind: plan.media_kind.clone(),
        season_number: plan.season_number,
        episode_number: plan.episode_number,
        child_count: plan.child_count,
        playable: plan.playable,
        file_size: plan.file_size,
        duration_ms: plan.duration_ms,
        modified_at: plan.modified_at,
        created_at: plan.modified_at.or_else(|| Some(current_timestamp())),
        updated_at: Some(current_timestamp()),
        missing_since: plan.missing_since,
        deleted_at: None,
    };

    let target = existing_by_key.remove(&plan.identity_key).or_else(|| {
        plan.explicit_id
            .and_then(|id| existing_by_id.get(&id).cloned())
    });

    let item_id = if let Some(existing) = target {
        diesel::update(media_items_dsl::media_items.filter(media_items_dsl::id.eq(existing.id)))
            .set(&values)
            .execute(conn)?;
        existing.id
    } else if let Some(explicit_id) = plan.explicit_id {
        let explicit_id_available = media_items_dsl::media_items
            .filter(media_items_dsl::id.eq(explicit_id))
            .select(media_items_dsl::id)
            .first::<i32>(conn)
            .optional()?
            .is_none();

        if explicit_id_available {
            diesel::insert_into(media_items_dsl::media_items)
                .values((media_items_dsl::id.eq(explicit_id), &values))
                .execute(conn)?;
            explicit_id
        } else {
            diesel::insert_into(media_items_dsl::media_items)
                .values(&values)
                .execute(conn)?;
            media_items_dsl::media_items
                .filter(media_items_dsl::identity_key.eq(&plan.identity_key))
                .select(media_items_dsl::id)
                .first::<i32>(conn)?
        }
    } else {
        diesel::insert_into(media_items_dsl::media_items)
            .values(&values)
            .execute(conn)?;
        media_items_dsl::media_items
            .filter(media_items_dsl::identity_key.eq(&plan.identity_key))
            .select(media_items_dsl::id)
            .first::<i32>(conn)?
    };

    item_ids_by_key.insert(plan.identity_key.clone(), item_id);
    Ok(())
}

fn plan_library_media_items(
    files: &[CatalogMediaFile],
    library_kind: &MediaLibraryKind,
    library_id: i32,
) -> PlannedLibraryItems {
    let mut items_by_key = HashMap::<String, PlannedMediaItem>::new();
    let mut leaf_identity_by_file_id = HashMap::new();

    for file in files {
        let available_leaf_count = if file.missing_since.is_none() { 1 } else { 0 };
        let missing_leaf_count = if file.missing_since.is_some() { 1 } else { 0 };
        if *library_kind == MediaLibraryKind::Shows && file.media_kind == "video" {
            let fallback_title = fallback_title_from_relative_path(&file.relative_path);
            let parsed = parse_show_path(
                &file.relative_path,
                file.display_title
                    .as_deref()
                    .unwrap_or(fallback_title.as_str()),
                library_id,
            );
            upsert_planned_item(
                &mut items_by_key,
                PlannedMediaItem {
                    identity_key: parsed.show_key.clone(),
                    parent_identity_key: None,
                    item_type: "show".into(),
                    display_title: parsed.show_title.clone(),
                    relative_path: parent_relative_path(&file.relative_path, 1),
                    media_kind: Some("video".into()),
                    season_number: None,
                    episode_number: None,
                    playable: false,
                    child_count: 0,
                    file_size: None,
                    duration_ms: None,
                    modified_at: file.modified_at,
                    explicit_id: None,
                    missing_since: file.missing_since,
                    available_leaf_count,
                    missing_leaf_count,
                },
            );
            upsert_planned_item(
                &mut items_by_key,
                PlannedMediaItem {
                    identity_key: parsed.season_key.clone(),
                    parent_identity_key: Some(parsed.show_key.clone()),
                    item_type: "season".into(),
                    display_title: parsed.season_title.clone(),
                    relative_path: parent_relative_path(&file.relative_path, 2),
                    media_kind: Some("video".into()),
                    season_number: parsed.season_number,
                    episode_number: None,
                    playable: false,
                    child_count: 0,
                    file_size: None,
                    duration_ms: None,
                    modified_at: file.modified_at,
                    explicit_id: None,
                    missing_since: file.missing_since,
                    available_leaf_count,
                    missing_leaf_count,
                },
            );
            upsert_planned_item(
                &mut items_by_key,
                PlannedMediaItem {
                    identity_key: parsed.episode_key.clone(),
                    parent_identity_key: Some(parsed.season_key.clone()),
                    item_type: "episode".into(),
                    display_title: parsed.episode_title.clone(),
                    relative_path: Some(file.relative_path.clone()),
                    media_kind: Some(file.media_kind.clone()),
                    season_number: parsed.season_number,
                    episode_number: parsed.episode_number,
                    playable: true,
                    child_count: 0,
                    file_size: Some(file.file_size),
                    duration_ms: file.duration_ms,
                    modified_at: file.modified_at,
                    explicit_id: Some(file.library_file_id),
                    missing_since: file.missing_since,
                    available_leaf_count,
                    missing_leaf_count,
                },
            );
            leaf_identity_by_file_id.insert(file.library_file_id, parsed.episode_key);
            continue;
        }

        let item_type = match file.media_kind.as_str() {
            "audio" => "track",
            "image" => "photo",
            "book" => "book",
            _ => "movie",
        };
        let identity_key = format!("file:{}", file.library_file_id);
        upsert_planned_item(
            &mut items_by_key,
            PlannedMediaItem {
                identity_key: identity_key.clone(),
                parent_identity_key: None,
                item_type: item_type.into(),
                display_title: file
                    .display_title
                    .clone()
                    .unwrap_or_else(|| fallback_title_from_relative_path(&file.relative_path)),
                relative_path: Some(file.relative_path.clone()),
                media_kind: Some(file.media_kind.clone()),
                season_number: None,
                episode_number: None,
                playable: matches!(file.media_kind.as_str(), "video" | "audio"),
                child_count: 0,
                file_size: Some(file.file_size),
                duration_ms: file.duration_ms,
                modified_at: file.modified_at,
                explicit_id: Some(file.library_file_id),
                missing_since: file.missing_since,
                available_leaf_count,
                missing_leaf_count,
            },
        );
        leaf_identity_by_file_id.insert(file.library_file_id, identity_key);
    }

    let mut child_counts = HashMap::<String, i32>::new();
    for item in items_by_key.values() {
        if let Some(parent_identity_key) = &item.parent_identity_key {
            *child_counts.entry(parent_identity_key.clone()).or_default() += 1;
        }
    }
    for item in items_by_key.values_mut() {
        item.child_count = child_counts
            .get(&item.identity_key)
            .copied()
            .unwrap_or_default();
    }

    let depth_by_key = items_by_key
        .keys()
        .map(|identity_key| {
            (
                identity_key.clone(),
                item_depth(identity_key, &items_by_key),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut items = items_by_key.into_values().collect::<Vec<_>>();
    items.sort_by(|left, right| {
        depth_by_key
            .get(&left.identity_key)
            .copied()
            .unwrap_or_default()
            .cmp(
                &depth_by_key
                    .get(&right.identity_key)
                    .copied()
                    .unwrap_or_default(),
            )
            .then_with(|| left.season_number.cmp(&right.season_number))
            .then_with(|| left.episode_number.cmp(&right.episode_number))
            .then_with(|| left.display_title.cmp(&right.display_title))
    });

    PlannedLibraryItems {
        items,
        leaf_identity_by_file_id,
    }
}

fn upsert_planned_item(
    items_by_key: &mut HashMap<String, PlannedMediaItem>,
    item: PlannedMediaItem,
) {
    items_by_key
        .entry(item.identity_key.clone())
        .and_modify(|existing| {
            existing.modified_at = existing.modified_at.max(item.modified_at);
            existing.available_leaf_count += item.available_leaf_count;
            existing.missing_leaf_count += item.missing_leaf_count;
            existing.missing_since = if existing.available_leaf_count > 0 {
                None
            } else {
                match (existing.missing_since, item.missing_since) {
                    (Some(left), Some(right)) => Some(left.min(right)),
                    (Some(left), None) => Some(left),
                    (None, Some(right)) => Some(right),
                    (None, None) => None,
                }
            };
            existing.duration_ms = Some(
                existing.duration_ms.unwrap_or_default() + item.duration_ms.unwrap_or_default(),
            )
            .filter(|value| *value > 0);
            existing.file_size = match (existing.file_size, item.file_size) {
                (Some(left), Some(right)) => Some(left + right),
                (Some(left), None) => Some(left),
                (None, Some(right)) => Some(right),
                (None, None) => None,
            };
            if existing.display_title.trim().is_empty() {
                existing.display_title = item.display_title.clone();
            }
        })
        .or_insert(item);
}

fn item_depth(
    identity_key: &str,
    items_by_key: &HashMap<String, PlannedMediaItem>,
) -> usize {
    let mut depth = 0;
    let mut next_parent = items_by_key
        .get(identity_key)
        .and_then(|item| item.parent_identity_key.as_deref());

    while let Some(parent_identity_key) = next_parent {
        depth += 1;
        next_parent = items_by_key
            .get(parent_identity_key)
            .and_then(|item| item.parent_identity_key.as_deref());
    }

    depth
}

fn parent_relative_path(
    relative_path: &str,
    depth: usize,
) -> Option<String> {
    let parts = relative_path
        .replace('\\', "/")
        .split('/')
        .filter(|part| !part.trim().is_empty())
        .take(depth)
        .map(str::to_string)
        .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn parse_show_path(
    relative_path: &str,
    fallback_title: &str,
    library_id: i32,
) -> ParsedShowPath {
    let normalized = relative_path.replace('\\', "/");
    let parts = normalized
        .split('/')
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>();
    let show_title = parts
        .first()
        .copied()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback_title)
        .trim()
        .to_string();
    let season_source =
        if parts.len() >= 2 { parts[parts.len().saturating_sub(2)] } else { fallback_title };
    let season_number = infer_season_number(season_source)
        .or_else(|| infer_season_number(fallback_title))
        .filter(|value| *value > 0);
    let episode_number = infer_episode_number(fallback_title)
        .or_else(|| infer_episode_number(parts.last().copied().unwrap_or_default()))
        .filter(|value| *value > 0);
    let episode_title = cleaned_episode_title(fallback_title, episode_number)
        .or_else(|| Some(fallback_title.trim().to_string()))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback_title.to_string());
    let season_title = season_number
        .map(|number| format!("Season {}", number))
        .unwrap_or_else(|| season_source.trim().to_string());
    let show_key = format!(
        "library:{}:show:{}",
        library_id,
        normalize_identity_segment(&show_title)
    );
    let season_key = format!(
        "{}:season:{}",
        show_key,
        season_number
            .map(|value| value.to_string())
            .unwrap_or_else(|| normalize_identity_segment(&season_title))
    );
    let episode_key = format!(
        "{}:episode:{}:{}",
        season_key,
        episode_number
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".into()),
        normalize_identity_segment(&episode_title)
    );

    ParsedShowPath {
        show_title,
        show_key,
        season_title,
        season_key,
        season_number,
        episode_title,
        episode_key,
        episode_number,
    }
}

/// Infer a season number from common folder or filename patterns.
pub fn infer_season_number(value: &str) -> Option<i32> {
    static SEASON_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
        vec![
            Regex::new(r"(?i)(?:^|[^a-z0-9])season\s*(\d{1,3})(?:[^0-9]|$)").unwrap(),
            Regex::new(r"(?i)(?:^|[^a-z0-9])series\s*(\d{1,3})(?:[^0-9]|$)").unwrap(),
            Regex::new(r"(?i)(?:^|[^a-z0-9])s(\d{1,3})(?:\s*e\d{1,3}|[^0-9]|$)").unwrap(),
        ]
    });

    first_pattern_number(value, &SEASON_PATTERNS)
}

/// Infer an episode number from common filename patterns such as `S03E01` or `3x01`.
pub fn infer_episode_number(value: &str) -> Option<i32> {
    static EPISODE_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
        vec![
            Regex::new(r"(?i)(?:^|[^a-z0-9])s\d{1,3}\s*e(\d{1,3})(?:[^0-9]|$)").unwrap(),
            Regex::new(r"(?i)(?:^|[^a-z0-9])\d{1,3}x(\d{1,3})(?:[^0-9]|$)").unwrap(),
            Regex::new(r"(?i)(?:^|[^a-z0-9])e(\d{1,3})(?:[^0-9]|$)").unwrap(),
        ]
    });

    first_pattern_number(value, &EPISODE_PATTERNS)
}

fn first_pattern_number(
    value: &str,
    patterns: &[Regex],
) -> Option<i32> {
    patterns.iter().find_map(|pattern| {
        pattern
            .captures(value)
            .and_then(|captures| captures.get(1))
            .and_then(|matched| matched.as_str().parse::<i32>().ok())
    })
}

fn cleaned_episode_title(
    value: &str,
    episode_number: Option<i32>,
) -> Option<String> {
    let mut cleaned = value.replace(['.', '_'], " ");
    if let Some(number) = episode_number {
        let markers = [
            format!("E{:02}", number),
            format!("e{:02}", number),
            format!("x{:02}", number),
        ];
        for marker in markers {
            cleaned = cleaned.replace(&marker, " ");
        }
    }
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    (!collapsed.trim().is_empty()).then(|| collapsed.trim().to_string())
}

fn normalize_identity_segment(value: &str) -> String {
    value
        .chars()
        .map(
            |character| {
                if character.is_ascii_alphanumeric() { character.to_ascii_lowercase() } else { '-' }
            },
        )
        .collect::<String>()
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Return persisted media files for a synchronized library.
pub fn get_library_files(
    conn: &mut SqliteConnection,
    library_id: i32,
) -> Result<Vec<PersistedMediaFileSummary>, diesel::result::Error> {
    let rows = load_catalog_files_for_library(conn, library_id, false)?;

    Ok(rows.into_iter().map(to_persisted_file_summary).collect())
}

/// Return whether a media library exists in the persistent catalog.
pub fn library_exists(
    conn: &mut SqliteConnection,
    library_id: i32,
) -> Result<bool, diesel::result::Error> {
    use crate::db::schema::media_libraries::dsl as media_libraries_dsl;

    Ok(media_libraries_dsl::media_libraries
        .filter(media_libraries_dsl::id.eq(library_id))
        .select(MediaLibrary::as_select())
        .first(conn)
        .optional()?
        .is_some())
}

/// List browser-facing media items, optionally filtered to one library.
pub fn list_media_items(
    conn: &mut SqliteConnection,
    library_id: Option<i32>,
) -> Result<Vec<MediaItemSummary>, diesel::result::Error> {
    list_media_items_with_preferred_languages(conn, library_id, &[])
}

/// List browser-facing media items using the caller's preferred metadata languages.
pub fn list_media_items_with_preferred_languages(
    conn: &mut SqliteConnection,
    library_id: Option<i32>,
    preferred_languages: &[String],
) -> Result<Vec<MediaItemSummary>, diesel::result::Error> {
    use crate::db::schema::media_items::dsl as media_items_dsl;

    let mut query = media_items_dsl::media_items.into_boxed();
    query = query.filter(media_items_dsl::deleted_at.is_null());
    if let Some(library_id) = library_id {
        query = query.filter(media_items_dsl::library_id.eq(library_id));
    }

    let rows = query
        .order((
            media_items_dsl::display_title.asc(),
            media_items_dsl::relative_path.asc(),
        ))
        .select(MediaItem::as_select())
        .load::<MediaItem>(conn)?;

    let metadata_links = preferred_metadata_links_by_item_id(
        conn,
        &rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        preferred_languages,
    )?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let mut summary = to_media_item_summary(row);
        let summary_id = summary.id;
        apply_primary_metadata_link(&mut summary, metadata_links.get(&summary_id));
        items.push(summary);
    }

    Ok(items)
}

/// Return unmatched movie-like items that are eligible for automatic metadata linking.
pub fn list_automatic_metadata_candidates(
    conn: &mut SqliteConnection,
    library_id: Option<i32>,
    limit: usize,
) -> Result<Vec<AutomaticMetadataCandidate>, diesel::result::Error> {
    list_automatic_metadata_candidates_with_options(conn, library_id, limit, false)
}

/// Return unmatched movie-like items for a user-triggered metadata refresh.
///
/// Manual library refreshes should retry currently unlinked movies even if a
/// previous automatic pass marked them as attempted. The normal candidate list
/// remains conservative so automatic polling does not repeatedly hit providers.
pub fn list_automatic_metadata_refresh_candidates(
    conn: &mut SqliteConnection,
    library_id: Option<i32>,
    limit: usize,
) -> Result<Vec<AutomaticMetadataCandidate>, diesel::result::Error> {
    list_automatic_metadata_candidates_with_options(conn, library_id, limit, true)
}

fn list_automatic_metadata_candidates_with_options(
    conn: &mut SqliteConnection,
    library_id: Option<i32>,
    limit: usize,
    include_previously_attempted: bool,
) -> Result<Vec<AutomaticMetadataCandidate>, diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as item_metadata_links_dsl;
    use crate::db::schema::media_items::dsl as media_items_dsl;
    use crate::db::schema::media_libraries::dsl as media_libraries_dsl;

    let libraries = media_libraries_dsl::media_libraries
        .select(MediaLibrary::as_select())
        .load::<MediaLibrary>(conn)?;
    let libraries_by_id = libraries
        .into_iter()
        .map(|library| (library.id, library))
        .collect::<HashMap<_, _>>();
    let linked_item_ids = item_metadata_links_dsl::item_metadata_links
        .filter(item_metadata_links_dsl::relation_kind.eq("primary"))
        .select(item_metadata_links_dsl::media_item_id)
        .load::<i32>(conn)?
        .into_iter()
        .collect::<HashSet<_>>();

    let rows = load_active_catalog_files(conn)?;

    let mut candidates = Vec::new();
    for row in rows {
        let Some(media_item_id) = row.media_item_id else {
            continue;
        };

        if linked_item_ids.contains(&media_item_id)
            || (!include_previously_attempted && row.metadata_match_attempted_at.is_some())
        {
            continue;
        }
        if row.media_kind != "video" {
            continue;
        }

        let Some(library) = libraries_by_id.get(&row.library_id) else {
            continue;
        };
        if library_id.is_some_and(|requested_library_id| requested_library_id != library.id) {
            continue;
        }
        let library_settings = media_library_settings_from_row(library.clone());
        if library_settings.kind != MediaLibraryKind::Movies {
            continue;
        }
        if library_settings.metadata_providers.is_empty() {
            continue;
        }

        candidates.push(AutomaticMetadataCandidate {
            item_id: media_item_id,
            relative_path: row.relative_path.clone(),
            display_title: row
                .display_title
                .unwrap_or_else(|| fallback_title_from_relative_path(&row.relative_path)),
            modified_at: row.modified_at,
            library_kind: library_settings.kind,
            metadata_providers: library_settings.metadata_providers,
        });
    }

    let show_rows = media_items_dsl::media_items
        .filter(media_items_dsl::item_type.eq("show"))
        .filter(media_items_dsl::deleted_at.is_null())
        .filter(media_items_dsl::missing_since.is_null())
        .select(MediaItem::as_select())
        .load::<MediaItem>(conn)?;
    for row in show_rows {
        if linked_item_ids.contains(&row.id) {
            continue;
        }

        let Some(library) = libraries_by_id.get(&row.library_id) else {
            continue;
        };
        if library_id.is_some_and(|requested_library_id| requested_library_id != library.id) {
            continue;
        }
        let library_settings = media_library_settings_from_row(library.clone());
        if library_settings.kind != MediaLibraryKind::Shows {
            continue;
        }
        if library_settings.metadata_providers.is_empty() {
            continue;
        }

        candidates.push(AutomaticMetadataCandidate {
            item_id: row.id,
            relative_path: row.relative_path.unwrap_or_default(),
            display_title: row.display_title,
            modified_at: row.modified_at,
            library_kind: library_settings.kind,
            metadata_providers: library_settings.metadata_providers,
        });
    }

    candidates.sort_by(|left, right| {
        right
            .modified_at
            .cmp(&left.modified_at)
            .then_with(|| left.display_title.cmp(&right.display_title))
    });
    candidates.truncate(limit);

    Ok(candidates)
}

/// Mark a media item as having been considered by the automatic metadata linker.
pub fn mark_metadata_match_attempted(
    conn: &mut SqliteConnection,
    item_id: i32,
    attempted_at: i64,
) -> Result<(), diesel::result::Error> {
    use crate::db::schema::media_file_libraries::dsl as media_file_libraries_dsl;

    diesel::update(
        media_file_libraries_dsl::media_file_libraries
            .filter(media_file_libraries_dsl::media_item_id.eq(item_id)),
    )
    .set(media_file_libraries_dsl::metadata_match_attempted_at.eq(attempted_at))
    .execute(conn)?;
    Ok(())
}

/// Return a single browser-facing media item by its stable identifier.
pub fn get_media_item(
    conn: &mut SqliteConnection,
    item_id: i32,
    data_dir: &str,
) -> Result<Option<MediaItemDetail>, diesel::result::Error> {
    get_media_item_with_preferred_languages(conn, item_id, data_dir, &[])
}

/// Return a single browser-facing media item by its stable identifier and preferred metadata languages.
pub fn get_media_item_with_preferred_languages(
    conn: &mut SqliteConnection,
    item_id: i32,
    data_dir: &str,
    preferred_languages: &[String],
) -> Result<Option<MediaItemDetail>, diesel::result::Error> {
    use crate::db::schema::media_items::dsl as media_items_dsl;

    let item = media_items_dsl::media_items
        .filter(media_items_dsl::id.eq(item_id))
        .filter(media_items_dsl::deleted_at.is_null())
        .select(MediaItem::as_select())
        .first(conn)
        .optional()?;

    let Some(item) = item else {
        return Ok(None);
    };

    let backing_file = load_backing_media_file(conn, item_id)?;
    let mut detail = to_media_item_detail(item.clone(), backing_file.as_ref());
    detail.hierarchy = load_media_item_hierarchy(conn, &item, preferred_languages)?;
    detail.children =
        list_media_item_children_with_preferred_languages(conn, item.id, preferred_languages)?;

    let metadata_links =
        prioritized_metadata_links_for_item(conn, item_id, item.library_id, preferred_languages)?;
    if !metadata_links.is_empty() {
        let primary_link = metadata_links
            .iter()
            .find(|link| link.relation_kind == "primary");
        if let Some(title) = primary_link
            .and_then(|link| link.title.as_deref())
            .map(str::trim)
            .filter(|title| !title.is_empty())
        {
            detail.display_title = title.to_string();
        }
        let presentation = presentation_from_metadata_links(conn, &metadata_links)?;
        detail.tagline = presentation.tagline;
        detail.overview = presentation.overview;
        detail.genres = presentation.genres;
        detail.release_year = presentation.release_year;
        if presentation.logo_url.is_some() {
            detail.logo_url = Some(format!("/api/v1/items/{}/artwork?kind=logo", item_id));
        }
        detail.rating = presentation.rating;
        detail.content_rating = presentation.content_rating;
        detail.linked_media_type = presentation.media_type;
        detail.has_metadata = true;
        if let Some(link) = primary_link {
            detail.metadata_refresh_state = Some(link.refresh_state.clone());
            detail.metadata_refresh_error = link.refresh_error.clone();
            detail.artwork_updated_at = link.updated_at;
        }
        detail.trailer_title = presentation.trailer_title;
        detail.trailer_url = presentation.trailer_url;
        detail.theme_song_url = presentation.theme_song_url;
        if presentation.poster_available {
            detail.poster_url = Some(format!("/api/v1/items/{}/artwork?kind=poster", item_id));
        }
        if presentation.backdrop_available {
            detail.backdrop_url = Some(format!("/api/v1/items/{}/artwork?kind=backdrop", item_id));
        }
    }

    if let Some(source_path) = resolve_media_item_source_path(conn, item_id)? {
        let assets = discover_item_assets(item_id, &source_path, data_dir);
        if assets.poster_path.is_some() {
            detail.poster_url = Some(format!("/api/v1/items/{}/artwork?kind=poster", item_id));
        }
        if assets.backdrop_path.is_some() {
            detail.backdrop_url = Some(format!("/api/v1/items/{}/artwork?kind=backdrop", item_id));
        }
        if assets.theme_song_path.is_some() {
            detail.theme_song_url = Some(format!("/api/v1/items/{}/theme", item_id));
        }
        detail.subtitle_tracks = assets
            .subtitle_paths
            .iter()
            .enumerate()
            .map(|(index, subtitle_path)| MediaSubtitleTrack {
                index,
                label: subtitle_label_from_path(&source_path, subtitle_path),
                format: subtitle_path
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(|value| value.to_ascii_uppercase())
                    .unwrap_or_else(|| "Subtitle".into()),
                url: format!("/api/v1/items/{}/subtitles/{}", item_id, index),
            })
            .collect();
    }
    detail.audio_tracks = backing_file
        .as_ref()
        .and_then(|file| audio_tracks_from_metadata_json(file.metadata_json.as_deref()))
        .unwrap_or_default();

    Ok(Some(detail))
}

/// Search browser-facing media items by title or relative path.
pub fn search_media_items(
    conn: &mut SqliteConnection,
    query: &str,
    library_id: Option<i32>,
) -> Result<Vec<MediaItemSummary>, diesel::result::Error> {
    search_media_items_with_preferred_languages(conn, query, library_id, &[])
}

/// Search browser-facing media items using the caller's preferred metadata languages.
pub fn search_media_items_with_preferred_languages(
    conn: &mut SqliteConnection,
    query: &str,
    library_id: Option<i32>,
    preferred_languages: &[String],
) -> Result<Vec<MediaItemSummary>, diesel::result::Error> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let query = query.trim().to_ascii_lowercase();
    let items = list_media_items_with_preferred_languages(conn, library_id, preferred_languages)?;

    Ok(items
        .into_iter()
        .filter(|item| {
            item.display_title.to_ascii_lowercase().contains(&query)
                || item.relative_path.to_ascii_lowercase().contains(&query)
                || item.media_kind.to_ascii_lowercase().contains(&query)
        })
        .collect())
}

/// Return Kodi/Plex-style media shelves for the browser home screen.
pub fn get_media_home(
    conn: &mut SqliteConnection,
    user_id: Option<i32>,
    library_id: Option<i32>,
) -> Result<MediaHome, diesel::result::Error> {
    get_media_home_with_preferred_languages(conn, user_id, library_id, &[])
}

/// Return Kodi/Plex-style media shelves using the caller's preferred metadata languages.
pub fn get_media_home_with_preferred_languages(
    conn: &mut SqliteConnection,
    user_id: Option<i32>,
    library_id: Option<i32>,
    preferred_languages: &[String],
) -> Result<MediaHome, diesel::result::Error> {
    let items = list_media_items_with_preferred_languages(conn, library_id, preferred_languages)?;

    let continue_watching =
        get_continue_watching_items(conn, user_id, library_id, preferred_languages)?;
    let recently_added = sort_recently_added(&items);
    let recommended = sort_recommended(&items, &continue_watching);
    let collection_provider_order = match library_id {
        Some(library_id) => media_library_metadata_provider_order(conn, library_id)?,
        None => Vec::new(),
    };
    let collections = list_metadata_collection_summaries_with_preferred_languages(
        conn,
        library_id,
        preferred_languages,
        &collection_provider_order,
    )?;

    Ok(MediaHome {
        library_id,
        shelves: vec![
            MediaShelf {
                id: "continue_watching".into(),
                title: "Continue watching".into(),
                items: continue_watching,
            },
            MediaShelf {
                id: "recently_added".into(),
                title: "Recently added".into(),
                items: recently_added,
            },
            MediaShelf {
                id: "recommended".into(),
                title: "Recommended".into(),
                items: recommended,
            },
        ],
        collections,
    })
}

/// Return a browser playback decision for a media item.
pub fn get_playback_decision(
    conn: &mut SqliteConnection,
    item_id: i32,
    profile: Option<&ClientProfile>,
) -> Result<Option<PlaybackDecision>, diesel::result::Error> {
    use crate::db::schema::media_items::dsl as media_items_dsl;

    let item = media_items_dsl::media_items
        .filter(media_items_dsl::id.eq(item_id))
        .filter(media_items_dsl::deleted_at.is_null())
        .select(MediaItem::as_select())
        .first(conn)
        .optional()?;

    let Some(item) = item else {
        return Ok(None);
    };

    let backing_file = load_backing_media_file(conn, item_id)?;

    Ok(Some(if let Some(row) = backing_file {
        if row.missing_since.is_some() {
            return Ok(Some(PlaybackDecision {
                item_id,
                can_direct_play: false,
                transcode_required: false,
                reason: "This item is missing from disk and cannot be played.".into(),
                stream_url: None,
                mime_type: detect_mime_type(&row),
                transcode_container: None,
                transcode_video_codec: None,
                transcode_audio_codec: None,
                video_transcode_required: false,
                audio_transcode_required: false,
                source_video_codec: row.video_codec,
                source_audio_codec: row.audio_codec,
                source_container: row.container,
            }));
        }

        let default_profile = ClientProfile {
            client_type: "web".into(),
            client_name: "Web".into(),
            supported_containers: vec!["mp4".into(), "webm".into()],
            supported_video_codecs: vec![
                "h264".into(),
                "av1".into(),
                "vp8".into(),
                "vp9".into(),
            ],
            supported_audio_codecs: vec![
                "aac".into(),
                "mp3".into(),
                "opus".into(),
                "vorbis".into(),
                "flac".into(),
            ],
            supported_subtitle_formats: vec!["vtt".into()],
            max_video_width: 0,
            max_video_height: 0,
            max_bitrate_kbps: 0,
            supports_adaptive_streaming: false,
            prefer_hls: false,
        };
        let p = profile.unwrap_or(&default_profile);

        let can_direct_play = item.playable && can_client_direct_play(&row, p);
        let mime_type = detect_mime_type(&row);

        let video_codec = row.video_codec.as_deref().unwrap_or("");
        let audio_codec = row.audio_codec.as_deref().unwrap_or("");
        let video_transcode_required = !video_codec.is_empty()
            && !p
                .supported_video_codecs
                .iter()
                .any(|c| codec_matches(video_codec, c));
        let audio_transcode_required = !audio_codec.is_empty()
            && !p
                .supported_audio_codecs
                .iter()
                .any(|c| codec_matches(audio_codec, c));

        // Target codecs when transcoding is required
        let transcode_video_codec =
            if video_transcode_required { Some("libx264".into()) } else { None };
        let transcode_audio_codec =
            if audio_transcode_required { Some("aac".into()) } else { None };
        let transcode_container = if !can_direct_play { Some("mp4".into()) } else { None };

        PlaybackDecision {
            item_id,
            can_direct_play,
            transcode_required: item.playable && !can_direct_play,
            reason: if can_direct_play {
                "Client direct play is supported for this item.".into()
            } else {
                "A transcode path will be required for playback.".into()
            },
            stream_url: can_direct_play.then(|| format!("/api/v1/items/{}/stream", item_id)),
            mime_type,
            transcode_container,
            transcode_video_codec,
            transcode_audio_codec,
            video_transcode_required,
            audio_transcode_required,
            source_video_codec: row.video_codec,
            source_audio_codec: row.audio_codec,
            source_container: row.container,
        }
    } else {
        PlaybackDecision {
            item_id,
            can_direct_play: false,
            transcode_required: false,
            reason: "This item is a container and cannot be played directly.".into(),
            stream_url: None,
            mime_type: None,
            transcode_container: None,
            transcode_video_codec: None,
            transcode_audio_codec: None,
            video_transcode_required: false,
            audio_transcode_required: false,
            source_video_codec: None,
            source_audio_codec: None,
            source_container: None,
        }
    }))
}

/// Resolve the direct-play source path for a media item.
pub fn resolve_media_item_source_path(
    conn: &mut SqliteConnection,
    item_id: i32,
) -> Result<Option<PathBuf>, diesel::result::Error> {
    use crate::db::schema::media_items::dsl as media_items_dsl;
    use crate::db::schema::media_libraries::dsl as media_libraries_dsl;

    let item = media_items_dsl::media_items
        .filter(media_items_dsl::id.eq(item_id))
        .filter(media_items_dsl::deleted_at.is_null())
        .select(MediaItem::as_select())
        .first(conn)
        .optional()?;
    let media_file = load_backing_media_file(conn, item_id)?;
    let Some(media_file) = media_file else {
        if let Some(item) = item.filter(|item| item.playable) {
            log::warn!(
                "Playable media item {} ({}) is missing a backing media_files link",
                item.id,
                item.relative_path.as_deref().unwrap_or_default()
            );
        }
        return Ok(None);
    };
    if media_file.missing_since.is_some() {
        return Ok(None);
    }
    if let Some(item_relative_path) = item
        .as_ref()
        .and_then(|item| item.relative_path.as_deref())
        .filter(|value| !value.trim().is_empty())
    {
        if item_relative_path != media_file.relative_path {
            log::warn!(
                "Ignoring mismatched backing media file for item {}: item path {:?}, media file path {:?}",
                item_id,
                item_relative_path,
                media_file.relative_path
            );
            return Ok(None);
        }
    }

    let library = media_libraries_dsl::media_libraries
        .filter(media_libraries_dsl::id.eq(media_file.library_id))
        .select(MediaLibrary::as_select())
        .first(conn)
        .optional()?;

    Ok(library.map(|library| {
        if !media_file.source_root_path.trim().is_empty() {
            PathBuf::from(media_file.source_root_path).join(&media_file.relative_path)
        } else {
            let fallback_root = media_library_settings_from_row(library)
                .configured_paths()
                .into_iter()
                .next()
                .unwrap_or_default();
            PathBuf::from(fallback_root).join(&media_file.relative_path)
        }
    }))
}

/// Resolve a local theme-song asset path for a media item.
pub fn resolve_item_theme_song_path(
    conn: &mut SqliteConnection,
    item_id: i32,
    data_dir: &str,
) -> Result<Option<PathBuf>, diesel::result::Error> {
    let Some(source_path) = resolve_media_item_source_path(conn, item_id)? else {
        return Ok(None);
    };

    Ok(discover_item_assets(item_id, &source_path, data_dir).theme_song_path)
}

/// Return ordered YouTube theme-song lookup candidates for a secondary metadata provider.
pub fn get_item_youtube_theme_provider_references(
    conn: &mut SqliteConnection,
    item_id: i32,
    provider_id: MetadataProviderId,
) -> Result<Vec<(String, String, String)>, diesel::result::Error> {
    get_item_secondary_provider_references(conn, item_id, provider_id)
}

/// Return ordered lookup candidates for a secondary metadata provider.
pub fn get_item_secondary_provider_references(
    conn: &mut SqliteConnection,
    item_id: i32,
    provider_id: MetadataProviderId,
) -> Result<Vec<(String, String, String)>, diesel::result::Error> {
    let registry = MetadataRegistry::new();
    let Some(provider) = registry.provider(&provider_id) else {
        return Ok(Vec::new());
    };
    let source_provider_ids = provider.descriptor().extends_provider_ids;
    if source_provider_ids.is_empty() {
        return Ok(Vec::new());
    }

    get_item_theme_song_source_references(conn, item_id, &source_provider_ids)
}

/// Return ordered YouTube trailer lookup candidates for a secondary metadata provider.
pub fn get_item_youtube_trailer_provider_references(
    conn: &mut SqliteConnection,
    item_id: i32,
    provider_id: MetadataProviderId,
) -> Result<Vec<(String, String, String)>, diesel::result::Error> {
    get_item_secondary_provider_references(conn, item_id, provider_id)
}

/// Return ordered collection lookup candidates for a secondary theme-song provider.
pub fn get_item_youtube_theme_collection_references(
    conn: &mut SqliteConnection,
    item_id: i32,
    provider_id: MetadataProviderId,
) -> Result<Vec<(i32, String, String, String)>, diesel::result::Error> {
    use crate::db::schema::media_items::dsl as media_items_dsl;
    use crate::db::schema::metadata_collection_items::dsl as collection_items_dsl;
    use crate::db::schema::metadata_collections::dsl as collections_dsl;

    let registry = MetadataRegistry::new();
    let Some(provider) = registry.provider(&provider_id) else {
        return Ok(Vec::new());
    };
    let source_provider_values = provider
        .descriptor()
        .extends_provider_ids
        .into_iter()
        .map(|provider_id| provider_id.as_storage_value().to_string())
        .collect::<Vec<_>>();
    if source_provider_values.is_empty() {
        return Ok(Vec::new());
    }

    let mut current_id = Some(item_id);
    let mut item_ids = Vec::new();
    let mut visited = HashSet::new();
    while let Some(current_item_id) = current_id {
        if !visited.insert(current_item_id) {
            break;
        }
        item_ids.push(current_item_id);
        current_id = media_items_dsl::media_items
            .filter(media_items_dsl::id.eq(current_item_id))
            .filter(media_items_dsl::deleted_at.is_null())
            .select(media_items_dsl::parent_id)
            .first::<Option<i32>>(conn)
            .optional()?
            .flatten();
    }
    if item_ids.is_empty() {
        return Ok(Vec::new());
    }

    let collection_item_rows = collection_items_dsl::metadata_collection_items
        .filter(collection_items_dsl::media_item_id.eq_any(&item_ids))
        .select(MetadataCollectionItem::as_select())
        .load::<MetadataCollectionItem>(conn)?;
    if collection_item_rows.is_empty() {
        return Ok(Vec::new());
    }

    let collection_item_by_collection_id = collection_item_rows
        .into_iter()
        .map(|item| (item.collection_id, item))
        .collect::<HashMap<_, _>>();
    let mut collection_rows = collections_dsl::metadata_collections
        .filter(
            collections_dsl::id.eq_any(
                collection_item_by_collection_id
                    .keys()
                    .copied()
                    .collect::<Vec<_>>(),
            ),
        )
        .filter(collections_dsl::provider_id.eq_any(&source_provider_values))
        .filter(collections_dsl::relation_kind.eq("primary"))
        .select(MetadataCollection::as_select())
        .load::<MetadataCollection>(conn)?;

    let source_provider_rank = source_provider_values
        .iter()
        .enumerate()
        .map(|(index, provider_id)| (provider_id.clone(), index))
        .collect::<HashMap<_, _>>();
    let fallback_provider_rank = source_provider_rank.len();
    collection_rows.sort_by(|left, right| {
        let left_provider_rank = source_provider_rank
            .get(&left.provider_id)
            .copied()
            .unwrap_or(fallback_provider_rank);
        let right_provider_rank = source_provider_rank
            .get(&right.provider_id)
            .copied()
            .unwrap_or(fallback_provider_rank);
        let left_item_rank = collection_item_by_collection_id
            .get(&left.id)
            .and_then(|item| {
                item_ids
                    .iter()
                    .position(|item_id| *item_id == item.media_item_id)
            })
            .unwrap_or(item_ids.len());
        let right_item_rank = collection_item_by_collection_id
            .get(&right.id)
            .and_then(|item| {
                item_ids
                    .iter()
                    .position(|item_id| *item_id == item.media_item_id)
            })
            .unwrap_or(item_ids.len());

        left_item_rank
            .cmp(&right_item_rank)
            .then_with(|| left_provider_rank.cmp(&right_provider_rank))
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| right.id.cmp(&left.id))
    });

    let mut seen = HashSet::new();
    Ok(collection_rows
        .into_iter()
        .filter(|collection| {
            seen.insert((
                collection.source_provider_id.clone(),
                collection.source_external_id.clone(),
            ))
        })
        .map(|collection| {
            (
                collection.id,
                "collection".to_string(),
                collection.source_provider_id,
                collection.source_external_id,
            )
        })
        .collect())
}

fn get_item_theme_song_source_references(
    conn: &mut SqliteConnection,
    item_id: i32,
    source_provider_ids: &[MetadataProviderId],
) -> Result<Vec<(String, String, String)>, diesel::result::Error> {
    use crate::db::schema::media_items::dsl as media_items_dsl;

    let mut current_id = Some(item_id);
    let mut visited = HashSet::new();

    while let Some(current_item_id) = current_id {
        if !visited.insert(current_item_id) {
            break;
        }

        let links =
            get_item_theme_song_source_metadata_links(conn, current_item_id, source_provider_ids)?;
        if !links.is_empty() {
            let mut references = Vec::new();
            let mut seen = HashSet::new();
            for link in links {
                append_theme_song_source_references(conn, &link, &mut references, &mut seen)?;
            }
            return Ok(references);
        }

        current_id = media_items_dsl::media_items
            .filter(media_items_dsl::id.eq(current_item_id))
            .filter(media_items_dsl::deleted_at.is_null())
            .select(media_items_dsl::parent_id)
            .first::<Option<i32>>(conn)
            .optional()?
            .flatten();
    }

    Ok(Vec::new())
}

fn append_theme_song_source_references(
    conn: &mut SqliteConnection,
    link: &ItemMetadataLink,
    references: &mut Vec<(String, String, String)>,
    seen: &mut HashSet<(String, String, String)>,
) -> Result<(), diesel::result::Error> {
    if let Some(media_type) = link
        .media_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let source_reference = (
            media_type.to_string(),
            link.provider_id.clone(),
            link.external_id.clone(),
        );
        if seen.insert(source_reference.clone()) {
            references.push(source_reference);
        }

        for (database_id, external_id) in metadata_external_ids(conn, link.id)? {
            let external_reference = (media_type.to_string(), database_id, external_id);
            if seen.insert(external_reference.clone()) {
                references.push(external_reference);
            }
        }
    }

    Ok(())
}

fn get_item_theme_song_source_metadata_links(
    conn: &mut SqliteConnection,
    item_id: i32,
    source_provider_ids: &[MetadataProviderId],
) -> Result<Vec<ItemMetadataLink>, diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as metadata_links_dsl;

    let source_provider_values = source_provider_ids
        .iter()
        .map(|provider_id| provider_id.as_storage_value().to_string())
        .collect::<Vec<_>>();
    let source_provider_rank = source_provider_values
        .iter()
        .enumerate()
        .map(|(index, provider_id)| (provider_id.clone(), index))
        .collect::<HashMap<_, _>>();
    let fallback_provider_rank = source_provider_rank.len();

    let mut links = metadata_links_dsl::item_metadata_links
        .filter(metadata_links_dsl::media_item_id.eq(item_id))
        .filter(metadata_links_dsl::provider_id.eq_any(&source_provider_values))
        .filter(metadata_links_dsl::relation_kind.eq("primary"))
        .filter(metadata_links_dsl::media_type.is_not_null())
        .select(ItemMetadataLink::as_select())
        .load::<ItemMetadataLink>(conn)?;

    links.sort_by(|left, right| {
        let left_provider_rank = source_provider_rank
            .get(&left.provider_id)
            .copied()
            .unwrap_or(fallback_provider_rank);
        let right_provider_rank = source_provider_rank
            .get(&right.provider_id)
            .copied()
            .unwrap_or(fallback_provider_rank);

        left_provider_rank
            .cmp(&right_provider_rank)
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| right.id.cmp(&left.id))
    });

    Ok(links)
}

fn metadata_external_ids(
    conn: &mut SqliteConnection,
    metadata_link_id: i32,
) -> Result<Vec<(String, String)>, diesel::result::Error> {
    use crate::db::schema::item_metadata_external_ids::dsl as external_ids_dsl;

    external_ids_dsl::item_metadata_external_ids
        .filter(external_ids_dsl::metadata_link_id.eq(metadata_link_id))
        .order((external_ids_dsl::source.asc(), external_ids_dsl::id.asc()))
        .select((external_ids_dsl::source, external_ids_dsl::external_id))
        .load::<(String, String)>(conn)
}

/// Resolve a local subtitle asset path for a media item by track index.
pub fn resolve_item_subtitle_path(
    conn: &mut SqliteConnection,
    item_id: i32,
    track_index: usize,
    data_dir: &str,
) -> Result<Option<PathBuf>, diesel::result::Error> {
    let Some(source_path) = resolve_media_item_source_path(conn, item_id)? else {
        return Ok(None);
    };

    Ok(discover_item_assets(item_id, &source_path, data_dir)
        .subtitle_paths
        .into_iter()
        .nth(track_index))
}

/// Resolve a local poster or backdrop asset path for a media item.
pub fn resolve_local_item_artwork_path(
    conn: &mut SqliteConnection,
    item_id: i32,
    kind: ArtworkKind,
    data_dir: &str,
) -> Result<Option<PathBuf>, diesel::result::Error> {
    let Some(source_path) = resolve_media_item_source_path(conn, item_id)? else {
        return Ok(None);
    };

    let assets = discover_item_assets(item_id, &source_path, data_dir);
    Ok(match kind {
        ArtworkKind::Poster => assets.poster_path,
        ArtworkKind::Backdrop => assets.backdrop_path,
        ArtworkKind::Logo => None,
    })
}

/// Store or update playback progress for one media item.
pub fn upsert_playback_progress(
    conn: &mut SqliteConnection,
    user_id: i32,
    item_id: i32,
    position_ms: i64,
    duration_ms: Option<i64>,
    completed: bool,
) -> Result<(), diesel::result::Error> {
    use crate::db::schema::playback_progress::dsl as playback_progress_dsl;

    let existing = playback_progress_dsl::playback_progress
        .filter(playback_progress_dsl::user_id.eq(user_id))
        .filter(playback_progress_dsl::media_item_id.eq(item_id))
        .select(PlaybackProgress::as_select())
        .first(conn)
        .optional()?;

    let progress = NewPlaybackProgress {
        user_id,
        media_item_id: item_id,
        position_ms,
        duration_ms,
        completed,
        updated_at: Some(current_timestamp()),
    };

    if let Some(existing) = existing {
        diesel::update(
            playback_progress_dsl::playback_progress
                .filter(playback_progress_dsl::id.eq(existing.id)),
        )
        .set(&progress)
        .execute(conn)?;
    } else {
        diesel::insert_into(playback_progress_dsl::playback_progress)
            .values(&progress)
            .execute(conn)?;
    }

    Ok(())
}

/// Return saved playback progress for a user and item.
pub fn get_user_playback_progress(
    conn: &mut SqliteConnection,
    user_id: Option<i32>,
    item_id: i32,
) -> Result<Option<PlaybackProgress>, diesel::result::Error> {
    use crate::db::schema::playback_progress::dsl as playback_progress_dsl;

    let Some(user_id) = user_id else {
        return Ok(None);
    };

    playback_progress_dsl::playback_progress
        .filter(playback_progress_dsl::user_id.eq(user_id))
        .filter(playback_progress_dsl::media_item_id.eq(item_id))
        .filter(playback_progress_dsl::completed.eq(false))
        .select(PlaybackProgress::as_select())
        .first(conn)
        .optional()
}

fn apply_playback_progress_to_summary(
    summary: &mut MediaItemSummary,
    progress: &PlaybackProgress,
) {
    summary.playback_position_ms = Some(progress.position_ms);
    summary.playback_duration_ms = progress.duration_ms;
}

fn get_continue_watching_items(
    conn: &mut SqliteConnection,
    user_id: Option<i32>,
    library_id: Option<i32>,
    preferred_languages: &[String],
) -> Result<Vec<MediaItemSummary>, diesel::result::Error> {
    use crate::db::schema::playback_progress::dsl as playback_progress_dsl;

    let Some(user_id) = user_id else {
        return Ok(Vec::new());
    };

    let progress_rows = playback_progress_dsl::playback_progress
        .filter(playback_progress_dsl::user_id.eq(user_id))
        .filter(playback_progress_dsl::completed.eq(false))
        .order(playback_progress_dsl::updated_at.desc())
        .select(PlaybackProgress::as_select())
        .load::<PlaybackProgress>(conn)?;

    let mut items = Vec::new();
    for progress in progress_rows {
        if let Some(row) = load_media_item_row(conn, progress.media_item_id)? {
            let mut item =
                media_item_summary_with_preferred_languages(conn, row, preferred_languages)?;
            apply_playback_progress_to_summary(&mut item, &progress);
            if library_id.is_none() || Some(item.library_id) == library_id {
                items.push(item);
            }
        }
    }

    Ok(items)
}

fn sort_recently_added(items: &[MediaItemSummary]) -> Vec<MediaItemSummary> {
    let items_by_id = items
        .iter()
        .cloned()
        .map(|item| (item.id, item))
        .collect::<HashMap<_, _>>();
    let mut show_groups = HashMap::<i32, Vec<MediaItemSummary>>::new();
    let mut entries = Vec::<(Option<i64>, MediaItemSummary)>::new();

    let mut leaf_items = items
        .iter()
        .filter(|item| item.child_count == 0)
        .cloned()
        .collect::<Vec<_>>();
    leaf_items.sort_by(|left, right| right.modified_at.cmp(&left.modified_at));

    for item in leaf_items {
        if item.item_type == "episode" {
            if let Some(show_id) = root_show_item_id(&item, &items_by_id) {
                show_groups.entry(show_id).or_default().push(item);
                continue;
            }
        }

        entries.push((item.modified_at, item));
    }

    for (show_id, episodes) in show_groups {
        let representative = if episodes.len() == 1 {
            episodes[0].clone()
        } else {
            let unique_season_ids = episodes
                .iter()
                .filter_map(|episode| episode.parent_id)
                .collect::<HashSet<_>>();
            if unique_season_ids.len() == 1 {
                unique_season_ids
                    .into_iter()
                    .next()
                    .and_then(|season_id| items_by_id.get(&season_id).cloned())
                    .unwrap_or_else(|| episodes[0].clone())
            } else {
                items_by_id
                    .get(&show_id)
                    .cloned()
                    .unwrap_or_else(|| episodes[0].clone())
            }
        };
        let modified_at = episodes
            .iter()
            .filter_map(|episode| episode.modified_at)
            .max();
        entries.push((modified_at, representative));
    }

    entries.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.display_title.cmp(&right.1.display_title))
    });
    entries.into_iter().map(|(_, item)| item).collect()
}

fn root_show_item_id(
    item: &MediaItemSummary,
    items_by_id: &HashMap<i32, MediaItemSummary>,
) -> Option<i32> {
    let mut current = item;

    while let Some(parent_id) = current.parent_id {
        let parent = items_by_id.get(&parent_id)?;
        if parent.item_type == "show" {
            return Some(parent.id);
        }
        current = parent;
    }

    None
}

fn sort_recommended(
    items: &[MediaItemSummary],
    continue_watching: &[MediaItemSummary],
) -> Vec<MediaItemSummary> {
    let items_by_id = items
        .iter()
        .cloned()
        .map(|item| (item.id, item))
        .collect::<HashMap<_, _>>();
    let continue_ids = recommended_excluded_item_ids(continue_watching, &items_by_id);
    let mut seen_recommendations = HashSet::<i32>::new();

    let mut items = items
        .iter()
        .filter_map(|item| {
            if continue_ids.contains(&item.id) {
                return None;
            }

            let recommendation = recommended_representative_item(item, &items_by_id)?;
            if continue_ids.contains(&recommendation.id)
                || !seen_recommendations.insert(recommendation.id)
            {
                return None;
            }

            Some(recommendation)
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        right
            .duration_ms
            .unwrap_or_default()
            .cmp(&left.duration_ms.unwrap_or_default())
            .then_with(|| right.modified_at.cmp(&left.modified_at))
    });
    items
}

fn recommended_excluded_item_ids(
    continue_watching: &[MediaItemSummary],
    items_by_id: &HashMap<i32, MediaItemSummary>,
) -> HashSet<i32> {
    continue_watching
        .iter()
        .flat_map(|item| {
            let mut ids = vec![item.id];
            if let Some(show_id) = root_show_item_id(item, items_by_id) {
                ids.push(show_id);
            }
            ids
        })
        .collect()
}

fn recommended_representative_item(
    item: &MediaItemSummary,
    items_by_id: &HashMap<i32, MediaItemSummary>,
) -> Option<MediaItemSummary> {
    if matches!(item.item_type.as_str(), "season" | "episode") {
        if let Some(show_id) = root_show_item_id(item, items_by_id) {
            if let Some(show) = items_by_id.get(&show_id) {
                return Some(show.clone());
            }
        }
        return None;
    }

    Some(item.clone())
}

/// Return one browser-facing media item summary by its stable identifier.
pub fn get_media_item_summary(
    conn: &mut SqliteConnection,
    item_id: i32,
) -> Result<Option<MediaItemSummary>, diesel::result::Error> {
    match load_media_item_row(conn, item_id)? {
        Some(row) => Ok(Some(media_item_summary_with_preferred_title(conn, row)?)),
        None => Ok(None),
    }
}

/// Return one browser-facing media item summary using the caller's metadata language order.
pub fn get_media_item_summary_with_preferred_languages(
    conn: &mut SqliteConnection,
    item_id: i32,
    preferred_languages: &[String],
) -> Result<Option<MediaItemSummary>, diesel::result::Error> {
    match load_media_item_row(conn, item_id)? {
        Some(row) => Ok(Some(media_item_summary_with_preferred_languages(
            conn,
            row,
            preferred_languages,
        )?)),
        None => Ok(None),
    }
}

/// Return one media item summary with its browser-facing parent hierarchy.
pub fn get_media_item_summary_with_hierarchy(
    conn: &mut SqliteConnection,
    item_id: i32,
    preferred_languages: &[String],
) -> Result<Option<(MediaItemSummary, Vec<MediaItemSummary>)>, diesel::result::Error> {
    let Some(row) = load_media_item_row(conn, item_id)? else {
        return Ok(None);
    };

    let hierarchy = load_media_item_hierarchy(conn, &row, preferred_languages)?;
    let summary = media_item_summary_with_preferred_languages(conn, row, preferred_languages)?;
    Ok(Some((summary, hierarchy)))
}

fn container_matches(
    file_container: &str,
    profile_container: &str,
) -> bool {
    file_container
        .split(',')
        .map(|value| value.trim().to_ascii_lowercase())
        .any(|value| {
            value == profile_container.to_ascii_lowercase()
                || (value == "mov" && matches!(profile_container, "mp4" | "m4v"))
                || (value == "matroska" && profile_container == "mkv")
        })
}

fn codec_matches(
    file_codec: &str,
    profile_codec: &str,
) -> bool {
    let normalized_file_codec = normalize_codec_name(file_codec);
    let normalized_profile_codec = normalize_codec_name(profile_codec);
    normalized_file_codec == normalized_profile_codec
}

fn normalize_codec_name(codec: &str) -> String {
    let normalized = codec
        .trim()
        .to_ascii_lowercase()
        .replace('-', "")
        .replace('_', "");

    match normalized.as_str() {
        "avc1" | "avc" | "h264" | "x264" => "h264".into(),
        "hev1" | "hvc1" | "hevc" | "h265" | "x265" => "hevc".into(),
        "mpeg4" | "mp4v" => "mpeg4".into(),
        "aac" | "aaclc" | "mp4a" => "aac".into(),
        "eac3" | "eac" => "eac3".into(),
        "ac3" | "ac-3" => "ac3".into(),
        "vorbis" => "vorbis".into(),
        "opus" => "opus".into(),
        "flac" => "flac".into(),
        "mp3" | "mpeg3" | "mpga" => "mp3".into(),
        "vp8" => "vp8".into(),
        "vp9" => "vp9".into(),
        "av1" => "av1".into(),
        _ => normalized,
    }
}

fn within_resolution_limits(
    file: &CatalogMediaFile,
    profile: &ClientProfile,
) -> bool {
    if profile.max_video_width > 0 {
        if let Some(w) = file.width {
            if w as u32 > profile.max_video_width {
                return false;
            }
        }
    }
    if profile.max_video_height > 0 {
        if let Some(h) = file.height {
            if h as u32 > profile.max_video_height {
                return false;
            }
        }
    }
    true
}

fn can_client_direct_play(
    file: &CatalogMediaFile,
    profile: &ClientProfile,
) -> bool {
    let extension = Path::new(&file.relative_path)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    let is_supported_container = if let Some(container) = file.container.as_deref() {
        profile
            .supported_containers
            .iter()
            .any(|c| container_matches(container, c))
    } else {
        // Fallback to extension check if container is missing
        extension
            .as_deref()
            .map(|ext| {
                profile
                    .supported_containers
                    .iter()
                    .any(|c| c.eq_ignore_ascii_case(ext))
            })
            .unwrap_or(false)
    };

    let is_supported_video = if let Some(codec) = file.video_codec.as_deref() {
        profile
            .supported_video_codecs
            .iter()
            .any(|c| codec_matches(codec, c))
    } else {
        true // No video track -> skip check
    };

    let is_supported_audio = if let Some(codec) = file.audio_codec.as_deref() {
        profile
            .supported_audio_codecs
            .iter()
            .any(|c| codec_matches(codec, c))
    } else {
        true // No audio track -> skip check
    };

    is_supported_container
        && is_supported_video
        && is_supported_audio
        && within_resolution_limits(file, profile)
}

fn detect_mime_type(file: &CatalogMediaFile) -> Option<String> {
    let extension = Path::new(&file.relative_path)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    match extension.as_deref() {
        Some("mp4" | "m4v") => Some("video/mp4".into()),
        Some("webm") => Some("video/webm".into()),
        Some("mp3") => Some("audio/mpeg".into()),
        Some("m4a") => Some("audio/mp4".into()),
        Some("ogg" | "opus") => Some("audio/ogg".into()),
        Some("wav") => Some("audio/wav".into()),
        Some("flac") => Some("audio/flac".into()),
        _ => None,
    }
}

fn inspect_library_with_inventory(library: &MediaLibrarySettings) -> LibraryInspection {
    let configured_paths = library.configured_paths();
    let path = configured_paths.first().cloned().unwrap_or_default();
    let name = display_name(library, &path);

    if configured_paths.is_empty() {
        return LibraryInspection {
            summary: LibraryScanSummary {
                name,
                path,
                paths: Vec::new(),
                recursive: library.recursive,
                kind: library.kind.clone(),
                status: LibraryScanStatus::EmptyPath,
                total_files: 0,
                video_files: 0,
                audio_files: 0,
                image_files: 0,
                book_files: 0,
                other_files: 0,
                error: Some("Library path is empty".into()),
            },
            files: Vec::new(),
            scanned_root_paths: HashSet::new(),
        };
    }

    let mut counters = FileCounters::default();
    let mut files = Vec::new();
    let mut scanned_root_paths = HashSet::new();
    let mut errors = Vec::new();
    let mut first_failure_status = None;

    for configured_path in &configured_paths {
        let filesystem_path = Path::new(configured_path);
        if !filesystem_path.exists() {
            first_failure_status.get_or_insert(LibraryScanStatus::MissingPath);
            errors.push(format!("{}: path does not exist", configured_path));
            continue;
        }

        if !filesystem_path.is_dir() {
            first_failure_status.get_or_insert(LibraryScanStatus::NotDirectory);
            errors.push(format!("{}: path is not a directory", configured_path));
            continue;
        }

        match scan_directory(
            filesystem_path,
            filesystem_path,
            library.recursive,
            &library.kind,
        ) {
            Ok((nested, nested_files)) => {
                scanned_root_paths.insert(configured_path.clone());
                counters.total_files += nested.total_files;
                counters.video_files += nested.video_files;
                counters.audio_files += nested.audio_files;
                counters.image_files += nested.image_files;
                counters.book_files += nested.book_files;
                counters.other_files += nested.other_files;
                files.extend(nested_files);
            }
            Err(error) => {
                first_failure_status.get_or_insert(LibraryScanStatus::Unreadable);
                errors.push(format!("{}: {}", configured_path, error));
            }
        }
    }

    if !files.is_empty() || errors.len() < configured_paths.len() {
        LibraryInspection {
            summary: LibraryScanSummary {
                name,
                path,
                paths: configured_paths,
                recursive: library.recursive,
                kind: library.kind.clone(),
                status: LibraryScanStatus::Available,
                total_files: counters.total_files,
                video_files: counters.video_files,
                audio_files: counters.audio_files,
                image_files: counters.image_files,
                book_files: counters.book_files,
                other_files: counters.other_files,
                error: (!errors.is_empty()).then(|| errors.join("; ")),
            },
            files,
            scanned_root_paths,
        }
    } else {
        LibraryInspection {
            summary: LibraryScanSummary {
                name,
                path,
                paths: configured_paths,
                recursive: library.recursive,
                kind: library.kind.clone(),
                status: first_failure_status.unwrap_or(LibraryScanStatus::Unreadable),
                total_files: 0,
                video_files: 0,
                audio_files: 0,
                image_files: 0,
                book_files: 0,
                other_files: 0,
                error: Some(errors.join("; ")),
            },
            files: Vec::new(),
            scanned_root_paths,
        }
    }
}

impl LibraryScanStatus {
    fn as_storage_value(&self) -> &'static str {
        match self {
            LibraryScanStatus::NeverScanned => "never_scanned",
            LibraryScanStatus::Available => "available",
            LibraryScanStatus::EmptyPath => "empty_path",
            LibraryScanStatus::MissingPath => "missing_path",
            LibraryScanStatus::NotDirectory => "not_directory",
            LibraryScanStatus::Unreadable => "unreadable",
        }
    }

    fn from_storage_value(value: &str) -> Self {
        match value.trim() {
            "available" => LibraryScanStatus::Available,
            "empty_path" => LibraryScanStatus::EmptyPath,
            "missing_path" => LibraryScanStatus::MissingPath,
            "not_directory" => LibraryScanStatus::NotDirectory,
            "unreadable" => LibraryScanStatus::Unreadable,
            _ => LibraryScanStatus::NeverScanned,
        }
    }
}

impl MediaLibraryKind {
    fn as_storage_value(&self) -> String {
        match self {
            MediaLibraryKind::Mixed => "mixed",
            MediaLibraryKind::Movies => "movies",
            MediaLibraryKind::Shows => "shows",
            MediaLibraryKind::Music => "music",
            MediaLibraryKind::Photos => "photos",
            MediaLibraryKind::Books => "books",
            MediaLibraryKind::HomeVideos => "home_videos",
        }
        .to_string()
    }

    fn from_storage_value(value: &str) -> Self {
        match value.trim() {
            "movies" => MediaLibraryKind::Movies,
            "shows" => MediaLibraryKind::Shows,
            "music" => MediaLibraryKind::Music,
            "photos" => MediaLibraryKind::Photos,
            "books" => MediaLibraryKind::Books,
            "home_videos" => MediaLibraryKind::HomeVideos,
            _ => MediaLibraryKind::Mixed,
        }
    }
}

fn media_library_settings_from_row(row: MediaLibrary) -> MediaLibrarySettings {
    let mut paths = serde_json::from_str::<Vec<String>>(&row.paths_json).unwrap_or_default();
    if paths.is_empty() {
        paths = parse_library_storage_paths(&row.path);
    }
    if paths.is_empty() {
        paths.push(row.path.clone());
    }

    let mut metadata_providers = serde_json::from_str::<Vec<String>>(&row.metadata_providers_json)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| MetadataProviderId::from_storage_value(&value))
        .collect::<Vec<_>>();
    if metadata_providers.is_empty() {
        metadata_providers.push(MetadataProviderId::Tmdb);
    }
    let metadata_languages = serde_json::from_str::<Vec<String>>(&row.metadata_languages_json)
        .unwrap_or_else(|_| vec!["en-US".into()]);
    let allowed_user_ids =
        serde_json::from_str::<Vec<i32>>(&row.allowed_user_ids_json).unwrap_or_default();
    let metadata_language_mode = match row.metadata_language_mode.as_str() {
        "manual" => MediaLibraryMetadataLanguageMode::Manual,
        _ => MediaLibraryMetadataLanguageMode::Auto,
    };

    let mut library = MediaLibrarySettings {
        name: row.name,
        path: paths.first().cloned().unwrap_or_default(),
        paths,
        recursive: row.recursive,
        kind: MediaLibraryKind::from_storage_value(&row.kind),
        metadata_providers,
        metadata_language_mode,
        metadata_languages,
        allowed_user_ids,
    };
    library.normalize();
    library
}

fn media_library_record_values(library: &MediaLibrarySettings) -> NewMediaLibrary {
    let mut normalized = library.clone();
    normalized.normalize();
    let primary_path = normalized.primary_path();

    NewMediaLibrary {
        name: normalized.name,
        path: primary_path,
        paths_json: serde_json::to_string(&normalized.paths).unwrap_or_else(|_| "[]".into()),
        kind: normalized.kind.as_storage_value(),
        recursive: normalized.recursive,
        metadata_providers_json: serde_json::to_string(
            &normalized
                .metadata_providers
                .iter()
                .map(|provider| provider.as_storage_value())
                .collect::<Vec<_>>(),
        )
        .unwrap_or_else(|_| "[\"tmdb\"]".into()),
        metadata_language_mode: match normalized.metadata_language_mode {
            MediaLibraryMetadataLanguageMode::Auto => "auto",
            MediaLibraryMetadataLanguageMode::Manual => "manual",
        }
        .into(),
        metadata_languages_json: serde_json::to_string(&normalized.metadata_languages)
            .unwrap_or_else(|_| "[\"en-US\"]".into()),
        allowed_user_ids_json: serde_json::to_string(&normalized.allowed_user_ids)
            .unwrap_or_else(|_| "[]".into()),
    }
}

fn insert_media_library(
    conn: &mut SqliteConnection,
    library: &MediaLibrarySettings,
) -> Result<(), diesel::result::Error> {
    use crate::db::schema::media_libraries::dsl as media_libraries_dsl;

    diesel::insert_into(media_libraries_dsl::media_libraries)
        .values(&media_library_record_values(library))
        .execute(conn)?;
    Ok(())
}

fn update_media_library(
    conn: &mut SqliteConnection,
    library_id: i32,
    library: &MediaLibrarySettings,
) -> Result<(), diesel::result::Error> {
    use crate::db::schema::media_libraries::dsl as media_libraries_dsl;

    diesel::update(
        media_libraries_dsl::media_libraries.filter(media_libraries_dsl::id.eq(library_id)),
    )
    .set(&media_library_record_values(library))
    .execute(conn)?;
    Ok(())
}

impl DiscoveredMediaFile {
    fn physical_path(&self) -> String {
        self.full_path.to_string_lossy().to_string()
    }

    fn to_new_media_file(
        &self,
        metadata: ExtractedMetadata,
    ) -> NewMediaFile {
        NewMediaFile {
            path: self.physical_path(),
            file_size: self.file_size,
            modified_at: self.modified_at,
            media_kind: self.media_kind.clone(),
            fingerprint_seed: self.fingerprint_seed.clone(),
            container: metadata.container,
            duration_ms: metadata.duration_ms,
            bit_rate: metadata.bit_rate,
            width: metadata.width,
            height: metadata.height,
            video_codec: metadata.video_codec,
            audio_codec: metadata.audio_codec,
            metadata_json: metadata.metadata_json,
            metadata_updated_at: metadata.metadata_updated_at,
        }
    }

    fn to_new_media_file_library(
        &self,
        media_file_id: i32,
        library_id: i32,
        display_title: Option<String>,
        metadata_match_attempted_at: Option<i64>,
    ) -> NewMediaFileLibrary {
        NewMediaFileLibrary {
            media_file_id,
            library_id,
            source_root_path: self.source_root_path.clone(),
            relative_path: self.relative_path.clone(),
            display_title,
            metadata_match_attempted_at,
            media_item_id: None,
            missing_since: None,
            deleted_at: None,
        }
    }
}

fn extracted_metadata_from_existing(existing: &CatalogMediaFile) -> ExtractedMetadata {
    ExtractedMetadata {
        container: existing.container.clone(),
        duration_ms: existing.duration_ms,
        bit_rate: existing.bit_rate,
        width: existing.width,
        height: existing.height,
        video_codec: existing.video_codec.clone(),
        audio_codec: existing.audio_codec.clone(),
        metadata_json: existing.metadata_json.clone(),
        metadata_updated_at: existing.metadata_updated_at,
    }
}

fn extracted_metadata_from_file_row(existing: &MediaFile) -> ExtractedMetadata {
    ExtractedMetadata {
        container: existing.container.clone(),
        duration_ms: existing.duration_ms,
        bit_rate: existing.bit_rate,
        width: existing.width,
        height: existing.height,
        video_codec: existing.video_codec.clone(),
        audio_codec: existing.audio_codec.clone(),
        metadata_json: existing.metadata_json.clone(),
        metadata_updated_at: existing.metadata_updated_at,
    }
}

fn default_metadata(_file: &DiscoveredMediaFile) -> ExtractedMetadata {
    ExtractedMetadata::default()
}

fn extract_metadata(
    file: &DiscoveredMediaFile,
    probe_context: ProbeContext<'_>,
) -> ExtractedMetadata {
    if !matches!(file.media_kind.as_str(), "video" | "audio") {
        return default_metadata(file);
    }

    if !probe_context.enabled {
        return default_metadata(file);
    }

    let output = Command::new(probe_context.ffprobe_path)
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(&file.full_path)
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            parse_ffprobe_metadata(&stdout).unwrap_or_else(|| default_metadata(file))
        }
        _ => default_metadata(file),
    }
}

fn parse_ffprobe_metadata(raw_json: &str) -> Option<ExtractedMetadata> {
    let parsed: Value = serde_json::from_str(raw_json).ok()?;
    let format = parsed.get("format");
    let streams = parsed
        .get("streams")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let video_stream = streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("video"));
    let audio_stream = streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("audio"));

    Some(ExtractedMetadata {
        container: format
            .and_then(|format| format.get("format_name"))
            .and_then(Value::as_str)
            .map(|value| value.split(',').next().unwrap_or(value).trim().to_string())
            .filter(|value| !value.is_empty()),
        duration_ms: format
            .and_then(|format| format.get("duration"))
            .and_then(Value::as_str)
            .and_then(|value| value.parse::<f64>().ok())
            .map(|value| (value * 1000.0).round() as i64),
        bit_rate: format
            .and_then(|format| format.get("bit_rate"))
            .and_then(Value::as_str)
            .and_then(|value| value.parse::<i64>().ok()),
        width: video_stream
            .and_then(|stream| stream.get("width"))
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok()),
        height: video_stream
            .and_then(|stream| stream.get("height"))
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok()),
        video_codec: video_stream
            .and_then(|stream| stream.get("codec_name"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        audio_codec: audio_stream
            .and_then(|stream| stream.get("codec_name"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        metadata_json: Some(raw_json.to_string()),
        metadata_updated_at: Some(current_timestamp()),
    })
}

/// Return audio stream summaries from stored ffprobe JSON.
pub fn audio_tracks_from_metadata_json(raw_json: Option<&str>) -> Option<Vec<MediaAudioTrack>> {
    let parsed: Value = serde_json::from_str(raw_json?).ok()?;
    let streams = parsed.get("streams")?.as_array()?;
    let mut audio_index = 0usize;
    let tracks = streams
        .iter()
        .filter_map(|stream| {
            if stream.get("codec_type").and_then(Value::as_str) != Some("audio") {
                return None;
            }

            let index = audio_index;
            audio_index += 1;
            let codec = stream
                .get("codec_name")
                .and_then(Value::as_str)
                .map(str::to_string);
            let tags = stream.get("tags");
            let language = tags
                .and_then(|tags| tags.get("language"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty() && *value != "und")
                .map(str::to_string);
            let title = tags
                .and_then(|tags| tags.get("title"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let default = stream
                .get("disposition")
                .and_then(|value| value.get("default"))
                .and_then(Value::as_i64)
                .unwrap_or_default()
                == 1;
            let label = title
                .map(str::to_string)
                .or_else(|| {
                    language
                        .as_ref()
                        .map(|language| language.to_ascii_uppercase())
                })
                .or_else(|| codec.as_ref().map(|codec| codec.to_ascii_uppercase()))
                .unwrap_or_else(|| format!("Audio {}", index + 1));

            Some(MediaAudioTrack {
                index,
                label,
                codec,
                language,
                default,
            })
        })
        .collect::<Vec<_>>();

    Some(tracks)
}

/// Select the best source audio stream for the user's preferred languages.
pub fn preferred_audio_stream_index(
    conn: &mut SqliteConnection,
    item_id: i32,
    preferred_languages: &[String],
) -> Result<Option<usize>, diesel::result::Error> {
    let Some(file) = load_backing_media_file(conn, item_id)? else {
        return Ok(None);
    };
    let tracks = audio_tracks_from_metadata_json(file.metadata_json.as_deref()).unwrap_or_default();
    Ok(match_audio_track_index(&tracks, preferred_languages))
}

fn match_audio_track_index(
    tracks: &[MediaAudioTrack],
    preferred_languages: &[String],
) -> Option<usize> {
    if tracks.len() <= 1 {
        return None;
    }

    let preferred = preferred_languages
        .iter()
        .flat_map(|language| audio_language_match_keys(language))
        .collect::<Vec<_>>();
    for language in preferred {
        if let Some(track) = tracks.iter().find(|track| {
            track
                .language
                .as_deref()
                .map(audio_language_match_keys)
                .unwrap_or_default()
                .iter()
                .any(|candidate| candidate == &language)
        }) {
            return Some(track.index);
        }
    }

    tracks
        .iter()
        .find(|track| track.default)
        .map(|track| track.index)
}

fn audio_language_match_keys(language: &str) -> Vec<String> {
    let normalized = language.trim().to_ascii_lowercase().replace('_', "-");
    let primary = normalized.split('-').next().unwrap_or("").to_string();
    let mut keys = Vec::new();
    for key in [
        normalized.as_str(),
        primary.as_str(),
    ] {
        if !key.is_empty() && !keys.iter().any(|entry| entry == key) {
            keys.push(key.to_string());
        }
    }
    let aliases = match primary.as_str() {
        "en" | "eng" | "english" => &["en", "eng", "english"][..],
        "es" | "spa" | "spanish" => &["es", "spa", "spanish"][..],
        "fr" | "fra" | "fre" | "french" => &["fr", "fra", "fre", "french"][..],
        "de" | "deu" | "ger" | "german" => &["de", "deu", "ger", "german"][..],
        "it" | "ita" | "italian" => &["it", "ita", "italian"][..],
        "ja" | "jpn" | "japanese" => &["ja", "jpn", "japanese"][..],
        "pt" | "por" | "portuguese" => &["pt", "por", "portuguese"][..],
        _ => &[][..],
    };
    for alias in aliases {
        if !keys.iter().any(|entry| entry == alias) {
            keys.push((*alias).to_string());
        }
    }
    keys
}

fn to_persisted_file_summary(row: CatalogMediaFile) -> PersistedMediaFileSummary {
    PersistedMediaFileSummary {
        id: row.id,
        library_id: row.library_id,
        relative_path: row.relative_path.clone(),
        file_size: row.file_size,
        modified_at: row.modified_at,
        media_kind: row.media_kind,
        fingerprint_seed: row.fingerprint_seed,
        display_title: row
            .display_title
            .unwrap_or_else(|| fallback_title_from_relative_path(&row.relative_path)),
        container: row.container,
        duration_ms: row.duration_ms,
        width: row.width,
        height: row.height,
        video_codec: row.video_codec,
        audio_codec: row.audio_codec,
        missing_since: row.missing_since,
    }
}

fn to_media_item_summary(item: MediaItem) -> MediaItemSummary {
    let relative_path = item.relative_path.unwrap_or_default();

    MediaItemSummary {
        id: item.id,
        library_id: item.library_id,
        parent_id: item.parent_id,
        item_type: item.item_type.clone(),
        display_title: item.display_title,
        relative_path,
        media_kind: item
            .media_kind
            .unwrap_or_else(|| default_media_kind_for_item_type(&item.item_type).to_string()),
        playable: item.playable,
        child_count: item.child_count,
        season_number: item.season_number,
        episode_number: item.episode_number,
        duration_ms: item.duration_ms,
        width: None,
        height: None,
        genres: Vec::new(),
        overview: None,
        backdrop_url: None,
        logo_url: None,
        has_metadata: false,
        metadata_refresh_state: None,
        metadata_refresh_error: None,
        artwork_updated_at: None,
        modified_at: item.modified_at,
        playback_position_ms: None,
        playback_duration_ms: None,
        missing_since: item.missing_since,
    }
}

fn load_media_item_row(
    conn: &mut SqliteConnection,
    item_id: i32,
) -> Result<Option<MediaItem>, diesel::result::Error> {
    use crate::db::schema::media_items::dsl as media_items_dsl;

    media_items_dsl::media_items
        .filter(media_items_dsl::id.eq(item_id))
        .filter(media_items_dsl::deleted_at.is_null())
        .select(MediaItem::as_select())
        .first(conn)
        .optional()
}

fn get_media_item_summary_without_metadata(
    conn: &mut SqliteConnection,
    item_id: i32,
) -> Result<Option<MediaItemSummary>, diesel::result::Error> {
    Ok(load_media_item_row(conn, item_id)?.map(to_media_item_summary))
}

fn media_item_summary_with_preferred_title(
    conn: &mut SqliteConnection,
    row: MediaItem,
) -> Result<MediaItemSummary, diesel::result::Error> {
    media_item_summary_with_preferred_languages(conn, row, &[])
}

fn media_item_summary_with_preferred_languages(
    conn: &mut SqliteConnection,
    row: MediaItem,
    preferred_languages: &[String],
) -> Result<MediaItemSummary, diesel::result::Error> {
    let mut summary = to_media_item_summary(row);
    let link = prioritized_primary_metadata_links_for_item(conn, &summary, preferred_languages)?
        .into_iter()
        .next();
    let link = link.as_ref().map(summary_metadata_link_from_full_link);
    apply_primary_metadata_link(&mut summary, link.as_ref());

    Ok(summary)
}

fn summary_metadata_link_from_full_link(link: &ItemMetadataLink) -> SummaryMetadataLink {
    SummaryMetadataLink {
        media_item_id: link.media_item_id,
        title: link.title.clone(),
        overview: link.overview.clone(),
        genres_json: link.genres_json.clone(),
        logo_url: link.logo_url.clone(),
        cached_logo_path: link.cached_logo_path.clone(),
        backdrop_url: link.backdrop_url.clone(),
        cached_backdrop_path: link.cached_backdrop_path.clone(),
        refresh_state: link.refresh_state.clone(),
        refresh_error: link.refresh_error.clone(),
        updated_at: link.updated_at,
        locale_key: link.locale_key.clone(),
    }
}

fn apply_primary_metadata_link(
    summary: &mut MediaItemSummary,
    link: Option<&SummaryMetadataLink>,
) {
    let Some(link) = link else {
        return;
    };

    if let Some(title) = link
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
    {
        summary.display_title = title.to_string();
    }
    summary.genres = link
        .genres_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .unwrap_or_default();
    summary.overview = link.overview.clone();
    if link.cached_backdrop_path.is_some() || link.backdrop_url.is_some() {
        summary.backdrop_url = Some(format!(
            "/api/v1/items/{}/artwork?kind=backdrop",
            summary.id
        ));
    }
    if link.cached_logo_path.is_some() || link.logo_url.is_some() {
        summary.logo_url = Some(format!("/api/v1/items/{}/artwork?kind=logo", summary.id));
    }
    summary.has_metadata = true;
    summary.metadata_refresh_state = Some(link.refresh_state.clone());
    summary.metadata_refresh_error = link.refresh_error.clone();
    summary.artwork_updated_at = link.updated_at;
}

fn preferred_metadata_links_by_item_id(
    conn: &mut SqliteConnection,
    item_ids: &[i32],
    preferred_languages: &[String],
) -> Result<HashMap<i32, SummaryMetadataLink>, diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as item_metadata_links_dsl;

    if item_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = item_metadata_links_dsl::item_metadata_links
        .filter(item_metadata_links_dsl::media_item_id.eq_any(item_ids))
        .filter(item_metadata_links_dsl::relation_kind.eq("primary"))
        .order((
            item_metadata_links_dsl::media_item_id.asc(),
            item_metadata_links_dsl::updated_at.desc(),
            item_metadata_links_dsl::id.desc(),
        ))
        .select((
            item_metadata_links_dsl::media_item_id,
            item_metadata_links_dsl::title,
            item_metadata_links_dsl::overview,
            item_metadata_links_dsl::genres_json,
            item_metadata_links_dsl::logo_url,
            item_metadata_links_dsl::cached_logo_path,
            item_metadata_links_dsl::backdrop_url,
            item_metadata_links_dsl::cached_backdrop_path,
            item_metadata_links_dsl::refresh_state,
            item_metadata_links_dsl::refresh_error,
            item_metadata_links_dsl::updated_at,
            item_metadata_links_dsl::locale_key,
        ))
        .load::<(
            i32,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            Option<String>,
            Option<i64>,
            String,
        )>(conn)?
        .into_iter()
        .map(
            |(
                media_item_id,
                title,
                overview,
                genres_json,
                logo_url,
                cached_logo_path,
                backdrop_url,
                cached_backdrop_path,
                refresh_state,
                refresh_error,
                updated_at,
                locale_key,
            )| SummaryMetadataLink {
                media_item_id,
                title,
                overview,
                genres_json,
                logo_url,
                cached_logo_path,
                backdrop_url,
                cached_backdrop_path,
                refresh_state,
                refresh_error,
                updated_at,
                locale_key,
            },
        )
        .collect::<Vec<_>>();

    let language_rank = preferred_languages
        .iter()
        .enumerate()
        .map(|(index, language)| (normalize_locale_key(language), index))
        .collect::<HashMap<_, _>>();
    let default_rank = preferred_languages.len();
    let mut by_item_id = HashMap::new();
    for row in rows {
        let next_rank = language_rank
            .get(&normalize_locale_key(&row.locale_key))
            .copied()
            .unwrap_or(default_rank);
        match by_item_id.entry(row.media_item_id) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert((next_rank, row));
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                if next_rank < entry.get().0 {
                    entry.insert((next_rank, row));
                }
            }
        }
    }

    Ok(by_item_id
        .into_iter()
        .map(|(item_id, (_rank, link))| (item_id, link))
        .collect())
}

/// Return the best metadata link for a media item, preferring links that
/// structurally match the item's show/season/episode identity.
pub fn get_preferred_item_metadata_link(
    conn: &mut SqliteConnection,
    item_id: i32,
) -> Result<Option<ItemMetadataLink>, diesel::result::Error> {
    let Some(item) = get_media_item_summary_without_metadata(conn, item_id)? else {
        return Ok(None);
    };

    preferred_item_metadata_link_for_summary(conn, &item)
}

fn preferred_item_metadata_link_for_summary(
    conn: &mut SqliteConnection,
    item: &MediaItemSummary,
) -> Result<Option<ItemMetadataLink>, diesel::result::Error> {
    let mut links = prioritized_primary_metadata_links_for_item(conn, item, &[])?;
    if links.is_empty() {
        return Ok(None);
    }

    let expected_media_type = expected_metadata_media_type(item);
    if let Some(expected_external_id) = expected_tmdb_external_id_for_item(conn, item)? {
        if let Some(index) = links.iter().position(|link| {
            link.provider_id == MetadataProviderId::Tmdb.as_storage_value()
                && link.external_id == expected_external_id
        }) {
            return Ok(Some(links.swap_remove(index)));
        }

        if let Some(expected_media_type) = expected_media_type {
            links.retain(|link| {
                !(link.provider_id == MetadataProviderId::Tmdb.as_storage_value()
                    && link.media_type.as_deref() == Some(expected_media_type))
            });
            if links.is_empty() {
                return Ok(None);
            }
        }
    }

    if let Some(expected_media_type) = expected_media_type {
        if let Some(index) = links
            .iter()
            .position(|link| link.media_type.as_deref() == Some(expected_media_type))
        {
            return Ok(Some(links.swap_remove(index)));
        }
    }

    Ok(links.into_iter().next())
}

fn expected_metadata_media_type(item: &MediaItemSummary) -> Option<&'static str> {
    match item.item_type.as_str() {
        "episode" => Some("tv_episode"),
        "season" => Some("tv_season"),
        "show" => Some("tv"),
        "movie" => Some("movie"),
        _ => None,
    }
}

fn expected_tmdb_external_id_for_item(
    conn: &mut SqliteConnection,
    item: &MediaItemSummary,
) -> Result<Option<String>, diesel::result::Error> {
    match item.item_type.as_str() {
        "season" => {
            let Some(show_external_id) = show_tmdb_external_id_for_item(conn, item)? else {
                return Ok(None);
            };
            let Some(season_number) = item.season_number else {
                return Ok(None);
            };
            Ok(Some(format!(
                "tv:{show_external_id}:season:{season_number}"
            )))
        }
        "episode" => {
            let Some(show_external_id) = show_tmdb_external_id_for_item(conn, item)? else {
                return Ok(None);
            };
            let (Some(season_number), Some(episode_number)) =
                (item.season_number, item.episode_number)
            else {
                return Ok(None);
            };
            Ok(Some(format!(
                "tv:{show_external_id}:season:{season_number}:episode:{episode_number}"
            )))
        }
        _ => Ok(None),
    }
}

fn show_tmdb_external_id_for_item(
    conn: &mut SqliteConnection,
    item: &MediaItemSummary,
) -> Result<Option<String>, diesel::result::Error> {
    let mut current_parent_id = item.parent_id;
    while let Some(parent_id) = current_parent_id {
        let Some(parent) = get_media_item_summary_without_metadata(conn, parent_id)? else {
            return Ok(None);
        };
        if parent.item_type == "show" {
            let link = preferred_item_metadata_link_for_summary(conn, &parent)?;
            if let Some(link) = link.filter(|link| {
                link.provider_id == MetadataProviderId::Tmdb.as_storage_value()
                    && link.media_type.as_deref() == Some("tv")
            }) {
                return Ok(Some(link.external_id));
            }
            return Ok(None);
        }
        current_parent_id = parent.parent_id;
    }

    Ok(None)
}

fn media_library_metadata_provider_order(
    conn: &mut SqliteConnection,
    library_id: i32,
) -> Result<Vec<MetadataProviderId>, diesel::result::Error> {
    use crate::db::schema::media_libraries::dsl as media_libraries_dsl;

    let library = media_libraries_dsl::media_libraries
        .filter(media_libraries_dsl::id.eq(library_id))
        .select(MediaLibrary::as_select())
        .first::<MediaLibrary>(conn)
        .optional()?;

    Ok(library
        .map(media_library_settings_from_row)
        .map(|settings| settings.metadata_providers)
        .unwrap_or_else(|| vec![MetadataProviderId::Tmdb]))
}

fn metadata_language_priority(preferred_languages: &[String]) -> HashMap<String, usize> {
    let mut languages = preferred_languages
        .iter()
        .map(|language| normalize_locale_key(language))
        .filter(|language| !language.is_empty())
        .collect::<Vec<_>>();
    if !languages
        .iter()
        .any(|language| language == DEFAULT_METADATA_LOCALE)
    {
        languages.push(DEFAULT_METADATA_LOCALE.to_string());
    }

    languages
        .into_iter()
        .enumerate()
        .map(|(index, language)| (language, index))
        .collect()
}

fn prioritized_metadata_links_for_item(
    conn: &mut SqliteConnection,
    item_id: i32,
    library_id: i32,
    preferred_languages: &[String],
) -> Result<Vec<ItemMetadataLink>, diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as item_metadata_links_dsl;

    let provider_order = media_library_metadata_provider_order(conn, library_id)?;
    let provider_rank = provider_order
        .iter()
        .enumerate()
        .map(|(index, provider)| (provider.as_storage_value().to_string(), index))
        .collect::<HashMap<_, _>>();
    let fallback_provider_rank = provider_rank.len();
    let language_rank = metadata_language_priority(preferred_languages);
    let fallback_language_rank = language_rank.len();

    let mut rows = item_metadata_links_dsl::item_metadata_links
        .filter(item_metadata_links_dsl::media_item_id.eq(item_id))
        .filter(item_metadata_links_dsl::relation_kind.eq_any(["primary", "secondary"]))
        .select(ItemMetadataLink::as_select())
        .load::<ItemMetadataLink>(conn)?
        .into_iter()
        .filter(|link| provider_rank.contains_key(&link.provider_id))
        .collect::<Vec<_>>();

    rows.sort_by(|left, right| {
        let left_provider_rank = provider_rank
            .get(&left.provider_id)
            .copied()
            .unwrap_or(fallback_provider_rank);
        let right_provider_rank = provider_rank
            .get(&right.provider_id)
            .copied()
            .unwrap_or(fallback_provider_rank);
        let left_language_rank = language_rank
            .get(&normalize_locale_key(&left.locale_key))
            .copied()
            .unwrap_or(fallback_language_rank);
        let right_language_rank = language_rank
            .get(&normalize_locale_key(&right.locale_key))
            .copied()
            .unwrap_or(fallback_language_rank);
        let left_relation_rank = if left.relation_kind == "primary" { 0 } else { 1 };
        let right_relation_rank = if right.relation_kind == "primary" { 0 } else { 1 };

        left_provider_rank
            .cmp(&right_provider_rank)
            .then_with(|| left_relation_rank.cmp(&right_relation_rank))
            .then_with(|| left_language_rank.cmp(&right_language_rank))
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| right.id.cmp(&left.id))
    });

    let mut seen = HashSet::<(String, String)>::new();
    Ok(rows
        .into_iter()
        .filter(|link| seen.insert((link.provider_id.clone(), link.relation_kind.clone())))
        .collect())
}

fn prioritized_primary_metadata_links_for_item(
    conn: &mut SqliteConnection,
    item: &MediaItemSummary,
    preferred_languages: &[String],
) -> Result<Vec<ItemMetadataLink>, diesel::result::Error> {
    Ok(
        prioritized_metadata_links_for_item(conn, item.id, item.library_id, preferred_languages)?
            .into_iter()
            .filter(|link| link.relation_kind == "primary")
            .collect(),
    )
}

/// Return the preferred metadata link that can serve the requested artwork kind.
pub fn get_preferred_item_artwork_metadata_link_for_languages(
    conn: &mut SqliteConnection,
    item_id: i32,
    preferred_languages: &[String],
    artwork_kind: ArtworkKind,
) -> Result<Option<ItemMetadataLink>, diesel::result::Error> {
    let mut current_item_id = Some(item_id);
    let mut visited = HashSet::new();
    while let Some(current_id) = current_item_id {
        if !visited.insert(current_id) {
            break;
        }
        let Some(item) = get_media_item_summary_without_metadata(conn, current_id)? else {
            return Ok(None);
        };
        if let Some(link) = prioritized_metadata_links_for_item(
            conn,
            current_id,
            item.library_id,
            preferred_languages,
        )?
        .into_iter()
        .find(|link| metadata_link_has_artwork(link, artwork_kind))
        {
            return Ok(Some(link));
        }
        current_item_id = item.parent_id;
    }

    Ok(None)
}

fn metadata_link_has_artwork(
    link: &ItemMetadataLink,
    artwork_kind: ArtworkKind,
) -> bool {
    match artwork_kind {
        ArtworkKind::Poster => link.cached_artwork_path.is_some() || link.artwork_url.is_some(),
        ArtworkKind::Backdrop => link.cached_backdrop_path.is_some() || link.backdrop_url.is_some(),
        ArtworkKind::Logo => link.cached_logo_path.is_some() || link.logo_url.is_some(),
    }
}

fn to_media_item_detail(
    item: MediaItem,
    backing_file: Option<&CatalogMediaFile>,
) -> MediaItemDetail {
    let relative_path = item.relative_path.unwrap_or_default();

    MediaItemDetail {
        id: item.id,
        library_id: item.library_id,
        parent_id: item.parent_id,
        item_type: item.item_type.clone(),
        display_title: item.display_title,
        relative_path,
        file_size: item.file_size,
        modified_at: item.modified_at,
        media_kind: item
            .media_kind
            .unwrap_or_else(|| default_media_kind_for_item_type(&item.item_type).to_string()),
        playable: item.playable,
        child_count: item.child_count,
        season_number: item.season_number,
        episode_number: item.episode_number,
        container: backing_file.and_then(|file| file.container.clone()),
        duration_ms: item.duration_ms,
        bit_rate: backing_file.and_then(|file| file.bit_rate),
        width: backing_file.and_then(|file| file.width),
        height: backing_file.and_then(|file| file.height),
        video_codec: backing_file.and_then(|file| file.video_codec.clone()),
        audio_codec: backing_file.and_then(|file| file.audio_codec.clone()),
        metadata_json: backing_file.and_then(|file| file.metadata_json.clone()),
        metadata_updated_at: backing_file.and_then(|file| file.metadata_updated_at),
        poster_url: None,
        backdrop_url: None,
        theme_song_url: None,
        tagline: None,
        overview: None,
        genres: Vec::new(),
        release_year: None,
        logo_url: None,
        rating: None,
        content_rating: None,
        linked_media_type: None,
        has_metadata: false,
        metadata_refresh_state: None,
        metadata_refresh_error: None,
        artwork_updated_at: None,
        trailer_title: None,
        trailer_url: None,
        audio_tracks: Vec::new(),
        subtitle_tracks: Vec::new(),
        hierarchy: Vec::new(),
        children: Vec::new(),
        playback_position_ms: None,
        playback_duration_ms: None,
        missing_since: item.missing_since,
    }
}

fn default_media_kind_for_item_type(item_type: &str) -> &'static str {
    match item_type {
        "track" => "audio",
        "photo" => "image",
        "book" => "book",
        _ => "video",
    }
}

fn load_backing_media_file(
    conn: &mut SqliteConnection,
    item_id: i32,
) -> Result<Option<CatalogMediaFile>, diesel::result::Error> {
    load_catalog_file_for_item(conn, item_id)
}

fn load_media_item_hierarchy(
    conn: &mut SqliteConnection,
    item: &MediaItem,
    preferred_languages: &[String],
) -> Result<Vec<MediaItemSummary>, diesel::result::Error> {
    use crate::db::schema::media_items::dsl as media_items_dsl;

    let mut hierarchy = Vec::new();
    let mut next_parent_id = item.parent_id;

    while let Some(parent_id) = next_parent_id {
        let Some(parent) = media_items_dsl::media_items
            .filter(media_items_dsl::id.eq(parent_id))
            .filter(media_items_dsl::deleted_at.is_null())
            .select(MediaItem::as_select())
            .first(conn)
            .optional()?
        else {
            break;
        };

        next_parent_id = parent.parent_id;
        hierarchy.push(media_item_summary_with_preferred_languages(
            conn,
            parent,
            preferred_languages,
        )?);
    }

    hierarchy.reverse();
    Ok(hierarchy)
}

/// Return direct child summaries for one browser-facing media item.
pub fn list_media_item_children(
    conn: &mut SqliteConnection,
    item_id: i32,
) -> Result<Vec<MediaItemSummary>, diesel::result::Error> {
    list_media_item_children_with_preferred_languages(conn, item_id, &[])
}

/// Return direct child summaries for one browser-facing media item and preferred languages.
pub fn list_media_item_children_with_preferred_languages(
    conn: &mut SqliteConnection,
    item_id: i32,
    preferred_languages: &[String],
) -> Result<Vec<MediaItemSummary>, diesel::result::Error> {
    use crate::db::schema::media_items::dsl as media_items_dsl;

    let rows = media_items_dsl::media_items
        .filter(media_items_dsl::parent_id.eq(item_id))
        .filter(media_items_dsl::deleted_at.is_null())
        .order((
            media_items_dsl::season_number.asc(),
            media_items_dsl::episode_number.asc(),
            media_items_dsl::display_title.asc(),
            media_items_dsl::relative_path.asc(),
        ))
        .select(MediaItem::as_select())
        .load::<MediaItem>(conn)?;

    let metadata_links = preferred_metadata_links_by_item_id(
        conn,
        &rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        preferred_languages,
    )?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let mut summary = to_media_item_summary(row);
        let summary_id = summary.id;
        apply_primary_metadata_link(&mut summary, metadata_links.get(&summary_id));
        items.push(summary);
    }

    Ok(items)
}

fn display_name(
    library: &MediaLibrarySettings,
    path: &str,
) -> String {
    if !library.name.trim().is_empty() {
        return library.name.trim().to_string();
    }

    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Unnamed library".into())
}

fn scan_directory(
    root: &Path,
    path: &Path,
    recursive: bool,
    library_kind: &MediaLibraryKind,
) -> io::Result<(FileCounters, Vec<DiscoveredMediaFile>)> {
    let mut counters = FileCounters::default();
    let mut files = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();

        if entry_path.is_dir() {
            if recursive {
                let (nested, nested_files) = scan_directory(root, &entry_path, true, library_kind)?;
                counters.total_files += nested.total_files;
                counters.video_files += nested.video_files;
                counters.audio_files += nested.audio_files;
                counters.image_files += nested.image_files;
                counters.book_files += nested.book_files;
                counters.other_files += nested.other_files;
                files.extend(nested_files);
            }
            continue;
        }

        if entry_path.is_file() {
            let kind = classify_file(&entry_path);
            if !should_include_library_item(&entry_path, kind, library_kind) {
                continue;
            }

            counters.total_files += 1;
            match kind {
                FileKind::Video => counters.video_files += 1,
                FileKind::Audio => counters.audio_files += 1,
                FileKind::Image => counters.image_files += 1,
                FileKind::Book => counters.book_files += 1,
                FileKind::Other => counters.other_files += 1,
            }

            let metadata = entry.metadata()?;
            let file_size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
            let modified_at = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .and_then(|duration| i64::try_from(duration.as_secs()).ok());
            let relative_path = normalize_relative_path(root, &entry_path);
            let media_kind = kind.as_storage_value().to_string();
            let raw_default_title = entry_path
                .file_stem()
                .and_then(|value| value.to_str())
                .map(|value| value.to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| fallback_title_from_relative_path(&relative_path));
            let default_title = match library_kind {
                MediaLibraryKind::Movies => movie_display_title_from_name(&raw_default_title),
                _ => raw_default_title,
            };

            files.push(DiscoveredMediaFile {
                full_path: entry_path,
                source_root_path: root.to_string_lossy().to_string(),
                fingerprint_seed: format!(
                    "{}:{}:{}:{}",
                    root.to_string_lossy(),
                    relative_path,
                    file_size,
                    modified_at.unwrap_or_default()
                ),
                relative_path,
                file_size,
                modified_at,
                media_kind,
                default_title,
            });
        }
    }

    Ok((counters, files))
}

fn detect_binary(binary: &str) -> BinaryCapability {
    let output = Command::new(binary).arg("-version").output();

    match output {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty());

            BinaryCapability {
                configured_path: binary.to_string(),
                available: true,
                version,
                error: None,
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let error = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("Process exited with status {}", output.status)
            };

            BinaryCapability {
                configured_path: binary.to_string(),
                available: false,
                version: None,
                error: Some(error),
            }
        }
        Err(error) => BinaryCapability {
            configured_path: binary.to_string(),
            available: false,
            version: None,
            error: Some(error.to_string()),
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileKind {
    Video,
    Audio,
    Image,
    Book,
    Other,
}

impl FileKind {
    fn as_storage_value(&self) -> &'static str {
        match self {
            FileKind::Video => "video",
            FileKind::Audio => "audio",
            FileKind::Image => "image",
            FileKind::Book => "book",
            FileKind::Other => "other",
        }
    }
}

fn classify_file(path: &Path) -> FileKind {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    match extension.as_deref() {
        Some("mkv" | "mp4" | "avi" | "mov" | "wmv" | "m4v" | "webm" | "ts") => FileKind::Video,
        Some("mp3" | "flac" | "aac" | "wav" | "ogg" | "m4a" | "opus") => FileKind::Audio,
        Some("jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tiff") => FileKind::Image,
        Some("pdf" | "epub" | "cbz" | "cbr" | "mobi") => FileKind::Book,
        _ => FileKind::Other,
    }
}

fn should_include_library_item(
    path: &Path,
    kind: FileKind,
    library_kind: &MediaLibraryKind,
) -> bool {
    match library_kind {
        MediaLibraryKind::Movies | MediaLibraryKind::Shows | MediaLibraryKind::HomeVideos => {
            kind == FileKind::Video
        }
        MediaLibraryKind::Music => kind == FileKind::Audio,
        MediaLibraryKind::Photos => kind == FileKind::Image,
        MediaLibraryKind::Books => kind == FileKind::Book,
        MediaLibraryKind::Mixed => {
            matches!(
                kind,
                FileKind::Video | FileKind::Audio | FileKind::Image | FileKind::Book
            ) && !is_named_theme_asset(path)
        }
    }
}

fn parse_library_storage_paths(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    if trimmed.starts_with('[') {
        serde_json::from_str::<Vec<String>>(trimmed).unwrap_or_default()
    } else if trimmed.is_empty() {
        Vec::new()
    } else {
        vec![trimmed.to_string()]
    }
}

fn media_file_inventory_key(file: &CatalogMediaFile) -> String {
    format!("{}\u{1f}{}", file.source_root_path, file.relative_path)
}

fn discovered_media_file_inventory_key(file: &DiscoveredMediaFile) -> String {
    format!("{}\u{1f}{}", file.source_root_path, file.relative_path)
}

#[derive(Debug, Default)]
struct ResolvedItemAssets {
    poster_path: Option<PathBuf>,
    backdrop_path: Option<PathBuf>,
    theme_song_path: Option<PathBuf>,
    subtitle_paths: Vec<PathBuf>,
}

fn discover_item_assets(
    item_id: i32,
    source_path: &Path,
    data_dir: &str,
) -> ResolvedItemAssets {
    let managed_dir = managed_item_asset_dir(data_dir, item_id);
    let source_dir = source_path.parent().unwrap_or_else(|| Path::new(""));
    let source_stem = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();

    let poster_path = find_first_existing_asset(
        &[
            managed_dir.as_path(),
            source_dir,
        ],
        &[
            "poster.jpg",
            "poster.jpeg",
            "poster.png",
            "poster.webp",
            "folder.jpg",
            "folder.jpeg",
            "folder.png",
            "cover.jpg",
            "cover.png",
        ],
        &[
            source_stem.clone(),
            format!("{}-poster", source_stem),
        ],
        &["jpg", "jpeg", "png", "webp"],
    );
    let backdrop_path = find_first_existing_asset(
        &[
            managed_dir.as_path(),
            source_dir,
        ],
        &[
            "backdrop.jpg",
            "backdrop.jpeg",
            "backdrop.png",
            "fanart.jpg",
            "fanart.jpeg",
            "fanart.png",
            "background.jpg",
            "background.png",
        ],
        &[
            format!("{}-backdrop", source_stem),
            format!("{}-fanart", source_stem),
        ],
        &["jpg", "jpeg", "png", "webp"],
    );
    let theme_song_path = find_first_existing_asset(
        &[
            managed_dir.as_path(),
            source_dir,
        ],
        &[
            "theme.mp3",
            "theme.flac",
            "theme.m4a",
            "theme.ogg",
            "theme.opus",
            "theme.wav",
        ],
        &[],
        &[],
    );

    let subtitle_paths = collect_subtitle_assets(
        &[
            managed_dir.as_path(),
            source_dir,
        ],
        &source_stem,
    );

    ResolvedItemAssets {
        poster_path,
        backdrop_path,
        theme_song_path,
        subtitle_paths,
    }
}

fn managed_item_asset_dir(
    data_dir: &str,
    item_id: i32,
) -> PathBuf {
    let item_hex = format!("{:08x}", item_id.max(0));
    let shard = &item_hex[0..2];
    Path::new(data_dir)
        .join("item_assets")
        .join(shard)
        .join(item_hex)
}

fn find_first_existing_asset(
    directories: &[&Path],
    fixed_names: &[&str],
    stem_names: &[String],
    extensions: &[&str],
) -> Option<PathBuf> {
    for directory in directories {
        for fixed_name in fixed_names {
            let candidate = directory.join(fixed_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }

        for stem_name in stem_names {
            for extension in extensions {
                let candidate = directory.join(format!("{}.{}", stem_name, extension));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

fn collect_subtitle_assets(
    directories: &[&Path],
    source_stem: &str,
) -> Vec<PathBuf> {
    let mut subtitle_paths = Vec::new();
    let mut seen = HashSet::new();

    for directory in directories {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || !is_subtitle_extension(&path) {
                continue;
            }

            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .map(|value| value.to_ascii_lowercase())
                .unwrap_or_default();
            if stem != source_stem && !stem.starts_with(&format!("{}.", source_stem)) {
                continue;
            }

            let key = path.to_string_lossy().to_string();
            if seen.insert(key) {
                subtitle_paths.push(path);
            }
        }
    }

    subtitle_paths.sort();
    subtitle_paths
}

fn subtitle_label_from_path(
    source_path: &Path,
    subtitle_path: &Path,
) -> String {
    let source_stem = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    let subtitle_stem = subtitle_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let normalized_subtitle_stem = subtitle_stem.to_ascii_lowercase();

    if let Some(suffix) = normalized_subtitle_stem.strip_prefix(&format!("{}.", source_stem)) {
        if !suffix.trim().is_empty() {
            return suffix.to_ascii_uppercase();
        }
    }

    subtitle_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_uppercase())
        .unwrap_or_else(|| "Subtitle".into())
}

fn is_named_theme_asset(path: &Path) -> bool {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("theme"))
        .unwrap_or(false)
}

fn is_subtitle_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref(),
        Some("srt" | "vtt" | "ass" | "ssa" | "sub")
    )
}

fn normalize_relative_path(
    root: &Path,
    path: &Path,
) -> String {
    let relative: PathBuf = path.strip_prefix(root).unwrap_or(path).to_path_buf();
    relative.to_string_lossy().replace('\\', "/")
}

fn fallback_title_from_relative_path(relative_path: &str) -> String {
    Path::new(relative_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| relative_path.to_string())
}

fn movie_display_title_from_name(value: &str) -> String {
    static BRACKETED_TAG_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"[\{\[]([^\}\]]*)[\}\]]").unwrap());
    static YEAR_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"\b(19\d{2}|20\d{2}|21\d{2})\b").unwrap());
    static PARENTHETICAL_YEAR_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"[\(\[]\s*(19\d{2}|20\d{2}|21\d{2})\s*[\)\]]").unwrap());
    static DASH_FORMAT_SUFFIX_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(
            r"(?i)\s+[-–]\s+(?:bluray|blu-ray|brrip|web[- ]?dl|webrip|remux|dvdrip|hdtv|uhd|dvd|proper|repack|extended|unrated|director'?s cut|theatrical|final cut)?(?:[\s._-]*(?:2160p|1080p|720p|480p|4k|uhd|hdr|dv|x264|x265|h264|h265|hevc|av1|aac|dts|truehd|atmos|remux|bluray|blu-ray|web[- ]?dl|webrip|brrip|dvdrip))*\s*$",
        )
        .unwrap()
    });
    static NOISE_TOKEN_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(
            r"(?i)\b(2160p|1080p|720p|480p|4k|uhd|x264|x265|h264|h265|hevc|av1|hdr|dv|webrip|web[- ]?dl|bluray|blu-ray|brrip|dvdrip|remux|aac|dts|truehd|atmos)\b",
        )
        .unwrap()
    });
    static TITLE_COLON_DASH_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s*-\s+").unwrap());

    let without_tags = BRACKETED_TAG_REGEX.replace_all(value, " ");
    let mut normalized = DASH_FORMAT_SUFFIX_REGEX
        .replace(&without_tags, " ")
        .to_string();
    normalized = PARENTHETICAL_YEAR_REGEX
        .replace(&normalized, " ")
        .to_string();
    normalized = normalized.replace(['.', '_'], " ");
    if let Some(year_match) = YEAR_REGEX.find(&normalized) {
        if !normalized[..year_match.start()].trim().is_empty() {
            normalized = normalized[..year_match.start()].to_string();
        }
    }
    normalized = TITLE_COLON_DASH_REGEX
        .replace_all(&normalized, ": ")
        .to_string();
    normalized = NOISE_TOKEN_REGEX.replace_all(&normalized, " ").to_string();
    let cleaned = normalized
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|character: char| !character.is_ascii_alphanumeric())
        .to_string();

    if cleaned.is_empty() { value.to_string() } else { cleaned }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(
        id: i32,
        parent_id: Option<i32>,
        item_type: &str,
        title: &str,
        child_count: i32,
        duration_ms: Option<i64>,
        modified_at: Option<i64>,
    ) -> MediaItemSummary {
        MediaItemSummary {
            id,
            library_id: 1,
            parent_id,
            item_type: item_type.to_string(),
            display_title: title.to_string(),
            relative_path: title.to_string(),
            media_kind: "video".to_string(),
            playable: child_count == 0,
            child_count,
            season_number: None,
            episode_number: None,
            duration_ms,
            width: None,
            height: None,
            genres: Vec::new(),
            overview: None,
            backdrop_url: None,
            logo_url: None,
            has_metadata: false,
            metadata_refresh_state: None,
            metadata_refresh_error: None,
            artwork_updated_at: None,
            modified_at,
            playback_position_ms: None,
            playback_duration_ms: None,
            missing_since: None,
        }
    }

    #[test]
    fn recommended_collapses_show_children_to_single_show() {
        let show = summary(
            10,
            None,
            "show",
            "Example Show",
            1,
            Some(7_200_000),
            Some(10),
        );
        let season = summary(
            11,
            Some(10),
            "season",
            "Season 1",
            1,
            Some(7_200_000),
            Some(20),
        );
        let episode = summary(
            12,
            Some(11),
            "episode",
            "Episode 1",
            0,
            Some(3_600_000),
            Some(30),
        );
        let movie = summary(
            20,
            None,
            "movie",
            "Example Movie",
            0,
            Some(5_400_000),
            Some(40),
        );

        let recommended = sort_recommended(&[season, episode, show, movie], &[]);

        assert_eq!(
            recommended
                .iter()
                .map(|item| item.item_type.as_str())
                .collect::<Vec<_>>(),
            vec!["show", "movie"]
        );
        assert_eq!(
            recommended
                .iter()
                .map(|item| item.display_title.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Example Show",
                "Example Movie"
            ]
        );
    }

    #[test]
    fn recommended_excludes_show_when_child_is_continue_watching() {
        let show = summary(
            10,
            None,
            "show",
            "Example Show",
            1,
            Some(7_200_000),
            Some(10),
        );
        let season = summary(
            11,
            Some(10),
            "season",
            "Season 1",
            1,
            Some(7_200_000),
            Some(20),
        );
        let episode = summary(
            12,
            Some(11),
            "episode",
            "Episode 1",
            0,
            Some(3_600_000),
            Some(30),
        );
        let movie = summary(
            20,
            None,
            "movie",
            "Example Movie",
            0,
            Some(5_400_000),
            Some(40),
        );

        let recommended = sort_recommended(
            &[
                show,
                season,
                episode.clone(),
                movie,
            ],
            std::slice::from_ref(&episode),
        );

        assert_eq!(recommended.len(), 1);
        assert_eq!(recommended[0].display_title, "Example Movie");
    }

    #[test]
    fn home_shelf_sorters_do_not_cap_at_twelve_items() {
        let items = (0..14)
            .map(|index| {
                summary(
                    index + 1,
                    None,
                    "movie",
                    &format!("Movie {index:02}"),
                    0,
                    Some(1_000 + i64::from(index)),
                    Some(i64::from(index)),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(sort_recently_added(&items).len(), 14);
        assert_eq!(sort_recommended(&items, &[]).len(), 14);
    }
}
