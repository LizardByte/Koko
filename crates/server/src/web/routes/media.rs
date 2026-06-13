//! Media and system discovery routes.

// lib imports
use std::collections::{
    HashMap,
    HashSet,
};
use std::io::SeekFrom;
use std::path::PathBuf;
use std::sync::atomic::{
    AtomicBool,
    AtomicU64,
    Ordering,
};

use once_cell::sync::Lazy;
use rocket::delete;
use rocket::fs::NamedFile;
use rocket::get;
use rocket::http::{
    ContentType,
    Status,
};
use rocket::outcome::Outcome;
use rocket::post;
use rocket::request::{
    FromRequest,
    Request,
};
use rocket::response::stream::ReaderStream;
use rocket::response::{
    self,
    Responder,
    Response,
};
use rocket::serde::Deserialize;
use rocket::serde::json::Json;
use rocket::tokio::fs::File;
use rocket::tokio::io::{
    AsyncReadExt,
    AsyncSeekExt,
    Take,
};
use rocket::tokio::process::ChildStdout;
use rocket_okapi::openapi;
use schemars::JsonSchema;
use serde::Serialize;
use strsim::normalized_levenshtein;

// local imports
use crate::auth::UserGuard;
use crate::config::{
    MetadataProviderId,
    Settings,
    current_settings,
};
use crate::db::DbConn;
use crate::db::models::ItemMetadataLink;
use crate::globals;
use crate::media::{
    MediaHome,
    MediaItemDetail,
    MediaItemSummary,
    PersistedLibrarySummary,
    PersistedMediaFileSummary,
    PlaybackDecision,
    ShowMetadataDescendantPlan,
    ShowMetadataEpisodePlan,
    ShowMetadataSeasonPlan,
    TranscodingCapability,
    apply_user_playback_context_to_detail,
    delete_missing_media_items,
    get_item_secondary_provider_references,
    get_item_youtube_theme_collection_references,
    get_library_files,
    get_library_metadata_languages,
    get_library_metadata_providers,
    get_media_home_with_preferred_languages,
    get_media_item,
    get_media_item_summary,
    get_media_item_with_preferred_languages,
    get_persisted_library_summaries,
    get_playback_decision,
    get_preferred_item_artwork_metadata_link_for_languages,
    get_preferred_item_metadata_link,
    inspect_transcoding_capability,
    library_exists,
    list_automatic_metadata_candidates,
    list_automatic_metadata_refresh_candidates,
    list_library_settings,
    list_media_item_children,
    list_media_items,
    list_media_items_for_user_with_preferred_languages,
    mark_metadata_match_attempted,
    preferred_audio_stream_index,
    resolve_item_subtitle_path,
    resolve_item_theme_song_path,
    resolve_local_item_artwork_path,
    resolve_media_item_source_path,
    search_media_items_for_user_with_preferred_languages,
    sync_persisted_library_catalog_for_library,
    upsert_playback_progress,
    upsert_show_metadata_descendant_items,
    user_can_access_library,
};
use crate::metadata::{
    ArtworkKind,
    DEFAULT_METADATA_LOCALE,
    ItemMetadataSummary,
    MetadataCollectionSummary,
    MetadataPersonCreditSummary,
    MetadataPersonEnrichmentTarget,
    MetadataPersonSummary,
    MetadataProviderRole,
    MetadataProviderStatus,
    MetadataSearchResult,
    MetadataSnapshotFetchOptions,
    ProviderDescendantTarget,
    ProviderEpisodeMetadataSnapshotFetch,
    ProviderMetadataPerson,
    StoredMetadataSnapshot,
    expected_artwork_cache_path,
    fetch_provider_episode_metadata_snapshot_for_locale_with_options,
    fetch_provider_metadata_snapshot_for_locale_with_options,
    fetch_provider_person_metadata_for_locale,
    fetch_provider_season_metadata_snapshot_for_locale_with_options,
    fetch_provider_secondary_collection_metadata,
    fetch_provider_secondary_metadata,
    get_item_metadata_summaries,
    get_metadata_person_for_languages,
    get_metadata_person_locale_peer_ids,
    get_primary_item_metadata_link,
    guess_provider_movie_match,
    guess_provider_show_match,
    list_due_item_metadata_links,
    list_metadata_collection_summaries_with_preferred_languages,
    list_metadata_people_for_library,
    list_metadata_person_credit_summaries_for_person_ids,
    list_pending_item_metadata_links,
    list_provider_statuses,
    load_provider_show_descendant_targets,
    managed_metadata_asset_dir,
    metadata_asset_db_path,
    normalize_locale_key,
    persist_item_metadata_assets,
    persist_metadata_people_assets,
    provider_locale_key,
    provider_uses_localized_metadata,
    resolve_metadata_asset_db_path,
    search_metadata_people_with_preferred_languages,
    search_provider,
    set_item_metadata_refresh_state,
    sort_item_metadata_summaries_for_languages,
    try_cache_item_artwork,
    update_cached_artwork_path,
    update_metadata_person_details,
    upsert_item_metadata_link,
    upsert_item_metadata_snapshot_with_refresh_interval,
    upsert_secondary_collection_theme_song_url,
};
use crate::utils::current_timestamp;

pub enum SessionStream {
    File(RangedFile),
    Transcode {
        content_type: ContentType,
        stdout: ChildStdout,
    },
}

impl<'r> Responder<'r, 'static> for SessionStream {
    fn respond_to(
        self,
        _request: &'r Request<'_>,
    ) -> response::Result<'static> {
        match self {
            SessionStream::File(file) => file.respond_to(_request),
            SessionStream::Transcode {
                content_type,
                stdout,
            } => Response::build()
                .header(content_type)
                .streamed_body(ReaderStream::one(stdout))
                .ok(),
        }
    }
}

pub struct RangedFile {
    content_type: ContentType,
    body: Take<File>,
    content_length: u64,
    content_range: Option<String>,
}

impl<'r> Responder<'r, 'static> for RangedFile {
    fn respond_to(
        self,
        _request: &'r Request<'_>,
    ) -> response::Result<'static> {
        let mut response = Response::build();
        response
            .status(if self.content_range.is_some() { Status::PartialContent } else { Status::Ok })
            .header(self.content_type)
            .raw_header("Accept-Ranges", "bytes")
            .raw_header("Content-Length", self.content_length.to_string())
            .streamed_body(self.body);
        if let Some(content_range) = self.content_range {
            response.raw_header("Content-Range", content_range);
        }
        response.ok()
    }
}

pub struct RangeHeader(Option<String>);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RangeHeader {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> rocket::request::Outcome<Self, Self::Error> {
        Outcome::Success(RangeHeader(
            request.headers().get_one("Range").map(str::to_string),
        ))
    }
}

#[derive(Clone, Copy)]
struct ByteRange {
    start: u64,
    end: u64,
}

fn parse_byte_range(
    header: &str,
    total_length: u64,
) -> Option<ByteRange> {
    if total_length == 0 {
        return None;
    }

    let range = header
        .trim()
        .strip_prefix("bytes=")?
        .split(',')
        .next()?
        .trim();
    let (start, end) = range.split_once('-')?;
    if start.is_empty() {
        let suffix_length = end.trim().parse::<u64>().ok()?.min(total_length);
        return Some(ByteRange {
            start: total_length.saturating_sub(suffix_length),
            end: total_length - 1,
        });
    }

    let start = start.trim().parse::<u64>().ok()?;
    if start >= total_length {
        return None;
    }

    let end = if end.trim().is_empty() {
        total_length - 1
    } else {
        end.trim().parse::<u64>().ok()?.min(total_length - 1)
    };
    (end >= start).then_some(ByteRange { start, end })
}

fn content_type_for_path(path: &std::path::Path) -> ContentType {
    path.extension()
        .and_then(|extension| extension.to_str())
        .and_then(ContentType::from_extension)
        .unwrap_or(ContentType::Binary)
}

async fn open_ranged_file(
    path: PathBuf,
    range: &RangeHeader,
) -> Result<RangedFile, Status> {
    let metadata = rocket::tokio::fs::metadata(&path)
        .await
        .map_err(|_| Status::NotFound)?;
    let total_length = metadata.len();
    let selected_range = range
        .0
        .as_deref()
        .and_then(|header| parse_byte_range(header, total_length));
    let byte_range = selected_range.unwrap_or_else(|| ByteRange {
        start: 0,
        end: total_length.saturating_sub(1),
    });
    let content_length = if total_length == 0 {
        0
    } else {
        byte_range
            .end
            .saturating_sub(byte_range.start)
            .saturating_add(1)
    };
    let mut file = File::open(&path).await.map_err(|_| Status::NotFound)?;
    if byte_range.start > 0 {
        file.seek(SeekFrom::Start(byte_range.start))
            .await
            .map_err(|_| Status::InternalServerError)?;
    }

    Ok(RangedFile {
        content_type: content_type_for_path(&path),
        body: file.take(content_length),
        content_length,
        content_range: selected_range
            .map(|range| format!("bytes {}-{}/{}", range.start, range.end, total_length)),
    })
}

async fn stop_active_transcode(session_id: &str) -> bool {
    let handle = ACTIVE_TRANSCODE_TASKS.lock().await.remove(session_id);
    if let Some(handle) = handle {
        handle.abort();
        true
    } else {
        false
    }
}

async fn replace_active_transcode(
    session_id: String,
    handle: tokio::task::JoinHandle<()>,
) {
    if let Some(previous_handle) = ACTIVE_TRANSCODE_TASKS
        .lock()
        .await
        .insert(session_id, handle)
    {
        previous_handle.abort();
    }
}

/// Capability summary returned to clients during bootstrap.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ServerCapabilitiesResponse {
    /// Application name.
    pub app_name: String,
    /// Current server version.
    pub version: String,
    /// Base server URL derived from the current settings.
    pub server_url: String,
    /// Whether HTTPS is enabled.
    pub https_enabled: bool,
    /// Number of configured libraries.
    pub libraries_configured: usize,
    /// Supported API versions.
    pub api_versions: Vec<String>,
    /// Current transcoding-tool capability details.
    pub transcoding: TranscodingCapability,
}

/// Metadata response for one browser-facing media item.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ItemMetadataResponse {
    /// Stable database identifier for the item.
    pub item_id: i32,
    /// Provider statuses visible to the current server configuration.
    pub providers: Vec<MetadataProviderStatus>,
    /// Stored metadata matches for the item.
    pub matches: Vec<ItemMetadataSummary>,
}

/// Browser-facing person detail with related media credits.
#[derive(Debug, Serialize, JsonSchema)]
pub struct MetadataPersonResponse {
    /// Normalized person record.
    pub person: MetadataPersonSummary,
    /// Media items linked to this person.
    pub credits: Vec<MetadataPersonItemCredit>,
}

/// One media item credit for a person.
#[derive(Debug, Serialize, JsonSchema)]
pub struct MetadataPersonItemCredit {
    /// Stored credit details.
    pub credit: MetadataPersonCreditSummary,
    /// Related media item.
    pub item: MediaItemSummary,
    /// Breadcrumb-like media hierarchy for the credited item.
    pub hierarchy: Vec<MediaItemSummary>,
}

/// Browser-facing mixed search result.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "result_type", rename_all = "snake_case")]
pub enum MediaSearchResult {
    /// Media item result such as a movie, show, season, episode, or future item type.
    Item {
        /// Matching media item.
        item: MediaItemSummary,
    },
    /// Collection grouping result.
    Collection {
        /// Matching collection.
        collection: MetadataCollectionSummary,
    },
    /// Metadata person result.
    Person {
        /// Matching person.
        person: MetadataPersonSummary,
    },
}

/// Active backend activity summary that the browser can poll.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SystemActivity {
    /// Stable activity identifier.
    pub id: String,
    /// High-level activity category.
    pub category: String,
    /// Activity scope such as `item` or `library`.
    pub scope: String,
    /// Activity source such as `manual_item_refresh`.
    pub source: String,
    /// Current activity state such as `queued` or `running`.
    pub state: String,
    /// Human-friendly label for the activity.
    pub label: String,
    /// Provider identifier when the activity is metadata-related.
    pub provider_id: Option<String>,
    /// Owning library identifier, when known.
    pub library_id: Option<i32>,
    /// Root item identifier for item-scoped work, when known.
    pub root_item_id: Option<i32>,
    /// All item identifiers currently tracked by the activity.
    pub item_ids: Vec<i32>,
    /// Total number of tracked items.
    pub total_items: i32,
    /// Number of completed item refreshes.
    pub completed_items: i32,
    /// Number of failed item refreshes.
    pub failed_items: i32,
    /// Unix timestamp when the activity was queued.
    pub queued_at: i64,
    /// Unix timestamp when the activity first started running.
    pub started_at: Option<i64>,
    /// Unix timestamp for the latest activity update.
    pub updated_at: i64,
}

/// Pollable activity response for the browser client.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SystemActivitiesResponse {
    /// Unix timestamp when the snapshot was generated.
    pub generated_at: i64,
    /// Active activities currently tracked by the backend.
    pub activities: Vec<SystemActivity>,
}

/// Response returned after deleting missing catalog items.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct MissingItemsCleanupResponse {
    /// Library scoped by the cleanup request.
    pub library_id: i32,
    /// File rows removed from the active catalog.
    pub deleted_files: i64,
    /// Item rows removed from the active catalog.
    pub deleted_items: i64,
    /// Collection membership rows removed from active collection/list views.
    pub removed_collection_items: i64,
    /// Refreshed library summary after cleanup.
    pub library: PersistedLibrarySummary,
}

/// One locale supported by Koko metadata preferences.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct MetadataLocale {
    /// Koko locale key.
    pub key: String,
    /// Human-friendly display name.
    pub name: String,
    /// TMDB locale key.
    pub tmdb: String,
    /// TheTVDB locale key.
    pub tvdb: String,
}

/// Playback progress payload from the browser client.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlaybackProgressRequest {
    /// Current playback position in milliseconds.
    pub position_ms: i64,
    /// Current known duration in milliseconds, when available.
    pub duration_ms: Option<i64>,
    /// Whether playback has completed.
    pub completed: bool,
}

/// Request payload for linking a media item to provider metadata.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LinkMetadataRequest {
    /// Provider to link.
    pub provider_id: MetadataProviderId,
    /// Provider-side external identifier.
    pub external_id: String,
    /// Provider-specific media type such as `movie` or `tv`.
    pub media_type: String,
}

/// Request to start a playback session.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateSessionRequest {
    pub item_id: i32,
    pub client_profile: crate::media::ClientProfile,
}

static NEXT_SYSTEM_ACTIVITY_ID: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(1));
static ACTIVE_SYSTEM_ACTIVITIES: Lazy<
    tokio::sync::RwLock<HashMap<String, MetadataRefreshActivityRecord>>,
> = Lazy::new(|| tokio::sync::RwLock::new(HashMap::new()));
static ACTIVE_METADATA_REFRESH_ITEMS: Lazy<tokio::sync::RwLock<HashMap<i32, String>>> =
    Lazy::new(|| tokio::sync::RwLock::new(HashMap::new()));
static ACTIVE_METADATA_REFRESH_EXECUTIONS: Lazy<tokio::sync::RwLock<HashSet<i32>>> =
    Lazy::new(|| tokio::sync::RwLock::new(HashSet::new()));
static ACTIVE_LIBRARY_METADATA_REFRESHES: Lazy<tokio::sync::RwLock<HashSet<i32>>> =
    Lazy::new(|| tokio::sync::RwLock::new(HashSet::new()));
static ACTIVE_MANUAL_CATALOG_SCAN_RUNNING: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));
static ACTIVE_PLAYBACK_SESSIONS: Lazy<
    tokio::sync::RwLock<HashMap<String, crate::media::PlaybackSession>>,
> = Lazy::new(|| tokio::sync::RwLock::new(HashMap::new()));
static ACTIVE_TRANSCODE_TASKS: Lazy<
    tokio::sync::Mutex<HashMap<String, tokio::task::JoinHandle<()>>>,
> = Lazy::new(|| tokio::sync::Mutex::new(HashMap::new()));

#[derive(Debug, Clone)]
struct MetadataRefreshActivityRecord {
    activity: SystemActivity,
}

fn next_system_activity_id() -> String {
    format!(
        "activity-{}",
        NEXT_SYSTEM_ACTIVITY_ID.fetch_add(1, Ordering::SeqCst)
    )
}

fn begin_catalog_scan_execution() -> bool {
    ACTIVE_MANUAL_CATALOG_SCAN_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

fn finish_catalog_scan_execution() {
    ACTIVE_MANUAL_CATALOG_SCAN_RUNNING.store(false, Ordering::SeqCst);
}

fn metadata_refresh_interval_seconds(settings: &Settings) -> Option<i64> {
    settings
        .metadata
        .refresh_interval_days
        .and_then(|days| i64::from(days).checked_mul(24 * 60 * 60))
}

fn metadata_locales_for_provider(
    provider_id: MetadataProviderId,
    languages: &[String],
) -> Vec<String> {
    let source_languages = if provider_uses_localized_metadata(provider_id) {
        languages.to_vec()
    } else {
        vec![DEFAULT_METADATA_LOCALE.to_string()]
    };

    let mut seen = HashSet::new();
    let mut locales = source_languages
        .into_iter()
        .map(|language| normalize_locale_key(&language))
        .filter(|language| seen.insert(language.clone()))
        .collect::<Vec<_>>();
    if locales.is_empty() {
        locales.push(DEFAULT_METADATA_LOCALE.to_string());
    }
    locales
}

fn secondary_providers_for_library(
    settings: &Settings,
    library_providers: &[MetadataProviderId],
) -> Vec<MetadataProviderId> {
    list_provider_statuses(&settings.metadata)
        .into_iter()
        .filter(|provider| {
            provider.role == MetadataProviderRole::Secondary
                && provider.configured
                && provider.implemented
                && library_providers.contains(&provider.id)
                && provider
                    .extends_provider_ids
                    .iter()
                    .any(|primary_provider| library_providers.contains(primary_provider))
        })
        .map(|provider| provider.id)
        .collect()
}

async fn persist_secondary_metadata_for_item(
    db: &DbConn,
    item_id: i32,
    settings: &Settings,
) -> Result<(), Status> {
    let library_id = db
        .run(move |conn| {
            get_media_item_summary(conn, item_id)?
                .map(|item| item.library_id)
                .ok_or(diesel::result::Error::NotFound)
        })
        .await
        .map_err(|error| match error {
            diesel::result::Error::NotFound => Status::NotFound,
            error => {
                log::error!(
                    "Failed to load library for media item {} secondary provider metadata: {}",
                    item_id,
                    error
                );
                Status::InternalServerError
            }
        })?;
    let library_providers = load_item_library_metadata_providers(db, library_id).await?;
    let secondary_providers = secondary_providers_for_library(settings, &library_providers);
    if secondary_providers.is_empty() {
        return Ok(());
    }
    let library_languages = load_item_library_metadata_languages(db, library_id).await?;

    for provider_id in secondary_providers {
        let uses_localized_metadata = provider_uses_localized_metadata(provider_id.clone());
        let languages = metadata_locales_for_provider(provider_id.clone(), &library_languages);
        let references = db
            .run({
                let provider_id = provider_id.clone();
                move |conn| get_item_secondary_provider_references(conn, item_id, provider_id)
            })
            .await
            .map_err(|error| {
                log::error!(
                    "Failed to resolve secondary metadata references for media item {}: {}",
                    item_id,
                    error
                );
                Status::InternalServerError
            })?;

        for locale_key in &languages {
            let provider_locale = uses_localized_metadata
                .then(|| provider_locale_key(provider_id.clone(), locale_key));
            for (media_type, database_id, external_id) in &references {
                match fetch_provider_secondary_metadata(
                    provider_id.clone(),
                    media_type,
                    database_id,
                    external_id,
                    locale_key,
                )
                .await
                {
                    Ok(Some(details)) => {
                        db.run({
                            let provider_id = provider_id.clone();
                            let media_type = media_type.clone();
                            let database_id = database_id.clone();
                            let external_id = external_id.clone();
                            let locale_key = locale_key.clone();
                            let provider_locale = provider_locale.clone();
                            let details = details.clone();
                            let refresh_interval_seconds =
                                metadata_refresh_interval_seconds(settings);
                            move |conn| {
                                let snapshot = StoredMetadataSnapshot {
                                    provider_id,
                                    external_id: format!(
                                        "{media_type}:{database_id}:{external_id}"
                                    ),
                                    media_type: Some(media_type),
                                    title: None,
                                    overview: None,
                                    artwork_url: None,
                                    backdrop_url: None,
                                    release_year: None,
                                    locale_key,
                                    provider_locale_key: provider_locale,
                                    provider_payload_json: None,
                                };
                                upsert_item_metadata_link(
                                    conn,
                                    item_id,
                                    &snapshot,
                                    &details,
                                    "secondary",
                                    refresh_interval_seconds,
                                )
                            }
                        })
                        .await
                        .map_err(|error| {
                            log::error!(
                                "Failed to persist secondary metadata for media item {}: {}",
                                item_id,
                                error
                            );
                            Status::InternalServerError
                        })?;
                        break;
                    }
                    Ok(None) => {}
                    Err(error) => {
                        log::warn!(
                            "Failed to load {} secondary metadata for media item {} locale {} ({} \
                             {} {}): {}",
                            provider_id.as_storage_value(),
                            item_id,
                            locale_key,
                            media_type,
                            database_id,
                            external_id,
                            error
                        );
                    }
                }
            }
        }

        let collection_references = db
            .run({
                let provider_id = provider_id.clone();
                move |conn| get_item_youtube_theme_collection_references(conn, item_id, provider_id)
            })
            .await
            .map_err(|error| {
                log::error!(
                    "Failed to resolve secondary collection theme-song references for media item \
                     {}: {}",
                    item_id,
                    error
                );
                Status::InternalServerError
            })?;

        for (collection_id, media_type, database_id, external_id) in collection_references {
            match fetch_provider_secondary_collection_metadata(
                provider_id.clone(),
                &media_type,
                &database_id,
                &external_id,
                crate::metadata::DEFAULT_METADATA_LOCALE,
            )
            .await
            {
                Ok(Some(collection)) => {
                    let Some(url) = collection.theme_song_url else {
                        continue;
                    };
                    db.run({
                        let provider_id = provider_id.clone();
                        let media_type = media_type.clone();
                        let database_id = database_id.clone();
                        let external_id = external_id.clone();
                        move |conn| {
                            upsert_secondary_collection_theme_song_url(
                                conn,
                                collection_id,
                                provider_id,
                                &media_type,
                                &database_id,
                                &external_id,
                                &url,
                            )
                        }
                    })
                    .await
                    .map_err(|error| {
                        log::error!(
                            "Failed to persist secondary collection theme-song metadata for media \
                             item {}: {}",
                            item_id,
                            error
                        );
                        Status::InternalServerError
                    })?;
                    break;
                }
                Ok(None) => {}
                Err(error) => {
                    log::warn!(
                        "Failed to load {} secondary collection metadata for media item {} ({} {} \
                         {}): {}",
                        provider_id.as_storage_value(),
                        item_id,
                        media_type,
                        database_id,
                        external_id,
                        error
                    );
                }
            }
        }
    }

    Ok(())
}

async fn persist_snapshot_for_item(
    db: &DbConn,
    item_id: i32,
    snapshot: &StoredMetadataSnapshot,
    settings: &Settings,
    options: PersistSnapshotOptions,
) -> Result<ItemMetadataSummary, Status> {
    let snapshot = if options.cache_person_assets {
        persist_metadata_people_assets(snapshot, &settings.general.data_dir)
            .await
            .map_err(|error| {
                log::error!(
                    "Failed to persist metadata people assets for media item {}: {}",
                    item_id,
                    error
                );
                Status::BadGateway
            })?
    } else {
        snapshot.clone()
    };
    let (poster_path, backdrop_path, logo_path) =
        persist_item_metadata_assets(&snapshot, item_id, &settings.general.data_dir)
            .await
            .map_err(|error| {
                log::error!(
                    "Failed to persist metadata assets for media item {}: {}",
                    item_id,
                    error
                );
                Status::BadGateway
            })?;

    let mut summary = db
        .run({
            let snapshot = snapshot.clone();
            let refresh_interval_seconds = metadata_refresh_interval_seconds(settings);
            move |conn| {
                upsert_item_metadata_snapshot_with_refresh_interval(
                    conn,
                    item_id,
                    &snapshot,
                    refresh_interval_seconds,
                )
            }
        })
        .await
        .map_err(|error| {
            log::error!(
                "Failed to persist linked metadata for media item {}: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?;

    if let Some(poster_path) = poster_path {
        let summary_id = summary.id;
        let poster_path_string = metadata_asset_db_path(&settings.general.data_dir, &poster_path);
        let data_dir = settings.general.data_dir.clone();
        db.run(move |conn| {
            update_cached_artwork_path(
                conn,
                summary_id,
                ArtworkKind::Poster,
                &poster_path,
                &data_dir,
            )
        })
        .await
        .map_err(|error| {
            log::error!(
                "Failed to store poster cache path for media item {}: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?;
        summary.cached_artwork_path = Some(poster_path_string);
    }

    if let Some(backdrop_path) = backdrop_path {
        let summary_id = summary.id;
        let backdrop_path_string =
            metadata_asset_db_path(&settings.general.data_dir, &backdrop_path);
        let data_dir = settings.general.data_dir.clone();
        db.run(move |conn| {
            update_cached_artwork_path(
                conn,
                summary_id,
                ArtworkKind::Backdrop,
                &backdrop_path,
                &data_dir,
            )
        })
        .await
        .map_err(|error| {
            log::error!(
                "Failed to store backdrop cache path for media item {}: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?;
        summary.cached_backdrop_path = Some(backdrop_path_string);
    }

    if let Some(logo_path) = logo_path {
        let summary_id = summary.id;
        let logo_path_string = metadata_asset_db_path(&settings.general.data_dir, &logo_path);
        let data_dir = settings.general.data_dir.clone();
        db.run(move |conn| {
            update_cached_artwork_path(conn, summary_id, ArtworkKind::Logo, &logo_path, &data_dir)
        })
        .await
        .map_err(|error| {
            log::error!(
                "Failed to store logo cache path for media item {}: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?;
        summary.cached_logo_path = Some(logo_path_string);
    }

    persist_secondary_metadata_for_item(db, item_id, settings).await?;

    Ok(summary)
}

fn supports_manual_metadata_linking(item: &MediaItemSummary) -> bool {
    matches!(item.item_type.as_str(), "movie" | "show")
}

fn provider_search_media_type(
    provider_id: MetadataProviderId,
    item: &MediaItemSummary,
) -> Option<&'static str> {
    match (provider_id, item.item_type.as_str()) {
        (_, "movie") => Some("movie"),
        (MetadataProviderId::Tmdb, "show") => Some("tv"),
        (MetadataProviderId::Tvdb, "show") => Some("series"),
        _ => None,
    }
}

fn parse_metadata_provider_selection(value: Option<String>) -> Vec<MetadataProviderId> {
    value
        .unwrap_or_default()
        .split(',')
        .filter_map(|provider| MetadataProviderId::from_storage_value(provider.trim()))
        .collect()
}

fn metadata_search_score(
    query: &str,
    year: Option<i32>,
    result: &MetadataSearchResult,
) -> f64 {
    let query_title = query.trim();
    let result_title = result.title.trim();
    let title_score = normalized_levenshtein(
        &query_title.to_ascii_lowercase(),
        &result_title.to_ascii_lowercase(),
    );
    let year_score = match (year, result.release_year) {
        (Some(left), Some(right)) if left == right => 0.16,
        (Some(left), Some(right)) => {
            let distance = (left - right).abs() as f64;
            -(0.06 + distance.min(12.0) * 0.035)
        }
        (Some(_), None) => -0.05,
        _ => 0.0,
    };
    let casing_score = if query_title == result_title {
        0.03
    } else {
        metadata_search_casing_penalty(query_title, result_title, &result.provider_id)
    };
    ((title_score + year_score).clamp(0.0, 1.0) + casing_score).clamp(0.0, 1.0)
}

fn metadata_search_casing_penalty(
    query_title: &str,
    result_title: &str,
    provider_id: &MetadataProviderId,
) -> f64 {
    if !matches!(
        provider_id,
        MetadataProviderId::Tmdb | MetadataProviderId::Tvdb
    ) {
        return 0.0;
    }

    let comparable_letters = query_title
        .chars()
        .zip(result_title.chars())
        .filter(|(left, right)| {
            left.is_ascii_alphabetic()
                && right.is_ascii_alphabetic()
                && left.eq_ignore_ascii_case(right)
        })
        .count();
    if comparable_letters == 0 {
        return 0.0;
    }

    let mismatched_case_letters = query_title
        .chars()
        .zip(result_title.chars())
        .filter(|(left, right)| {
            left.is_ascii_alphabetic()
                && right.is_ascii_alphabetic()
                && left.eq_ignore_ascii_case(right)
                && left != right
        })
        .count();
    if mismatched_case_letters == 0 {
        return 0.0;
    }

    let mismatch_ratio = mismatched_case_letters as f64 / comparable_letters as f64;
    -(0.04 + mismatch_ratio * 0.06)
}

#[derive(Debug, Clone)]
enum MetadataRefreshFetchKind {
    Direct,
    TmdbShowSeason {
        show_external_id: String,
        season_number: i32,
    },
    TmdbShowEpisode {
        show_external_id: String,
        season_number: i32,
        episode_number: i32,
    },
    TvdbSeason {
        show_external_id: String,
        season_number: i32,
        season_external_id: String,
    },
    TvdbEpisode {
        show_external_id: String,
        season_number: i32,
        episode_number: i32,
        episode_external_id: String,
    },
}

impl MetadataRefreshFetchKind {
    fn snapshot_fetch_options(&self) -> MetadataSnapshotFetchOptions {
        match self {
            Self::Direct => MetadataSnapshotFetchOptions::WITHOUT_PERSON_DETAILS,
            Self::TmdbShowSeason { .. }
            | Self::TmdbShowEpisode { .. }
            | Self::TvdbSeason { .. }
            | Self::TvdbEpisode { .. } => MetadataSnapshotFetchOptions::WITHOUT_PERSON_DETAILS,
        }
    }
}

#[derive(Debug, Clone)]
struct MetadataRefreshTarget {
    item_id: i32,
    library_id: i32,
    provider_id: MetadataProviderId,
    item_type: String,
    display_title: String,
    relative_path: String,
    external_id: String,
    media_type: String,
    fetch_kind: MetadataRefreshFetchKind,
}

#[derive(Debug, Clone)]
struct MetadataRefreshJob {
    root: MetadataRefreshTarget,
    descendants: Vec<MetadataRefreshTarget>,
}

#[derive(Debug, Clone, Copy)]
struct PersistSnapshotOptions {
    cache_person_assets: bool,
}

impl PersistSnapshotOptions {
    const WITHOUT_PERSON_ASSETS: Self = Self {
        cache_person_assets: false,
    };

    fn for_target(_target: &MetadataRefreshTarget) -> Self {
        Self::WITHOUT_PERSON_ASSETS
    }
}

fn describe_metadata_refresh_target(target: &MetadataRefreshTarget) -> String {
    format!(
        "media item {} \"{}\" ({}) in library {} [{}]",
        target.item_id,
        target.display_title,
        target.item_type,
        target.library_id,
        target.relative_path
    )
}

fn flatten_metadata_refresh_job(job: &MetadataRefreshJob) -> Vec<MetadataRefreshTarget> {
    let mut targets = Vec::with_capacity(1 + job.descendants.len());
    targets.push(job.root.clone());
    targets.extend(job.descendants.clone());
    targets
}

async fn begin_metadata_refresh_execution(item_id: i32) -> bool {
    ACTIVE_METADATA_REFRESH_EXECUTIONS
        .write()
        .await
        .insert(item_id)
}

async fn finish_metadata_refresh_execution(item_id: i32) {
    ACTIVE_METADATA_REFRESH_EXECUTIONS
        .write()
        .await
        .remove(&item_id);
}

async fn begin_library_metadata_refresh(library_id: i32) -> bool {
    ACTIVE_LIBRARY_METADATA_REFRESHES
        .write()
        .await
        .insert(library_id)
}

async fn finish_library_metadata_refresh(library_id: i32) {
    ACTIVE_LIBRARY_METADATA_REFRESHES
        .write()
        .await
        .remove(&library_id);
}

async fn register_library_scan_activity(
    library_id: i32,
    library_name: &str,
) -> String {
    let activity_id = next_system_activity_id();
    let now = current_timestamp();
    ACTIVE_SYSTEM_ACTIVITIES.write().await.insert(
        activity_id.clone(),
        MetadataRefreshActivityRecord {
            activity: SystemActivity {
                id: activity_id.clone(),
                category: "library_scan".into(),
                scope: "library".into(),
                source: "manual_library_scan".into(),
                state: "queued".into(),
                label: format!("Scan library catalog for {library_name}"),
                provider_id: None,
                library_id: Some(library_id),
                root_item_id: None,
                item_ids: Vec::new(),
                total_items: 1,
                completed_items: 0,
                failed_items: 0,
                queued_at: now,
                started_at: None,
                updated_at: now,
            },
        },
    );
    activity_id
}

async fn mark_library_scan_activity_running(activity_id: &str) {
    if let Some(record) = ACTIVE_SYSTEM_ACTIVITIES.write().await.get_mut(activity_id) {
        record.activity.state = "running".into();
        record
            .activity
            .started_at
            .get_or_insert_with(current_timestamp);
        record.activity.updated_at = current_timestamp();
    }
}

async fn complete_library_scan_activity(
    activity_id: &str,
    failed: bool,
) {
    if let Some(record) = ACTIVE_SYSTEM_ACTIVITIES.write().await.get_mut(activity_id) {
        record.activity.state = if failed { "failed" } else { "completed" }.into();
        record.activity.completed_items = 1;
        record.activity.failed_items = if failed { 1 } else { 0 };
        record.activity.updated_at = current_timestamp();
    }
    ACTIVE_SYSTEM_ACTIVITIES.write().await.remove(activity_id);
}

async fn register_metadata_refresh_activity(
    scope: &str,
    source: &str,
    label: String,
    library_id: Option<i32>,
    root_item_id: Option<i32>,
    targets: Vec<MetadataRefreshTarget>,
) -> Option<(String, Vec<MetadataRefreshTarget>)> {
    let mut item_registry = ACTIVE_METADATA_REFRESH_ITEMS.write().await;
    let mut seen_item_ids = HashSet::new();
    let queued_targets = targets
        .into_iter()
        .filter(|target| seen_item_ids.insert(target.item_id))
        .filter(|target| !item_registry.contains_key(&target.item_id))
        .collect::<Vec<_>>();
    if queued_targets.is_empty() {
        return None;
    }

    let activity_id = next_system_activity_id();
    for target in &queued_targets {
        item_registry.insert(target.item_id, activity_id.clone());
    }
    drop(item_registry);

    let now = current_timestamp();
    ACTIVE_SYSTEM_ACTIVITIES.write().await.insert(
        activity_id.clone(),
        MetadataRefreshActivityRecord {
            activity: SystemActivity {
                id: activity_id.clone(),
                category: "metadata_refresh".into(),
                scope: scope.into(),
                source: source.into(),
                state: "queued".into(),
                label,
                provider_id: queued_targets
                    .first()
                    .map(|target| target.provider_id.as_storage_value().to_string()),
                library_id,
                root_item_id,
                item_ids: queued_targets.iter().map(|target| target.item_id).collect(),
                total_items: i32::try_from(queued_targets.len()).unwrap_or(i32::MAX),
                completed_items: 0,
                failed_items: 0,
                queued_at: now,
                started_at: None,
                updated_at: now,
            },
        },
    );

    Some((activity_id, queued_targets))
}

async fn extend_metadata_refresh_activity(
    activity_id: &str,
    targets: Vec<MetadataRefreshTarget>,
) -> Vec<MetadataRefreshTarget> {
    let mut item_registry = ACTIVE_METADATA_REFRESH_ITEMS.write().await;
    let mut seen_item_ids = HashSet::new();
    let queued_targets = targets
        .into_iter()
        .filter(|target| seen_item_ids.insert(target.item_id))
        .filter(|target| !item_registry.contains_key(&target.item_id))
        .collect::<Vec<_>>();
    if queued_targets.is_empty() {
        return Vec::new();
    }

    for target in &queued_targets {
        item_registry.insert(target.item_id, activity_id.to_string());
    }
    drop(item_registry);

    if let Some(record) = ACTIVE_SYSTEM_ACTIVITIES.write().await.get_mut(activity_id) {
        record
            .activity
            .item_ids
            .extend(queued_targets.iter().map(|target| target.item_id));
        record.activity.total_items =
            i32::try_from(record.activity.item_ids.len()).unwrap_or(i32::MAX);
        record.activity.updated_at = current_timestamp();
    }

    queued_targets
}

async fn register_metadata_refresh_activity_for_items(
    scope: &str,
    source: &str,
    label: String,
    library_id: Option<i32>,
    root_item_id: Option<i32>,
    provider_id: Option<MetadataProviderId>,
    item_ids: Vec<i32>,
) -> Option<(String, Vec<i32>)> {
    let mut item_registry = ACTIVE_METADATA_REFRESH_ITEMS.write().await;
    let mut seen_item_ids = HashSet::new();
    let queued_item_ids = item_ids
        .into_iter()
        .filter(|item_id| seen_item_ids.insert(*item_id))
        .filter(|item_id| !item_registry.contains_key(item_id))
        .collect::<Vec<_>>();
    if queued_item_ids.is_empty() {
        return None;
    }

    let activity_id = next_system_activity_id();
    for item_id in &queued_item_ids {
        item_registry.insert(*item_id, activity_id.clone());
    }
    drop(item_registry);

    let now = current_timestamp();
    ACTIVE_SYSTEM_ACTIVITIES.write().await.insert(
        activity_id.clone(),
        MetadataRefreshActivityRecord {
            activity: SystemActivity {
                id: activity_id.clone(),
                category: "metadata_refresh".into(),
                scope: scope.into(),
                source: source.into(),
                state: "queued".into(),
                label,
                provider_id: provider_id
                    .as_ref()
                    .map(|provider_id| provider_id.as_storage_value().to_string()),
                library_id,
                root_item_id,
                item_ids: queued_item_ids.clone(),
                total_items: i32::try_from(queued_item_ids.len()).unwrap_or(i32::MAX),
                completed_items: 0,
                failed_items: 0,
                queued_at: now,
                started_at: None,
                updated_at: now,
            },
        },
    );

    Some((activity_id, queued_item_ids))
}

async fn register_manual_library_automatch_activity(
    db: &DbConn,
    library_id: i32,
) -> Option<(String, Vec<i32>)> {
    let candidates = match db
        .run(move |conn| {
            list_automatic_metadata_refresh_candidates(conn, Some(library_id), usize::MAX)
        })
        .await
    {
        Ok(candidates) => candidates,
        Err(error) => {
            log::warn!(
                "Failed to load automatic metadata candidates for library {} refresh activity: {}",
                library_id,
                error
            );
            return None;
        }
    };
    let item_ids = candidates
        .iter()
        .map(|candidate| candidate.item_id)
        .collect::<Vec<_>>();

    register_metadata_refresh_activity_for_items(
        "library",
        "manual_library_automatch",
        "Match unlinked library metadata".into(),
        Some(library_id),
        None,
        None,
        item_ids,
    )
    .await
}

async fn cancel_metadata_refresh_activity(activity_id: &str) {
    let removed = ACTIVE_SYSTEM_ACTIVITIES.write().await.remove(activity_id);
    if let Some(record) = removed {
        let mut item_registry = ACTIVE_METADATA_REFRESH_ITEMS.write().await;
        for item_id in &record.activity.item_ids {
            if item_registry.get(item_id).map(|value| value.as_str()) == Some(activity_id) {
                item_registry.remove(item_id);
            }
        }
    }
}

async fn mark_metadata_refresh_activity_running(activity_id: &str) {
    if let Some(record) = ACTIVE_SYSTEM_ACTIVITIES.write().await.get_mut(activity_id) {
        record.activity.state = "running".into();
        record
            .activity
            .started_at
            .get_or_insert_with(current_timestamp);
        record.activity.updated_at = current_timestamp();
    }
}

async fn record_metadata_refresh_activity_progress(
    activity_id: &str,
    failed: bool,
) {
    if let Some(record) = ACTIVE_SYSTEM_ACTIVITIES.write().await.get_mut(activity_id) {
        record.activity.completed_items += 1;
        if failed {
            record.activity.failed_items += 1;
        }
        record.activity.updated_at = current_timestamp();
    }
}

async fn complete_metadata_refresh_activity(activity_id: &str) {
    cancel_metadata_refresh_activity(activity_id).await;
}

async fn current_system_activities() -> Vec<SystemActivity> {
    let activities = ACTIVE_SYSTEM_ACTIVITIES.read().await;
    let mut snapshot = activities
        .values()
        .map(|record| record.activity.clone())
        .collect::<Vec<_>>();
    snapshot.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.label.cmp(&right.label))
    });
    snapshot
}

async fn load_show_descendant_refresh_targets(
    db: &DbConn,
    settings: &crate::config::Settings,
    show_item_id: i32,
    provider_id: MetadataProviderId,
    show_external_id: &str,
) -> Result<Vec<MetadataRefreshTarget>, Status> {
    let lookup = load_provider_show_descendant_targets(
        &settings.metadata,
        provider_id.clone(),
        show_external_id,
    )
    .await
    .map_err(|error| {
        log::error!(
            "Failed to load {} descendant metadata for show item {}: {}",
            provider_id.as_storage_value(),
            show_item_id,
            error
        );
        Status::ServiceUnavailable
    })?;
    if lookup.is_empty() {
        return Ok(Vec::new());
    }

    let descendant_plan = show_metadata_descendant_plan(&lookup);
    let descendant_items = db
        .run(move |conn| {
            upsert_show_metadata_descendant_items(conn, show_item_id, &descendant_plan)
        })
        .await
        .map_err(|error| {
            log::error!(
                "Failed to upsert missing show descendants for metadata propagation on item {}: {}",
                show_item_id,
                error
            );
            Status::InternalServerError
        })?;

    let mut targets = Vec::new();
    for (season_number, season_target) in provider_season_targets_by_number(&lookup) {
        let Some(season) = descendant_items
            .seasons_by_number
            .get(&season_number)
            .cloned()
        else {
            continue;
        };
        if let Some(target) = metadata_refresh_target_for_show_season(
            provider_id.clone(),
            show_external_id,
            season,
            &season_target,
        ) {
            targets.push(target);
        }

        if !descendant_items
            .seasons_with_local_episodes
            .contains(&season_number)
        {
            continue;
        }

        let mut episode_targets = lookup
            .iter()
            .filter(|target| target.season_number == season_number && target.episode_number > 0)
            .cloned()
            .collect::<Vec<_>>();
        episode_targets.sort_by_key(|target| target.episode_number);
        episode_targets.dedup_by_key(|target| target.episode_number);
        for episode_target in episode_targets {
            let Some(episode) = descendant_items
                .episodes_by_number
                .get(&(season_number, episode_target.episode_number))
                .cloned()
            else {
                continue;
            };
            if let Some(target) = metadata_refresh_target_for_show_episode(
                provider_id.clone(),
                show_external_id,
                episode,
                &episode_target,
            ) {
                targets.push(target);
            }
        }
    }

    Ok(targets)
}

fn show_metadata_descendant_plan(
    lookup: &[ProviderDescendantTarget]
) -> ShowMetadataDescendantPlan {
    let mut season_numbers = HashSet::new();
    let mut episode_numbers = HashSet::new();
    for target in lookup {
        if target.season_number > 0 {
            season_numbers.insert(target.season_number);
        }
        if target.season_number > 0 && target.episode_number > 0 {
            episode_numbers.insert((target.season_number, target.episode_number));
        }
    }

    let mut seasons = season_numbers
        .into_iter()
        .map(|season_number| ShowMetadataSeasonPlan {
            season_number,
            display_title: None,
        })
        .collect::<Vec<_>>();
    seasons.sort_by_key(|season| season.season_number);

    let mut episodes = episode_numbers
        .into_iter()
        .map(|(season_number, episode_number)| ShowMetadataEpisodePlan {
            season_number,
            episode_number,
            display_title: None,
        })
        .collect::<Vec<_>>();
    episodes.sort_by_key(|episode| (episode.season_number, episode.episode_number));

    ShowMetadataDescendantPlan { seasons, episodes }
}

fn provider_season_targets_by_number(
    lookup: &[ProviderDescendantTarget]
) -> Vec<(i32, ProviderDescendantTarget)> {
    let mut by_number = HashMap::new();
    for target in lookup.iter().filter(|target| target.season_number > 0) {
        by_number
            .entry(target.season_number)
            .or_insert_with(|| target.clone());
    }

    let mut seasons = by_number.into_iter().collect::<Vec<_>>();
    seasons.sort_by_key(|(season_number, _)| *season_number);
    seasons
}

fn metadata_refresh_target_for_show_season(
    provider_id: MetadataProviderId,
    show_external_id: &str,
    season: MediaItemSummary,
    provider_target: &ProviderDescendantTarget,
) -> Option<MetadataRefreshTarget> {
    let season_number = provider_target.season_number;
    match provider_id {
        MetadataProviderId::Tmdb => Some(MetadataRefreshTarget {
            item_id: season.id,
            library_id: season.library_id,
            provider_id,
            item_type: season.item_type,
            display_title: season.display_title,
            relative_path: season.relative_path,
            external_id: format!("tv:{show_external_id}:season:{season_number}"),
            media_type: "tv_season".into(),
            fetch_kind: MetadataRefreshFetchKind::TmdbShowSeason {
                show_external_id: show_external_id.to_string(),
                season_number,
            },
        }),
        MetadataProviderId::Tvdb => Some(MetadataRefreshTarget {
            item_id: season.id,
            library_id: season.library_id,
            provider_id,
            item_type: season.item_type,
            display_title: season.display_title,
            relative_path: season.relative_path,
            external_id: format!(
                "series:{show_external_id}:season:{}",
                provider_target.season_external_id
            ),
            media_type: "season".into(),
            fetch_kind: MetadataRefreshFetchKind::TvdbSeason {
                show_external_id: show_external_id.to_string(),
                season_number,
                season_external_id: provider_target.season_external_id.clone(),
            },
        }),
        _ => None,
    }
}

fn metadata_refresh_target_for_show_episode(
    provider_id: MetadataProviderId,
    show_external_id: &str,
    episode: MediaItemSummary,
    provider_target: &ProviderDescendantTarget,
) -> Option<MetadataRefreshTarget> {
    let season_number = provider_target.season_number;
    let episode_number = provider_target.episode_number;
    match provider_id {
        MetadataProviderId::Tmdb => Some(MetadataRefreshTarget {
            item_id: episode.id,
            library_id: episode.library_id,
            provider_id,
            item_type: episode.item_type,
            display_title: episode.display_title,
            relative_path: episode.relative_path,
            external_id: format!(
                "tv:{show_external_id}:season:{season_number}:episode:{episode_number}"
            ),
            media_type: "tv_episode".into(),
            fetch_kind: MetadataRefreshFetchKind::TmdbShowEpisode {
                show_external_id: show_external_id.to_string(),
                season_number,
                episode_number,
            },
        }),
        MetadataProviderId::Tvdb => Some(MetadataRefreshTarget {
            item_id: episode.id,
            library_id: episode.library_id,
            provider_id,
            item_type: episode.item_type,
            display_title: episode.display_title,
            relative_path: episode.relative_path,
            external_id: format!(
                "series:{show_external_id}:season:{season_number}:episode:{}",
                provider_target.episode_external_id
            ),
            media_type: "episode".into(),
            fetch_kind: MetadataRefreshFetchKind::TvdbEpisode {
                show_external_id: show_external_id.to_string(),
                season_number,
                episode_number,
                episode_external_id: provider_target.episode_external_id.clone(),
            },
        }),
        _ => None,
    }
}

async fn mark_metadata_refresh_target_pending(
    db: &DbConn,
    target: &MetadataRefreshTarget,
) -> Result<ItemMetadataSummary, Status> {
    db.run({
        let target = target.clone();
        move |conn| {
            set_item_metadata_refresh_state(
                conn,
                target.item_id,
                target.provider_id,
                &target.external_id,
                Some(&target.media_type),
                "pending",
                None,
            )
        }
    })
    .await
    .map_err(|error| {
        log::error!(
            "Failed to mark media item {} metadata refresh pending: {}",
            target.item_id,
            error
        );
        Status::InternalServerError
    })
}

async fn mark_metadata_refresh_targets_pending(
    db: &DbConn,
    targets: &[MetadataRefreshTarget],
) -> Result<(), Status> {
    for target in targets {
        mark_metadata_refresh_target_pending(db, target).await?;
    }

    Ok(())
}

async fn record_metadata_refresh_error(
    db: &DbConn,
    target: &MetadataRefreshTarget,
    message: &str,
) {
    if let Err(error) = db
        .run({
            let target = target.clone();
            let message = message.to_string();
            move |conn| {
                set_item_metadata_refresh_state(
                    conn,
                    target.item_id,
                    target.provider_id,
                    &target.external_id,
                    Some(&target.media_type),
                    "error",
                    Some(&message),
                )
            }
        })
        .await
    {
        log::warn!(
            "Failed to record metadata refresh error for media item {}: {}",
            target.item_id,
            error
        );
    }
}

async fn fetch_metadata_refresh_snapshots_for_language(
    settings: &crate::config::Settings,
    target: &MetadataRefreshTarget,
    language: &str,
) -> Result<StoredMetadataSnapshot, String> {
    let fetch_options = target.fetch_kind.snapshot_fetch_options();
    match &target.fetch_kind {
        MetadataRefreshFetchKind::Direct => {
            fetch_provider_metadata_snapshot_for_locale_with_options(
                &settings.metadata,
                target.provider_id.clone(),
                &target.external_id,
                &target.media_type,
                language,
                fetch_options,
            )
            .await
        }
        MetadataRefreshFetchKind::TmdbShowSeason {
            show_external_id,
            season_number,
        } => {
            fetch_provider_season_metadata_snapshot_for_locale_with_options(
                &settings.metadata,
                target.provider_id.clone(),
                show_external_id,
                *season_number,
                None,
                language,
                fetch_options,
            )
            .await
        }
        MetadataRefreshFetchKind::TmdbShowEpisode {
            show_external_id,
            season_number,
            episode_number,
        } => {
            fetch_provider_episode_metadata_snapshot_for_locale_with_options(
                &settings.metadata,
                target.provider_id.clone(),
                ProviderEpisodeMetadataSnapshotFetch {
                    show_external_id,
                    season_number: *season_number,
                    episode_number: *episode_number,
                    episode_external_id: None,
                    locale_key: language,
                    options: fetch_options,
                },
            )
            .await
        }
        MetadataRefreshFetchKind::TvdbSeason {
            show_external_id,
            season_number,
            season_external_id,
        } => {
            fetch_provider_season_metadata_snapshot_for_locale_with_options(
                &settings.metadata,
                target.provider_id.clone(),
                show_external_id,
                *season_number,
                Some(season_external_id),
                language,
                fetch_options,
            )
            .await
        }
        MetadataRefreshFetchKind::TvdbEpisode {
            show_external_id,
            season_number,
            episode_number,
            episode_external_id,
        } => {
            fetch_provider_episode_metadata_snapshot_for_locale_with_options(
                &settings.metadata,
                target.provider_id.clone(),
                ProviderEpisodeMetadataSnapshotFetch {
                    show_external_id,
                    season_number: *season_number,
                    episode_number: *episode_number,
                    episode_external_id: Some(episode_external_id),
                    locale_key: language,
                    options: fetch_options,
                },
            )
            .await
        }
    }
}

async fn fetch_metadata_refresh_snapshots(
    db: &DbConn,
    settings: &crate::config::Settings,
    target: &MetadataRefreshTarget,
) -> Result<Vec<StoredMetadataSnapshot>, String> {
    let languages = load_refresh_target_metadata_languages(db, target.item_id).await?;
    let languages = metadata_locales_for_provider(target.provider_id.clone(), &languages);
    let mut snapshots = Vec::new();
    for language in languages {
        snapshots.push(
            fetch_metadata_refresh_snapshots_for_language(settings, target, &language).await?,
        );
    }
    Ok(snapshots)
}

async fn execute_metadata_refresh_target(
    db: &DbConn,
    target: &MetadataRefreshTarget,
    settings: &crate::config::Settings,
) -> bool {
    if !begin_metadata_refresh_execution(target.item_id).await {
        log::info!(
            "Skipping duplicate {} metadata refresh for {}; another refresh for this item is \
             already running",
            target.provider_id.as_storage_value(),
            describe_metadata_refresh_target(target)
        );
        return false;
    }

    let failed = execute_metadata_refresh_target_inner(db, target, settings).await;
    finish_metadata_refresh_execution(target.item_id).await;
    failed
}

async fn execute_metadata_refresh_target_inner(
    db: &DbConn,
    target: &MetadataRefreshTarget,
    settings: &crate::config::Settings,
) -> bool {
    log::info!(
        "Starting {} metadata refresh for {} using target {} ({})",
        target.provider_id.as_storage_value(),
        describe_metadata_refresh_target(target),
        target.external_id,
        target.media_type
    );
    let snapshot_result = fetch_metadata_refresh_snapshots(db, settings, target).await;

    match snapshot_result {
        Ok(snapshots) => {
            for snapshot in snapshots {
                if let Err(status) = persist_snapshot_for_item(
                    db,
                    target.item_id,
                    &snapshot,
                    settings,
                    PersistSnapshotOptions::for_target(target),
                )
                .await
                {
                    let status_message = format!("{status:?}");
                    log::warn!(
                        "Failed to persist refreshed {} metadata snapshot for {}: {}",
                        target.provider_id.as_storage_value(),
                        describe_metadata_refresh_target(target),
                        status_message
                    );
                    record_metadata_refresh_error(db, target, &status_message).await;
                    return true;
                }
            }

            log::info!(
                "Completed {} metadata refresh for {} using target {} ({})",
                target.provider_id.as_storage_value(),
                describe_metadata_refresh_target(target),
                target.external_id,
                target.media_type
            );
            false
        }
        Err(error) => {
            log::warn!(
                "Failed to fetch refreshed {} metadata snapshot for {} using target {} ({}): {}",
                target.provider_id.as_storage_value(),
                describe_metadata_refresh_target(target),
                target.external_id,
                target.media_type,
                error
            );
            record_metadata_refresh_error(db, target, &error).await;
            true
        }
    }
}

async fn execute_metadata_refresh_targets(
    db: &DbConn,
    targets: &[MetadataRefreshTarget],
    settings: &crate::config::Settings,
) {
    for target in targets {
        execute_metadata_refresh_target(db, target, settings).await;
    }
}

fn parse_tmdb_child_external_id(external_id: &str) -> Option<(&str, i32, Option<i32>)> {
    let parts = external_id.split(':').collect::<Vec<_>>();
    match parts.as_slice() {
        [
            "tv",
            show_external_id,
            "season",
            season_number,
        ] => Some((*show_external_id, season_number.parse().ok()?, None)),
        [
            "tv",
            show_external_id,
            "season",
            season_number,
            "episode",
            episode_number,
        ] => Some((
            *show_external_id,
            season_number.parse().ok()?,
            Some(episode_number.parse().ok()?),
        )),
        _ => None,
    }
}

fn parse_tvdb_child_external_id(external_id: &str) -> Option<(&str, Option<i32>, &str)> {
    let parts = external_id.split(':').collect::<Vec<_>>();
    match parts.as_slice() {
        [
            "series",
            show_external_id,
            "season",
            season_external_id,
        ] => Some((*show_external_id, None, *season_external_id)),
        [
            "series",
            show_external_id,
            "season",
            season_number,
            "episode",
            episode_external_id,
        ] => Some((
            *show_external_id,
            season_number.parse().ok(),
            *episode_external_id,
        )),
        _ => None,
    }
}

fn pending_metadata_refresh_target(
    item: MediaItemSummary,
    link: ItemMetadataLink,
) -> Option<MetadataRefreshTarget> {
    let provider_id = MetadataProviderId::from_storage_value(&link.provider_id)?;
    let media_type = link.media_type.clone()?;
    let fetch_kind = match provider_id {
        MetadataProviderId::Tmdb => match media_type.as_str() {
            "movie" | "tv" => MetadataRefreshFetchKind::Direct,
            "tv_season" => {
                let (show_external_id, season_number, _) =
                    parse_tmdb_child_external_id(&link.external_id)?;
                MetadataRefreshFetchKind::TmdbShowSeason {
                    show_external_id: show_external_id.to_string(),
                    season_number,
                }
            }
            "tv_episode" => {
                let (show_external_id, season_number, episode_number) =
                    parse_tmdb_child_external_id(&link.external_id)?;
                MetadataRefreshFetchKind::TmdbShowEpisode {
                    show_external_id: show_external_id.to_string(),
                    season_number,
                    episode_number: episode_number?,
                }
            }
            _ => return None,
        },
        MetadataProviderId::Tvdb => match media_type.as_str() {
            "movie" | "series" => MetadataRefreshFetchKind::Direct,
            "season" => {
                let (show_external_id, _, season_external_id) =
                    parse_tvdb_child_external_id(&link.external_id)?;
                MetadataRefreshFetchKind::TvdbSeason {
                    show_external_id: show_external_id.to_string(),
                    season_number: item.season_number.unwrap_or_default(),
                    season_external_id: season_external_id.to_string(),
                }
            }
            "episode" => {
                let (show_external_id, season_number, episode_external_id) =
                    parse_tvdb_child_external_id(&link.external_id)?;
                MetadataRefreshFetchKind::TvdbEpisode {
                    show_external_id: show_external_id.to_string(),
                    season_number: season_number.or(item.season_number).unwrap_or_default(),
                    episode_number: item.episode_number.unwrap_or_default(),
                    episode_external_id: episode_external_id.to_string(),
                }
            }
            _ => return None,
        },
        _ => return None,
    };

    Some(MetadataRefreshTarget {
        item_id: item.id,
        library_id: item.library_id,
        provider_id,
        item_type: item.item_type,
        display_title: item.display_title,
        relative_path: item.relative_path,
        external_id: link.external_id,
        media_type,
        fetch_kind,
    })
}

pub(crate) async fn recover_pending_metadata_refreshes(
    db: &DbConn,
    settings: &crate::config::Settings,
) {
    let links = match db.run(list_pending_item_metadata_links).await {
        Ok(links) => links,
        Err(error) => {
            log::warn!("Failed to load pending metadata refresh links: {}", error);
            return;
        }
    };
    if links.is_empty() {
        return;
    }

    execute_metadata_refresh_links(
        db,
        settings,
        links,
        "automatic_pending_recovery",
        "Recover pending metadata refreshes",
    )
    .await;
}

async fn execute_metadata_refresh_links(
    db: &DbConn,
    settings: &crate::config::Settings,
    links: Vec<ItemMetadataLink>,
    source: &str,
    label: &str,
) {
    let mut targets = Vec::new();
    for link in links {
        let item_id = link.media_item_id;
        let item = match db
            .run(move |conn| get_media_item_summary(conn, item_id))
            .await
        {
            Ok(Some(item)) => item,
            Ok(None) => continue,
            Err(error) => {
                log::warn!(
                    "Failed to load pending metadata item {}: {}",
                    item_id,
                    error
                );
                continue;
            }
        };

        if let Some(target) = pending_metadata_refresh_target(item, link) {
            targets.push(target);
        }
    }

    let Some((activity_id, queued_targets)) =
        register_metadata_refresh_activity("metadata", source, label.into(), None, None, targets)
            .await
    else {
        return;
    };

    if let Err(status) = mark_metadata_refresh_targets_pending(db, &queued_targets).await {
        log::warn!(
            "Failed to mark automatic metadata refresh targets pending: {:?}",
            status
        );
        cancel_metadata_refresh_activity(&activity_id).await;
        return;
    }

    mark_metadata_refresh_activity_running(&activity_id).await;
    for target in queued_targets {
        let failed = execute_metadata_refresh_target(db, &target, settings).await;
        record_metadata_refresh_activity_progress(&activity_id, failed).await;
    }
    complete_metadata_refresh_activity(&activity_id).await;
}

async fn run_due_metadata_refreshes(
    db: &DbConn,
    settings: &crate::config::Settings,
) {
    if settings.metadata.refresh_interval_days.is_none() {
        return;
    }

    let now = current_timestamp();
    let links = match db
        .run(move |conn| list_due_item_metadata_links(conn, now, 8))
        .await
    {
        Ok(links) => links,
        Err(error) => {
            log::warn!("Failed to load due metadata refresh links: {}", error);
            return;
        }
    };
    if links.is_empty() {
        return;
    }

    execute_metadata_refresh_links(
        db,
        settings,
        links,
        "automatic_interval_refresh",
        "Refresh stale metadata",
    )
    .await;
}

/// Run scheduled metadata refresh work.
pub(crate) async fn run_scheduled_metadata_refreshes(
    db: &DbConn,
    settings: &crate::config::Settings,
) {
    recover_pending_metadata_refreshes(db, settings).await;
    run_due_metadata_refreshes(db, settings).await;
}

async fn build_metadata_refresh_job(
    db: &DbConn,
    settings: &crate::config::Settings,
    item: &MediaItemSummary,
    provider_id: MetadataProviderId,
    external_id: &str,
    media_type: &str,
) -> Result<MetadataRefreshJob, Status> {
    let root = MetadataRefreshTarget {
        item_id: item.id,
        library_id: item.library_id,
        provider_id,
        item_type: item.item_type.clone(),
        display_title: item.display_title.clone(),
        relative_path: item.relative_path.clone(),
        external_id: external_id.to_string(),
        media_type: media_type.to_string(),
        fetch_kind: MetadataRefreshFetchKind::Direct,
    };
    let descendants = if item.item_type == "show"
        && ((root.provider_id == MetadataProviderId::Tmdb && media_type == "tv")
            || (root.provider_id == MetadataProviderId::Tvdb && media_type == "series"))
    {
        match load_show_descendant_refresh_targets(
            db,
            settings,
            item.id,
            root.provider_id.clone(),
            external_id,
        )
        .await
        {
            Ok(descendants) => descendants,
            Err(status) => {
                if status == Status::ServiceUnavailable {
                    log::warn!(
                        "Skipping descendant metadata refresh expansion for {} because {} is \
                         unavailable; the root item will still be refreshed",
                        describe_metadata_refresh_target(&root),
                        root.provider_id.as_storage_value()
                    );
                    Vec::new()
                } else {
                    return Err(status);
                }
            }
        }
    } else {
        Vec::new()
    };

    Ok(MetadataRefreshJob { root, descendants })
}

async fn load_metadata_summary_for_item(
    db: &DbConn,
    item_id: i32,
    provider_id: MetadataProviderId,
) -> Result<ItemMetadataSummary, Status> {
    let summaries = db
        .run(move |conn| get_item_metadata_summaries(conn, item_id))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to load current metadata summary for media item {}: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?;

    summaries
        .into_iter()
        .find(|summary| summary.provider_id == provider_id)
        .ok_or(Status::NotFound)
}

async fn load_snapshot_descendant_refresh_targets(
    db: &DbConn,
    item_id: i32,
    snapshot: &StoredMetadataSnapshot,
    settings: &crate::config::Settings,
) -> Result<Vec<MetadataRefreshTarget>, Status> {
    if (snapshot.provider_id == MetadataProviderId::Tmdb
        && snapshot.media_type.as_deref() == Some("tv"))
        || (snapshot.provider_id == MetadataProviderId::Tvdb
            && snapshot.media_type.as_deref() == Some("series"))
    {
        Ok(load_show_descendant_refresh_targets(
            db,
            settings,
            item_id,
            snapshot.provider_id.clone(),
            &snapshot.external_id,
        )
        .await?)
    } else {
        Ok(Vec::new())
    }
}

async fn fetch_snapshots_for_item_metadata_languages(
    db: &DbConn,
    settings: &crate::config::Settings,
    item_id: i32,
    provider_id: MetadataProviderId,
    external_id: &str,
    media_type: &str,
    fetch_options: MetadataSnapshotFetchOptions,
) -> Result<Vec<StoredMetadataSnapshot>, Status> {
    let languages = load_refresh_target_metadata_languages(db, item_id)
        .await
        .map_err(|error| {
            log::error!(
                "Failed to load metadata languages for media item {}: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?;
    let languages = metadata_locales_for_provider(provider_id.clone(), &languages);
    let mut snapshots = Vec::new();
    for language in languages {
        snapshots.push(
            fetch_provider_metadata_snapshot_for_locale_with_options(
                &settings.metadata,
                provider_id.clone(),
                external_id,
                media_type,
                &language,
                fetch_options,
            )
            .await
            .map_err(|error| {
                log::error!(
                    "Failed to fetch {} metadata snapshot for item {} locale {}: {}",
                    provider_id.as_storage_value(),
                    item_id,
                    language,
                    error
                );
                Status::ServiceUnavailable
            })?,
        );
    }
    Ok(snapshots)
}

async fn persist_snapshot_tree_for_languages(
    db: &DbConn,
    item_id: i32,
    snapshots: &[StoredMetadataSnapshot],
    settings: &crate::config::Settings,
    persist_options: PersistSnapshotOptions,
) -> Result<ItemMetadataSummary, Status> {
    let descendants = match snapshots.first() {
        Some(snapshot) => {
            load_snapshot_descendant_refresh_targets(db, item_id, snapshot, settings).await?
        }
        None => return Err(Status::ServiceUnavailable),
    };
    if !descendants.is_empty() {
        mark_metadata_refresh_targets_pending(db, &descendants).await?;
    }

    let mut summary = None;
    for snapshot in snapshots {
        summary = Some(
            persist_snapshot_for_item(db, item_id, snapshot, settings, persist_options).await?,
        );
    }
    if !descendants.is_empty() {
        execute_metadata_refresh_targets(db, &descendants, settings).await;
    }
    summary.ok_or(Status::ServiceUnavailable)
}

async fn run_library_metadata_people_refresh(
    db: DbConn,
    settings: Settings,
    library_id: i32,
    library_name: String,
) {
    let targets = match db
        .run(move |conn| list_metadata_people_for_library(conn, library_id))
        .await
    {
        Ok(targets) => targets,
        Err(error) => {
            log::warn!(
                "Failed to load people for library {} metadata enrichment: {}",
                library_id,
                error
            );
            return;
        }
    };
    if targets.is_empty() {
        return;
    }

    log::info!(
        "Starting deferred people metadata refresh for library {} ({}) with {} person row(s)",
        library_id,
        library_name,
        targets.len()
    );

    let mut failed = 0usize;
    for target in targets {
        if !refresh_metadata_person_details(&db, &settings, &target).await {
            failed += 1;
        }
    }

    if failed == 0 {
        log::info!(
            "Completed deferred people metadata refresh for library {} ({})",
            library_id,
            library_name
        );
    } else {
        log::warn!(
            "Completed deferred people metadata refresh for library {} ({}) with {} failure(s)",
            library_id,
            library_name,
            failed
        );
    }
}

async fn refresh_metadata_person_details(
    db: &DbConn,
    settings: &Settings,
    target: &MetadataPersonEnrichmentTarget,
) -> bool {
    let mut details = match fetch_provider_person_metadata_for_locale(
        &settings.metadata,
        target.provider_id.clone(),
        &target.external_id,
        &target.locale_key,
    )
    .await
    {
        Ok(Some(details)) => details,
        Ok(None) => return true,
        Err(error) => {
            log::warn!(
                "Failed to fetch {} person metadata for {} ({}): {}",
                target.provider_id.as_storage_value(),
                target.name,
                target.external_id,
                error
            );
            return false;
        }
    };

    cache_deferred_person_image(settings, target, &mut details).await;

    let person_id = target.id;
    match db
        .run(move |conn| update_metadata_person_details(conn, person_id, &details))
        .await
    {
        Ok(_) => true,
        Err(error) => {
            log::warn!(
                "Failed to store {} person metadata for {} ({}): {}",
                target.provider_id.as_storage_value(),
                target.name,
                target.external_id,
                error
            );
            false
        }
    }
}

async fn cache_deferred_person_image(
    settings: &Settings,
    target: &MetadataPersonEnrichmentTarget,
    details: &mut ProviderMetadataPerson,
) {
    let Some(image_url) = details.image_url.as_deref() else {
        return;
    };
    let person_dir = managed_metadata_asset_dir(
        &settings.general.data_dir,
        target.provider_id.clone(),
        &target.external_id,
        Some("person"),
        &target.locale_key,
    );
    let cache_key = format!("{}_profile", target.provider_id.as_storage_value());
    let Some(path) = try_cache_item_artwork(image_url, &person_dir, &cache_key).await else {
        return;
    };
    details.cached_image_path = Some(metadata_asset_db_path(&settings.general.data_dir, &path));
}

fn linked_shows_needing_descendant_backfill(
    conn: &mut rocket_sync_db_pools::diesel::SqliteConnection
) -> Result<Vec<(i32, String)>, diesel::result::Error> {
    let items = list_media_items(conn, None)?;
    let mut pending = Vec::new();

    for show in items.into_iter().filter(|item| item.item_type == "show") {
        let Some(link) = get_primary_item_metadata_link(conn, show.id)? else {
            continue;
        };
        if link.provider_id != MetadataProviderId::Tmdb.as_storage_value()
            || link.media_type.as_deref() != Some("tv")
        {
            continue;
        }

        let seasons = list_media_item_children(conn, show.id)?;
        let mut needs_backfill = false;
        for season in seasons
            .into_iter()
            .filter(|item| item.item_type == "season")
        {
            if descendant_metadata_needs_backfill(get_primary_item_metadata_link(conn, season.id)?)
            {
                needs_backfill = true;
                break;
            }

            let episodes = list_media_item_children(conn, season.id)?;
            if episodes.into_iter().any(|episode| {
                episode.item_type == "episode"
                    && descendant_metadata_needs_backfill(
                        get_primary_item_metadata_link(conn, episode.id)
                            .ok()
                            .flatten(),
                    )
            }) {
                needs_backfill = true;
                break;
            }
        }

        if needs_backfill {
            pending.push((show.id, link.external_id));
        }
    }

    Ok(pending)
}

fn descendant_metadata_needs_backfill(link: Option<ItemMetadataLink>) -> bool {
    match link {
        None => true,
        Some(link) => link.refresh_state == "error",
    }
}

async fn run_automatic_movie_metadata_linking(
    db: &DbConn,
    settings: &crate::config::Settings,
    library_id: Option<i32>,
    retry_previously_attempted: bool,
    activity: Option<(String, Vec<i32>)>,
) {
    let ready_providers = list_provider_statuses(&settings.metadata)
        .into_iter()
        .filter(|provider| provider.configured && provider.implemented)
        .map(|provider| provider.id)
        .collect::<std::collections::HashSet<_>>();

    let mut candidates = match db
        .run(move |conn| {
            if retry_previously_attempted {
                list_automatic_metadata_refresh_candidates(conn, library_id, usize::MAX)
            } else {
                list_automatic_metadata_candidates(conn, library_id, 8)
            }
        })
        .await
    {
        Ok(candidates) => candidates,
        Err(error) => {
            log::warn!("Failed to load automatic metadata candidates: {}", error);
            return;
        }
    };

    let activity = if retry_previously_attempted {
        if activity.is_some() {
            activity
        } else {
            let item_ids = candidates
                .iter()
                .map(|candidate| candidate.item_id)
                .collect::<Vec<_>>();
            register_metadata_refresh_activity_for_items(
                "library",
                "manual_library_automatch",
                "Match unlinked library metadata".into(),
                library_id,
                None,
                None,
                item_ids,
            )
            .await
        }
    } else {
        None
    };
    if let Some((_, queued_item_ids)) = &activity {
        let queued_item_ids = queued_item_ids
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        candidates.retain(|candidate| queued_item_ids.contains(&candidate.item_id));
    }
    if let Some((activity_id, _)) = &activity {
        mark_metadata_refresh_activity_running(activity_id).await;
    }

    for candidate in candidates {
        let mut failed = false;
        let mut guessed_provider_id = None;
        let mut guess = None;
        for provider_id in candidate
            .metadata_providers
            .iter()
            .filter(|provider_id| ready_providers.contains(provider_id))
        {
            let guess_result = match candidate.library_kind {
                crate::config::MediaLibraryKind::Shows => {
                    guess_provider_show_match(
                        &settings.metadata,
                        provider_id.clone(),
                        &candidate.relative_path,
                        &candidate.display_title,
                    )
                    .await
                }
                _ => {
                    guess_provider_movie_match(
                        &settings.metadata,
                        provider_id.clone(),
                        &candidate.relative_path,
                        &candidate.display_title,
                    )
                    .await
                }
            };
            match guess_result {
                Ok(Some(result)) => {
                    guessed_provider_id = Some(provider_id.clone());
                    guess = Some(result);
                    break;
                }
                Ok(None) => {}
                Err(error) => {
                    log::warn!(
                        "Automatic {} match failed for item {} ({}): {}",
                        provider_id.as_storage_value(),
                        candidate.item_id,
                        candidate.relative_path,
                        error
                    );
                    failed = true;
                }
            }
        }

        if let (Some(provider_id), Some(result)) = (guessed_provider_id, guess) {
            if let Err(error) = db
                .run({
                    let external_id = result.external_id.clone();
                    let media_type = result.media_type.clone();
                    let provider_id = provider_id.clone();
                    move |conn| {
                        set_item_metadata_refresh_state(
                            conn,
                            candidate.item_id,
                            provider_id,
                            &external_id,
                            Some(&media_type),
                            "pending",
                            None,
                        )
                    }
                })
                .await
            {
                log::warn!(
                    "Failed to mark automatic metadata candidate {} pending: {}",
                    candidate.item_id,
                    error
                );
                failed = true;
            }
            match fetch_snapshots_for_item_metadata_languages(
                db,
                settings,
                candidate.item_id,
                provider_id.clone(),
                &result.external_id,
                &result.media_type,
                MetadataSnapshotFetchOptions::WITHOUT_PERSON_DETAILS,
            )
            .await
            {
                Ok(snapshots) => {
                    if let Err(status) = persist_snapshot_tree_for_languages(
                        db,
                        candidate.item_id,
                        &snapshots,
                        settings,
                        PersistSnapshotOptions::WITHOUT_PERSON_ASSETS,
                    )
                    .await
                    {
                        log::warn!(
                            "Failed to persist automatic metadata snapshot for item {}: {:?}",
                            candidate.item_id,
                            status
                        );
                        if let Err(error) = db
                            .run({
                                let external_id = result.external_id.clone();
                                let media_type = Some(result.media_type.clone());
                                let status_message = format!("{status:?}");
                                let provider_id = provider_id.clone();
                                move |conn| {
                                    set_item_metadata_refresh_state(
                                        conn,
                                        candidate.item_id,
                                        provider_id,
                                        &external_id,
                                        media_type.as_deref(),
                                        "error",
                                        Some(&status_message),
                                    )
                                }
                            })
                            .await
                        {
                            log::warn!(
                                "Failed to record automatic metadata error for item {}: {}",
                                candidate.item_id,
                                error
                            );
                        }
                        failed = true;
                    }
                }
                Err(error) => {
                    log::warn!(
                        "Failed to fetch automatic {} snapshot for item {}: {}",
                        provider_id.as_storage_value(),
                        candidate.item_id,
                        error
                    );
                    if let Err(persist_error) = db
                        .run({
                            let external_id = result.external_id.clone();
                            let media_type = result.media_type.clone();
                            let error_message = format!("{error:?}");
                            let provider_id = provider_id.clone();
                            move |conn| {
                                set_item_metadata_refresh_state(
                                    conn,
                                    candidate.item_id,
                                    provider_id,
                                    &external_id,
                                    Some(&media_type),
                                    "error",
                                    Some(&error_message),
                                )
                            }
                        })
                        .await
                    {
                        log::warn!(
                            "Failed to record automatic metadata error for item {}: {}",
                            candidate.item_id,
                            persist_error
                        );
                    }
                    failed = true;
                }
            }
        }

        if candidate.library_kind != crate::config::MediaLibraryKind::Shows {
            let attempted_at = current_timestamp();
            if let Err(error) = db
                .run(move |conn| {
                    mark_metadata_match_attempted(conn, candidate.item_id, attempted_at)
                })
                .await
            {
                log::warn!(
                    "Failed to record automatic metadata attempt for item {}: {}",
                    candidate.item_id,
                    error
                );
                failed = true;
            }
        }

        if let Some((activity_id, _)) = &activity {
            record_metadata_refresh_activity_progress(activity_id, failed).await;
        }
    }

    if let Some((activity_id, _)) = &activity {
        complete_metadata_refresh_activity(activity_id).await;
    }

    let pending_show_backfills = match db.run(linked_shows_needing_descendant_backfill).await {
        Ok(items) => items,
        Err(error) => {
            log::warn!(
                "Failed to load linked shows needing metadata backfill: {}",
                error
            );
            return;
        }
    };

    for (show_item_id, external_id) in pending_show_backfills {
        match load_show_descendant_refresh_targets(
            db,
            settings,
            show_item_id,
            MetadataProviderId::Tmdb,
            &external_id,
        )
        .await
        {
            Ok(targets) => {
                if let Err(status) = mark_metadata_refresh_targets_pending(db, &targets).await {
                    log::warn!(
                        "Failed to mark descendant metadata pending for show item {}: {:?}",
                        show_item_id,
                        status
                    );
                }
                execute_metadata_refresh_targets(db, &targets, settings).await;
            }
            Err(status) => {
                log::warn!(
                    "Failed to backfill descendant metadata for show item {}: {:?}",
                    show_item_id,
                    status
                );
            }
        }
    }

    if library_id.is_none() {
        recover_pending_metadata_refreshes(db, settings).await;
        run_due_metadata_refreshes(db, settings).await;
    }
}

fn current_user_id(user_guard: Option<&UserGuard>) -> Result<Option<i32>, Status> {
    user_guard
        .map(|user_guard| {
            user_guard
                .claims()
                .sub
                .parse::<i32>()
                .map_err(|_| Status::Unauthorized)
        })
        .transpose()
}

fn user_preferred_metadata_languages(
    conn: &mut rocket_sync_db_pools::diesel::SqliteConnection,
    user_id: Option<i32>,
) -> Result<Vec<String>, diesel::result::Error> {
    use crate::db::schema::users::dsl as users_dsl;
    use diesel::{
        ExpressionMethods,
        OptionalExtension,
        QueryDsl,
        RunQueryDsl,
    };

    let Some(user_id) = user_id else {
        return Ok(vec![crate::metadata::DEFAULT_METADATA_LOCALE.to_string()]);
    };

    let stored = users_dsl::users
        .filter(users_dsl::id.eq(user_id))
        .select(users_dsl::preferred_metadata_languages_json)
        .first::<String>(conn)
        .optional()?
        .unwrap_or_else(|| "[\"en-US\"]".into());

    Ok(crate::web::routes::user::parse_preferred_metadata_languages(&stored))
}

async fn load_item_library_metadata_providers(
    db: &DbConn,
    library_id: i32,
) -> Result<Vec<MetadataProviderId>, Status> {
    let providers = db
        .run(move |conn| get_library_metadata_providers(conn, library_id))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to load library metadata providers for library {}: {}",
                library_id,
                error
            );
            Status::InternalServerError
        })?;

    providers.ok_or(Status::NotFound)
}

async fn load_item_library_metadata_languages(
    db: &DbConn,
    library_id: i32,
) -> Result<Vec<String>, Status> {
    let languages = db
        .run(move |conn| get_library_metadata_languages(conn, library_id))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to load library metadata languages for library {}: {}",
                library_id,
                error
            );
            Status::InternalServerError
        })?;

    Ok(languages.unwrap_or_else(|| vec![crate::metadata::DEFAULT_METADATA_LOCALE.to_string()]))
}

async fn load_refresh_target_metadata_languages(
    db: &DbConn,
    item_id: i32,
) -> Result<Vec<String>, String> {
    let item = db
        .run(move |conn| get_media_item_summary(conn, item_id))
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "media item was not found".to_string())?;
    load_item_library_metadata_languages(db, item.library_id)
        .await
        .map_err(|status| format!("{status:?}"))
}

async fn load_library_refresh_jobs(
    db: &DbConn,
    settings: &crate::config::Settings,
    library_id: i32,
) -> Result<Vec<MetadataRefreshJob>, Status> {
    let items = db
        .run(move |conn| list_media_items(conn, Some(library_id)))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to load media items for library {} metadata refresh: {}",
                library_id,
                error
            );
            Status::InternalServerError
        })?;

    let mut jobs = Vec::new();
    for item in items
        .into_iter()
        .filter(|item| item.parent_id.is_none() && supports_manual_metadata_linking(item))
    {
        let item_id = item.id;
        let link = db
            .run(move |conn| get_primary_item_metadata_link(conn, item_id))
            .await
            .map_err(|error| {
                log::error!(
                    "Failed to load linked metadata for media item {} library refresh: {}",
                    item_id,
                    error
                );
                Status::InternalServerError
            })?;
        let Some(link) = link else {
            continue;
        };

        let provider_id =
            MetadataProviderId::from_storage_value(&link.provider_id).ok_or(Status::BadRequest)?;
        let Some(media_type) = link.media_type.clone() else {
            continue;
        };
        jobs.push(
            build_metadata_refresh_job(
                db,
                settings,
                &item,
                provider_id,
                &link.external_id,
                &media_type,
            )
            .await?,
        );
    }

    Ok(jobs)
}

async fn load_library_summary(
    db: &DbConn,
    library_id: i32,
) -> Result<PersistedLibrarySummary, Status> {
    let libraries = db
        .run(get_persisted_library_summaries)
        .await
        .map_err(|error| {
            log::error!("Failed to load media library summaries: {}", error);
            Status::InternalServerError
        })?;

    libraries
        .into_iter()
        .find(|library| library.id == library_id)
        .ok_or(Status::NotFound)
}

/// Return server bootstrap information for future browser and native clients.
#[openapi(tag = "Media")]
#[get("/api/v1/system/capabilities")]
pub async fn get_server_capabilities(
    db: DbConn
) -> Result<Json<ServerCapabilitiesResponse>, Status> {
    let settings = current_settings();
    let transcoding = inspect_transcoding_capability(&settings.ffmpeg);
    let libraries_configured = db
        .run(|conn| list_library_settings(conn).map(|libraries| libraries.len()))
        .await
        .map_err(|error| {
            log::error!("Failed to count persisted libraries: {}", error);
            Status::InternalServerError
        })?;

    Ok(Json(ServerCapabilitiesResponse {
        app_name: globals::GLOBAL_APP_NAME.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        server_url: globals::get_server_url(),
        https_enabled: settings.server.use_https,
        libraries_configured,
        api_versions: vec!["v1".into()],
        transcoding,
    }))
}

/// Trigger a full catalog scan and return the updated summary for one library.
#[openapi(tag = "Media")]
#[post("/api/v1/libraries/<library_id>/scan")]
pub async fn scan_library(
    db: DbConn,
    library_id: i32,
) -> Result<Json<PersistedLibrarySummary>, Status> {
    let settings = current_settings();
    let library_summary = load_library_summary(&db, library_id).await?;
    if !begin_catalog_scan_execution() {
        log::info!(
            "Skipping duplicate manual library {} scan request; a catalog scan is already running",
            library_id
        );
        return Ok(Json(library_summary));
    }

    let ffmpeg_settings = settings.ffmpeg.clone();
    let library_name = library_summary.name.clone();
    let activity_id = register_library_scan_activity(library_id, &library_name).await;
    tokio::spawn(async move {
        mark_library_scan_activity_running(&activity_id).await;
        log::info!(
            "Starting manual media library catalog scan for library {} ({})",
            library_id,
            library_name
        );
        let sync_result = db
            .run(move |conn| {
                sync_persisted_library_catalog_for_library(conn, &ffmpeg_settings, library_id)
            })
            .await;
        let failed = match sync_result {
            Ok(Some(summary)) => {
                log::info!(
                    "Completed manual media library catalog scan for library {} ({}): {} file(s), \
                     status {:?}",
                    summary.id,
                    summary.name,
                    summary.total_files,
                    summary.status
                );
                false
            }
            Ok(None) => {
                log::warn!(
                    "Manual media library catalog scan requested missing library {}",
                    library_id
                );
                true
            }
            Err(error) => {
                log::error!(
                    "Failed to run manual library scan for library {}: {}",
                    library_id,
                    error
                );
                true
            }
        };
        if failed {
            log::warn!(
                "Manual media library catalog scan did not complete successfully for library {}",
                library_id
            );
        }
        complete_library_scan_activity(&activity_id, failed).await;
        finish_catalog_scan_execution();
    });

    Ok(Json(library_summary))
}

/// Delete items currently marked missing from one library's active catalog.
#[openapi(tag = "Media")]
#[delete("/api/v1/libraries/<library_id>/missing")]
pub async fn delete_library_missing_items(
    db: DbConn,
    library_id: i32,
) -> Result<Json<MissingItemsCleanupResponse>, Status> {
    let exists = db
        .run(move |conn| library_exists(conn, library_id))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to inspect library {} before missing-item cleanup: {}",
                library_id,
                error
            );
            Status::InternalServerError
        })?;
    if !exists {
        return Err(Status::NotFound);
    }

    let cleanup = db
        .run(move |conn| delete_missing_media_items(conn, Some(library_id), None))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to delete missing media items for library {}: {}",
                library_id,
                error
            );
            Status::InternalServerError
        })?;
    let library = load_library_summary(&db, library_id).await?;

    Ok(Json(MissingItemsCleanupResponse {
        library_id,
        deleted_files: cleanup.deleted_files,
        deleted_items: cleanup.deleted_items,
        removed_collection_items: cleanup.removed_collection_items,
        library,
    }))
}

/// Return active backend activities such as metadata refresh work.
#[openapi(tag = "Media")]
#[get("/api/v1/system/activities")]
pub async fn get_system_activities() -> Json<SystemActivitiesResponse> {
    Json(SystemActivitiesResponse {
        generated_at: current_timestamp(),
        activities: current_system_activities().await,
    })
}

/// Return metadata provider status for the current server configuration.
#[openapi(tag = "Media")]
#[get("/api/v1/metadata/providers")]
pub fn get_metadata_providers() -> Json<Vec<MetadataProviderStatus>> {
    Json(list_provider_statuses(&current_settings().metadata))
}

/// Return known locale keys and provider-specific mappings.
#[openapi(tag = "Media")]
#[get("/api/v1/metadata/locales")]
pub fn get_metadata_locales() -> Json<Vec<MetadataLocale>> {
    Json(vec![
        MetadataLocale {
            key: "en-US".into(),
            name: "English (United States)".into(),
            tmdb: "en-US".into(),
            tvdb: "eng".into(),
        },
        MetadataLocale {
            key: "en-GB".into(),
            name: "English (United Kingdom)".into(),
            tmdb: "en-GB".into(),
            tvdb: "eng".into(),
        },
        MetadataLocale {
            key: "es-ES".into(),
            name: "Spanish (Spain)".into(),
            tmdb: "es-ES".into(),
            tvdb: "spa".into(),
        },
        MetadataLocale {
            key: "fr-FR".into(),
            name: "French (France)".into(),
            tmdb: "fr-FR".into(),
            tvdb: "fra".into(),
        },
        MetadataLocale {
            key: "de-DE".into(),
            name: "German (Germany)".into(),
            tmdb: "de-DE".into(),
            tvdb: "deu".into(),
        },
        MetadataLocale {
            key: "it-IT".into(),
            name: "Italian (Italy)".into(),
            tmdb: "it-IT".into(),
            tvdb: "ita".into(),
        },
        MetadataLocale {
            key: "ja-JP".into(),
            name: "Japanese (Japan)".into(),
            tmdb: "ja-JP".into(),
            tvdb: "jpn".into(),
        },
        MetadataLocale {
            key: "pt-BR".into(),
            name: "Portuguese (Brazil)".into(),
            tmdb: "pt-BR".into(),
            tvdb: "por".into(),
        },
    ])
}

/// Return lightweight scan summaries for the configured media libraries.
#[openapi(tag = "Media")]
#[get("/api/v1/libraries")]
pub async fn get_libraries(
    db: DbConn,
    user_guard: Option<UserGuard>,
) -> Result<Json<Vec<PersistedLibrarySummary>>, Status> {
    let user_id = current_user_id(user_guard.as_ref())?;

    let libraries = db
        .run(move |conn| {
            let libraries = get_persisted_library_summaries(conn)?;
            libraries
                .into_iter()
                .filter_map(
                    |library| match user_can_access_library(conn, library.id, user_id) {
                        Ok(true) => Some(Ok(library)),
                        Ok(false) => None,
                        Err(error) => Some(Err(error)),
                    },
                )
                .collect::<Result<Vec<_>, _>>()
        })
        .await
        .map_err(|error| {
            log::error!("Failed to load media library summaries: {}", error);
            Status::InternalServerError
        })?;

    Ok(Json(libraries))
}

/// Return Kodi/Plex-style shelves for the browser home screen.
#[openapi(tag = "Media")]
#[get("/api/v1/home?<library_id>")]
pub async fn get_home(
    db: DbConn,
    user_guard: Option<UserGuard>,
    library_id: Option<i32>,
) -> Result<Json<MediaHome>, Status> {
    let user_id = current_user_id(user_guard.as_ref())?;
    if let Some(library_id) = library_id {
        let can_access = db
            .run(move |conn| user_can_access_library(conn, library_id, user_id))
            .await
            .map_err(|error| {
                log::error!(
                    "Failed to check library access for {}: {}",
                    library_id,
                    error
                );
                Status::InternalServerError
            })?;
        if !can_access {
            return Err(Status::NotFound);
        }
    }

    let home = db
        .run(move |conn| {
            let languages = user_preferred_metadata_languages(conn, user_id)?;
            get_media_home_with_preferred_languages(conn, user_id, library_id, &languages)
        })
        .await
        .map_err(|error| {
            log::error!("Failed to build media home shelves: {}", error);
            Status::InternalServerError
        })?;

    Ok(Json(home))
}

/// Return the persisted file inventory for a synchronized media library.
#[openapi(tag = "Media")]
#[get("/api/v1/libraries/<library_id>/files")]
pub async fn get_library_inventory(
    db: DbConn,
    library_id: i32,
) -> Result<Json<Vec<PersistedMediaFileSummary>>, Status> {
    let file_rows = db
        .run(move |conn| get_library_files(conn, library_id))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to load media library inventory for id {}: {}",
                library_id,
                error
            );
            Status::InternalServerError
        })?;

    if file_rows.is_empty() {
        let exists = db
            .run(move |conn| library_exists(conn, library_id))
            .await
            .map_err(|error| {
                log::error!(
                    "Failed to confirm media library existence for id {}: {}",
                    library_id,
                    error
                );
                Status::InternalServerError
            })?;

        if !exists {
            return Err(Status::NotFound);
        }
    }

    Ok(Json(file_rows))
}

/// Return browser-facing media items, optionally filtered to one library.
#[openapi(tag = "Media")]
#[get("/api/v1/items?<library_id>")]
pub async fn get_items(
    db: DbConn,
    user_guard: Option<UserGuard>,
    library_id: Option<i32>,
) -> Result<Json<Vec<MediaItemSummary>>, Status> {
    let user_id = current_user_id(user_guard.as_ref())?;
    if let Some(library_id) = library_id {
        let can_access = db
            .run(move |conn| user_can_access_library(conn, library_id, user_id))
            .await
            .map_err(|error| {
                log::error!(
                    "Failed to check library access for {}: {}",
                    library_id,
                    error
                );
                Status::InternalServerError
            })?;
        if !can_access {
            return Err(Status::NotFound);
        }
    }

    let items = db
        .run(move |conn| {
            let languages = user_preferred_metadata_languages(conn, user_id)?;
            list_media_items_for_user_with_preferred_languages(
                conn, user_id, library_id, &languages,
            )
        })
        .await
        .map_err(|error| {
            log::error!("Failed to load media items: {}", error);
            Status::InternalServerError
        })?;

    Ok(Json(items))
}

/// Return details for one browser-facing media item.
#[openapi(tag = "Media")]
#[get("/api/v1/items/<item_id>")]
pub async fn get_item(
    db: DbConn,
    user_guard: Option<UserGuard>,
    item_id: i32,
) -> Result<Json<MediaItemDetail>, Status> {
    let settings = current_settings();
    let data_dir = settings.general.data_dir.clone();
    let user_id = current_user_id(user_guard.as_ref())?;

    let item = db
        .run(move |conn| {
            let languages = user_preferred_metadata_languages(conn, user_id)?;
            get_media_item_with_preferred_languages(conn, item_id, &data_dir, &languages)
        })
        .await
        .map_err(|error| {
            log::error!("Failed to load media item {}: {}", item_id, error);
            Status::InternalServerError
        })?;

    let mut item = item.ok_or(Status::NotFound)?;
    let can_access = db
        .run({
            let library_id = item.library_id;
            move |conn| user_can_access_library(conn, library_id, user_id)
        })
        .await
        .map_err(|error| {
            log::error!(
                "Failed to check library access for item {}: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?;
    if !can_access {
        return Err(Status::NotFound);
    }
    let item = db
        .run(move |conn| {
            apply_user_playback_context_to_detail(conn, user_id, &mut item)?;
            Ok::<_, diesel::result::Error>(item)
        })
        .await
        .map_err(|error| {
            log::error!(
                "Failed to load playback context for media item {}: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?;

    Ok(Json(item))
}

/// Create a new playback session.
#[openapi(tag = "Media")]
#[post("/api/v1/sessions", format = "json", data = "<request>")]
pub async fn create_session(
    db: DbConn,
    user_guard: Option<UserGuard>,
    request: Json<CreateSessionRequest>,
) -> Result<Json<crate::media::PlaybackSession>, Status> {
    let payload = request.into_inner();
    let user_id = current_user_id(user_guard.as_ref()).unwrap_or(None);
    let preferred_languages = db
        .run(move |conn| user_preferred_metadata_languages(conn, user_id))
        .await
        .map_err(|_| Status::InternalServerError)?;

    let decision = db
        .run({
            let profile = payload.client_profile.clone();
            move |conn| get_playback_decision(conn, payload.item_id, Some(&profile))
        })
        .await
        .map_err(|_| Status::InternalServerError)?
        .ok_or(Status::NotFound)?;
    let audio_stream_index = db
        .run({
            let preferred_languages = preferred_languages.clone();
            move |conn| preferred_audio_stream_index(conn, payload.item_id, &preferred_languages)
        })
        .await
        .map_err(|_| Status::InternalServerError)?
        .filter(|index| *index > 0);

    let session_id = crate::transcode::next_session_id();

    let session = crate::media::PlaybackSession {
        session_id: session_id.clone(),
        item_id: payload.item_id,
        user_id,
        client_profile: payload.client_profile,
        decision,
        created_at: current_timestamp(),
        audio_stream_index,
    };

    ACTIVE_PLAYBACK_SESSIONS
        .write()
        .await
        .insert(session_id, session.clone());

    Ok(Json(session))
}

/// Delete a playback session.
#[openapi(tag = "Media")]
#[delete("/api/v1/sessions/<session_id>")]
pub async fn delete_session(session_id: String) -> Status {
    let removed = ACTIVE_PLAYBACK_SESSIONS.write().await.remove(&session_id);

    if removed.is_some() {
        stop_active_transcode(&session_id).await;

        let settings = current_settings();
        let session_dir = PathBuf::from(&settings.general.data_dir)
            .join("transcode_cache")
            .join(&session_id);

        // Background cleanup
        tokio::spawn(async move {
            let _ = tokio::fs::remove_dir_all(session_dir).await;
        });

        Status::NoContent
    } else {
        Status::NotFound
    }
}

/// Stream content for a session (handles transcode or direct play).
#[rocket::get("/api/v1/sessions/<session_id>/stream?<start_ms>&<audio_stream_index>")]
pub async fn get_session_stream(
    db: DbConn,
    range: RangeHeader,
    session_id: String,
    start_ms: Option<i64>,
    audio_stream_index: Option<usize>,
) -> Result<SessionStream, Status> {
    let session = ACTIVE_PLAYBACK_SESSIONS
        .read()
        .await
        .get(&session_id)
        .cloned()
        .ok_or(Status::NotFound)?;
    let selected_audio_stream_index = audio_stream_index.or(session.audio_stream_index);

    if session.decision.can_direct_play && selected_audio_stream_index.unwrap_or_default() == 0 {
        stop_active_transcode(&session_id).await;

        let source_path = db
            .run(move |conn| resolve_media_item_source_path(conn, session.item_id))
            .await
            .map_err(|_| Status::InternalServerError)?
            .ok_or(Status::NotFound)?;
        return open_ranged_file(source_path, &range)
            .await
            .map(SessionStream::File);
    }

    let settings = current_settings();
    let session_dir = PathBuf::from(&settings.general.data_dir)
        .join("transcode_cache")
        .join(&session_id);

    // We'll write to an mp4 or matching container file
    let container = session
        .decision
        .transcode_container
        .clone()
        .unwrap_or_else(|| "mp4".into());
    let output_path = session_dir.join(format!("output.{}", container));

    let source_path = db
        .run(move |conn| resolve_media_item_source_path(conn, session.item_id))
        .await
        .map_err(|_| Status::InternalServerError)?
        .ok_or(Status::NotFound)?;

    let alternate_audio_stream_selected = selected_audio_stream_index.unwrap_or_default() > 0;
    let video_codec =
        if alternate_audio_stream_selected && session.decision.transcode_video_codec.is_none() {
            Some("libx264".into())
        } else {
            session.decision.transcode_video_codec.clone()
        };
    let audio_codec =
        if alternate_audio_stream_selected && session.decision.transcode_audio_codec.is_none() {
            Some("aac".into())
        } else {
            session.decision.transcode_audio_codec.clone()
        };

    let spec = crate::transcode::TranscodeSpec {
        source_path,
        output_path: output_path.clone(),
        container,
        video_codec,
        audio_codec,
        max_width: if session.client_profile.max_video_width > 0 {
            Some(session.client_profile.max_video_width)
        } else {
            None
        },
        max_height: if session.client_profile.max_video_height > 0 {
            Some(session.client_profile.max_video_height)
        } else {
            None
        },
        max_bitrate_kbps: if session.client_profile.max_bitrate_kbps > 0 {
            Some(session.client_profile.max_bitrate_kbps)
        } else {
            None
        },
        start_time_ms: start_ms.filter(|value| *value > 0),
        audio_stream_index: selected_audio_stream_index,
    };

    stop_active_transcode(&session_id).await;

    match crate::transcode::spawn_transcode_stdout(&session_id, &spec, &settings.ffmpeg).await {
        Ok(mut child) => {
            let stdout = child.stdout.take().ok_or(Status::InternalServerError)?;
            let stderr = child.stderr.take();
            let transcode_session_id = session_id.clone();
            let handle = tokio::spawn(async move {
                let mut stderr_text = Vec::new();
                if let Some(mut stderr) = stderr {
                    let _ = stderr.read_to_end(&mut stderr_text).await;
                }
                let status = child.wait().await;
                if !stderr_text.is_empty() {
                    log::warn!(
                        "FFmpeg stderr: {}",
                        String::from_utf8_lossy(&stderr_text).trim()
                    );
                }
                if let Ok(status) = status {
                    if !status.success() {
                        log::warn!("FFmpeg exited with status: {}", status);
                    }
                }
            });
            replace_active_transcode(transcode_session_id, handle).await;

            Ok(SessionStream::Transcode {
                content_type: ContentType::MP4,
                stdout,
            })
        }
        Err(e) => {
            log::error!("Failed to spawn transcode: {}", e);
            Err(Status::InternalServerError)
        }
    }
}

/// Return direct-play versus transcode information for a media item.
#[openapi(tag = "Media")]
#[get("/api/v1/items/<item_id>/playback")]
pub async fn get_item_playback(
    db: DbConn,
    item_id: i32,
) -> Result<Json<PlaybackDecision>, Status> {
    let decision = db
        .run(move |conn| get_playback_decision(conn, item_id, None))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to build playback decision for media item {}: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?;

    decision.map(Json).ok_or(Status::NotFound)
}

/// Serve a direct-play file stream for a browser-compatible media item.
#[get("/api/v1/items/<item_id>/stream")]
pub async fn stream_item(
    db: DbConn,
    range: RangeHeader,
    item_id: i32,
) -> Result<RangedFile, Status> {
    let decision = db
        .run(move |conn| get_playback_decision(conn, item_id, None))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to build playback decision before streaming item {}: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?
        .ok_or(Status::NotFound)?;

    if !decision.can_direct_play {
        return Err(Status::Conflict);
    }

    let source_path = db
        .run(move |conn| resolve_media_item_source_path(conn, item_id))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to resolve stream source for media item {}: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?
        .ok_or(Status::NotFound)?;

    open_ranged_file(source_path, &range).await
}

/// Persist browser playback progress for a media item.
#[openapi(tag = "Media")]
#[post(
    "/api/v1/items/<item_id>/progress",
    format = "json",
    data = "<request>"
)]
pub async fn update_item_progress(
    db: DbConn,
    user_guard: UserGuard,
    item_id: i32,
    request: Json<PlaybackProgressRequest>,
) -> Result<Status, Status> {
    let payload = request.into_inner();
    let user_id = current_user_id(Some(&user_guard))?.ok_or(Status::Unauthorized)?;

    db.run(move |conn| {
        upsert_playback_progress(
            conn,
            user_id,
            item_id,
            payload.position_ms,
            payload.duration_ms,
            payload.completed,
        )
    })
    .await
    .map_err(|error| {
        log::error!(
            "Failed to update playback progress for media item {}: {}",
            item_id,
            error
        );
        Status::InternalServerError
    })?;

    Ok(Status::Ok)
}

/// Return stored metadata matches and provider readiness for one media item.
#[openapi(tag = "Media")]
#[get("/api/v1/items/<item_id>/metadata")]
pub async fn get_item_metadata(
    db: DbConn,
    user_guard: Option<UserGuard>,
    item_id: i32,
) -> Result<Json<ItemMetadataResponse>, Status> {
    let data_dir = current_settings().general.data_dir;
    let user_id = current_user_id(user_guard.as_ref())?;

    let item_exists = db
        .run(move |conn| get_media_item(conn, item_id, &data_dir))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to confirm media item {} before metadata load: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?;

    if item_exists.is_none() {
        return Err(Status::NotFound);
    }

    let matches = db
        .run(move |conn| {
            let languages = user_preferred_metadata_languages(conn, user_id)?;
            let mut summaries = get_item_metadata_summaries(conn, item_id)?;
            sort_item_metadata_summaries_for_languages(&mut summaries, &languages);
            Ok::<_, diesel::result::Error>(summaries)
        })
        .await
        .map_err(|error| {
            log::error!(
                "Failed to load metadata matches for media item {}: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?;

    Ok(Json(ItemMetadataResponse {
        item_id,
        providers: list_provider_statuses(&current_settings().metadata),
        matches,
    }))
}

/// Return one normalized person and the media items they are credited on.
#[openapi(tag = "Media")]
#[get("/api/v1/people/<person_id>")]
pub async fn get_person(
    db: DbConn,
    user_guard: Option<UserGuard>,
    person_id: i32,
) -> Result<Json<MetadataPersonResponse>, Status> {
    let user_id = current_user_id(user_guard.as_ref())?;
    let response = db
        .run(move |conn| {
            let languages = user_preferred_metadata_languages(conn, user_id)?;
            let person = get_metadata_person_for_languages(conn, person_id, &languages)?
                .ok_or(diesel::result::Error::NotFound)?;
            let person_ids = get_metadata_person_locale_peer_ids(conn, person_id)?;
            let mut credits = Vec::new();
            let mut seen_items = std::collections::HashSet::new();
            for credit in list_metadata_person_credit_summaries_for_person_ids(conn, &person_ids)? {
                if !seen_items.insert(credit.media_item_id) {
                    continue;
                }
                if let Some((item, hierarchy)) =
                    crate::media::get_media_item_summary_with_hierarchy(
                        conn,
                        credit.media_item_id,
                        &languages,
                    )?
                {
                    credits.push(MetadataPersonItemCredit {
                        credit,
                        item,
                        hierarchy,
                    });
                }
            }
            Ok::<_, diesel::result::Error>(MetadataPersonResponse { person, credits })
        })
        .await
        .map_err(|error| match error {
            diesel::result::Error::NotFound => Status::NotFound,
            error => {
                log::error!(
                    "Failed to load metadata person {} detail: {}",
                    person_id,
                    error
                );
                Status::InternalServerError
            }
        })?;

    Ok(Json(response))
}

/// Serve a cached local person profile image.
#[get("/api/v1/people/<person_id>/image")]
pub async fn get_person_image(
    db: DbConn,
    user_guard: Option<UserGuard>,
    person_id: i32,
) -> Result<NamedFile, Status> {
    let user_id = current_user_id(user_guard.as_ref())?;
    let data_dir = current_settings().general.data_dir;
    let image_path = db
        .run(move |conn| {
            let languages = user_preferred_metadata_languages(conn, user_id)?;
            let person = get_metadata_person_for_languages(conn, person_id, &languages)?
                .ok_or(diesel::result::Error::NotFound)?;
            Ok::<_, diesel::result::Error>(person.cached_image_path)
        })
        .await
        .map_err(|error| match error {
            diesel::result::Error::NotFound => Status::NotFound,
            error => {
                log::error!(
                    "Failed to load metadata person {} image path: {}",
                    person_id,
                    error
                );
                Status::InternalServerError
            }
        })?
        .ok_or(Status::NotFound)?;
    let image_path = resolve_metadata_asset_db_path(&data_dir, &image_path);
    let expected_root = PathBuf::from(data_dir).join("metadata").join("people");
    if !image_path.starts_with(&expected_root) {
        log::warn!(
            "Refusing to serve metadata person image outside managed people cache: {:?}",
            image_path
        );
        return Err(Status::NotFound);
    }

    NamedFile::open(image_path)
        .await
        .map_err(|_| Status::NotFound)
}

/// Search a configured provider for metadata candidates for a media item.
#[openapi(tag = "Media")]
#[get("/api/v1/items/<item_id>/metadata/search?<query>&<providers>&<year>&<language>")]
pub async fn search_item_metadata(
    db: DbConn,
    item_id: i32,
    query: Option<String>,
    providers: Option<String>,
    year: Option<i32>,
    language: Option<String>,
) -> Result<Json<Vec<MetadataSearchResult>>, Status> {
    let settings = current_settings();
    let metadata_settings = settings.metadata.clone();
    let item = db
        .run(move |conn| get_media_item_summary(conn, item_id))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to load media item summary {} for metadata search: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?
        .ok_or(Status::NotFound)?;
    if !supports_manual_metadata_linking(&item) {
        return Err(Status::BadRequest);
    }

    let library_providers = load_item_library_metadata_providers(&db, item.library_id).await?;
    let requested_providers = parse_metadata_provider_selection(providers);
    let providers = if requested_providers.is_empty() {
        library_providers.clone()
    } else {
        requested_providers
    };
    let fallback_query = item.display_title.clone();
    let search_title = query
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_query);
    let effective_query = search_title.clone();
    let requested_language = language
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let library_languages = load_item_library_metadata_languages(&db, item.library_id).await?;
    let search_language = requested_language
        .or_else(|| library_languages.first().cloned())
        .unwrap_or_else(|| crate::metadata::DEFAULT_METADATA_LOCALE.to_string());
    let mut search_metadata_settings = metadata_settings.clone();
    let provider_statuses = list_provider_statuses(&metadata_settings)
        .into_iter()
        .map(|status| (status.id.clone(), status))
        .collect::<HashMap<_, _>>();

    let mut results = Vec::new();
    let mut saw_provider = false;
    let mut saw_success = false;
    for provider_id in providers {
        let Some(expected_media_type) = provider_search_media_type(provider_id.clone(), &item)
        else {
            continue;
        };
        let Some(status) = provider_statuses.get(&provider_id) else {
            continue;
        };
        saw_provider = true;
        if !library_providers.contains(&provider_id) || !status.configured || !status.implemented {
            continue;
        }

        if let Some(provider) = search_metadata_settings
            .providers
            .iter_mut()
            .find(|provider| provider.id == provider_id)
        {
            provider.language = crate::metadata::provider_locale_key(
                provider_id.clone(),
                &normalize_locale_key(&search_language),
            );
        }

        match search_provider(
            &search_metadata_settings,
            provider_id.clone(),
            &effective_query,
            Some(expected_media_type),
        )
        .await
        {
            Ok(provider_results) => {
                saw_success = true;
                results.extend(
                    provider_results
                        .into_iter()
                        .filter(|result| result.media_type == expected_media_type),
                );
            }
            Err(error) => {
                log::warn!(
                    "Metadata search failed for media item {} using provider {}: {}",
                    item_id,
                    provider_id.as_storage_value(),
                    error
                );
            }
        }
    }

    if !saw_provider {
        return Err(Status::NotFound);
    }
    if !saw_success && results.is_empty() {
        return Err(Status::ServiceUnavailable);
    }

    for result in &mut results {
        result.score = Some(metadata_search_score(&search_title, year, result));
    }
    results.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.title.cmp(&right.title))
    });

    Ok(Json(results))
}

/// Link a media item to a provider match and queue the fetched metadata snapshot.
#[openapi(tag = "Media")]
#[post(
    "/api/v1/items/<item_id>/metadata/link",
    format = "json",
    data = "<request>"
)]
pub async fn link_item_metadata(
    db: DbConn,
    item_id: i32,
    request: Json<LinkMetadataRequest>,
) -> Result<Json<ItemMetadataSummary>, Status> {
    let request = request.into_inner();
    let settings = current_settings();
    let item = db
        .run(move |conn| get_media_item_summary(conn, item_id))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to load media item summary {} for metadata link: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?
        .ok_or(Status::NotFound)?;
    if !supports_manual_metadata_linking(&item) {
        return Err(Status::BadRequest);
    }
    let library_providers = load_item_library_metadata_providers(&db, item.library_id).await?;
    let provider_status = list_provider_statuses(&settings.metadata)
        .into_iter()
        .find(|status| status.id == request.provider_id)
        .ok_or(Status::BadRequest)?;
    if !library_providers.contains(&request.provider_id)
        || !provider_status.configured
        || !provider_status.implemented
    {
        return Err(Status::BadRequest);
    }
    if Some(request.media_type.as_str())
        != provider_search_media_type(request.provider_id.clone(), &item)
    {
        return Err(Status::BadRequest);
    }

    let root_target = MetadataRefreshTarget {
        item_id: item.id,
        library_id: item.library_id,
        provider_id: request.provider_id.clone(),
        item_type: item.item_type.clone(),
        display_title: item.display_title.clone(),
        relative_path: item.relative_path.clone(),
        external_id: request.external_id.clone(),
        media_type: request.media_type.clone(),
        fetch_kind: MetadataRefreshFetchKind::Direct,
    };
    let Some((activity_id, queued_targets)) = register_metadata_refresh_activity(
        "item",
        "manual_item_link",
        format!("Link metadata for {}", item.display_title),
        Some(item.library_id),
        Some(item.id),
        vec![root_target.clone()],
    )
    .await
    else {
        return Ok(Json(
            load_metadata_summary_for_item(&db, item_id, request.provider_id.clone()).await?,
        ));
    };

    if let Err(status) = mark_metadata_refresh_targets_pending(&db, &queued_targets).await {
        cancel_metadata_refresh_activity(&activity_id).await;
        return Err(status);
    }

    let pending_summary =
        load_metadata_summary_for_item(&db, item_id, request.provider_id.clone()).await?;
    tokio::spawn(async move {
        mark_metadata_refresh_activity_running(&activity_id).await;

        let mut targets = queued_targets;
        match build_metadata_refresh_job(
            &db,
            &settings,
            &item,
            root_target.provider_id.clone(),
            &root_target.external_id,
            &root_target.media_type,
        )
        .await
        {
            Ok(refresh_job) => {
                let expanded_targets = flatten_metadata_refresh_job(&refresh_job);
                let additional_targets =
                    extend_metadata_refresh_activity(&activity_id, expanded_targets).await;
                if !additional_targets.is_empty() {
                    if let Err(status) =
                        mark_metadata_refresh_targets_pending(&db, &additional_targets).await
                    {
                        log::warn!(
                            "Failed to mark manual metadata link descendants pending for item {}: \
                             {:?}",
                            item_id,
                            status
                        );
                    }
                    targets.extend(additional_targets);
                }
            }
            Err(status) => {
                log::warn!(
                    "Failed to expand manual metadata link refresh targets for item {}: {:?}",
                    item_id,
                    status
                );
            }
        }

        for target in targets {
            let failed = execute_metadata_refresh_target(&db, &target, &settings).await;
            record_metadata_refresh_activity_progress(&activity_id, failed).await;
        }
        complete_metadata_refresh_activity(&activity_id).await;
    });

    Ok(Json(pending_summary))
}

/// Force-refresh the currently linked metadata snapshot for one media item.
#[openapi(tag = "Media")]
#[post("/api/v1/items/<item_id>/metadata/refresh")]
pub async fn refresh_item_metadata(
    db: DbConn,
    item_id: i32,
) -> Result<Json<ItemMetadataSummary>, Status> {
    let settings = current_settings();
    let item = db
        .run(move |conn| get_media_item_summary(conn, item_id))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to load media item summary {} for metadata refresh: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?
        .ok_or(Status::NotFound)?;
    if !supports_manual_metadata_linking(&item) {
        return Err(Status::BadRequest);
    }

    let link = db
        .run(move |conn| get_preferred_item_metadata_link(conn, item_id))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to load linked metadata for media item {} refresh: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?
        .ok_or(Status::NotFound)?;

    let provider_id =
        MetadataProviderId::from_storage_value(&link.provider_id).ok_or(Status::BadRequest)?;
    let media_type = link.media_type.clone().ok_or(Status::BadRequest)?;
    let external_id = link.external_id.clone();
    let refresh_job = build_metadata_refresh_job(
        &db,
        &settings,
        &item,
        provider_id.clone(),
        &external_id,
        &media_type,
    )
    .await?;
    let refresh_targets = flatten_metadata_refresh_job(&refresh_job);
    let Some((activity_id, queued_targets)) = register_metadata_refresh_activity(
        "item",
        "manual_item_refresh",
        format!("Refresh metadata for {}", item.display_title),
        Some(item.library_id),
        Some(item.id),
        refresh_targets,
    )
    .await
    else {
        return Ok(Json(
            load_metadata_summary_for_item(&db, item_id, provider_id.clone()).await?,
        ));
    };

    if let Err(status) = mark_metadata_refresh_targets_pending(&db, &queued_targets).await {
        cancel_metadata_refresh_activity(&activity_id).await;
        return Err(status);
    }

    let pending_summary = load_metadata_summary_for_item(&db, item_id, provider_id).await?;
    tokio::spawn(async move {
        mark_metadata_refresh_activity_running(&activity_id).await;
        for target in queued_targets {
            let failed = execute_metadata_refresh_target(&db, &target, &settings).await;
            record_metadata_refresh_activity_progress(&activity_id, failed).await;
        }
        complete_metadata_refresh_activity(&activity_id).await;
    });

    Ok(Json(pending_summary))
}

async fn run_manual_library_metadata_refresh(
    db: DbConn,
    settings: Settings,
    library_id: i32,
    library_name: String,
) -> (DbConn, Settings, String) {
    let automatch_activity = register_manual_library_automatch_activity(&db, library_id).await;
    let refresh_jobs = match load_library_refresh_jobs(&db, &settings, library_id).await {
        Ok(refresh_jobs) => refresh_jobs,
        Err(status) => {
            log::warn!(
                "Failed to prepare library {} metadata refresh jobs: {:?}",
                library_id,
                status
            );
            if let Some((automatch_activity_id, _)) = &automatch_activity {
                cancel_metadata_refresh_activity(automatch_activity_id).await;
            }
            return (db, settings, library_name);
        }
    };
    let refresh_targets = refresh_jobs
        .iter()
        .flat_map(flatten_metadata_refresh_job)
        .collect::<Vec<_>>();

    let Some((activity_id, queued_targets)) = register_metadata_refresh_activity(
        "library",
        "manual_library_refresh",
        format!("Refresh library metadata for {}", library_name),
        Some(library_id),
        None,
        refresh_targets,
    )
    .await
    else {
        run_automatic_movie_metadata_linking(
            &db,
            &settings,
            Some(library_id),
            true,
            automatch_activity,
        )
        .await;
        return (db, settings, library_name);
    };

    if let Err(status) = mark_metadata_refresh_targets_pending(&db, &queued_targets).await {
        cancel_metadata_refresh_activity(&activity_id).await;
        if let Some((automatch_activity_id, _)) = &automatch_activity {
            cancel_metadata_refresh_activity(automatch_activity_id).await;
        }
        log::warn!(
            "Failed to mark library {} metadata refresh targets pending: {:?}",
            library_id,
            status
        );
        return (db, settings, library_name);
    }

    mark_metadata_refresh_activity_running(&activity_id).await;
    for target in queued_targets {
        let failed = execute_metadata_refresh_target(&db, &target, &settings).await;
        record_metadata_refresh_activity_progress(&activity_id, failed).await;
    }
    complete_metadata_refresh_activity(&activity_id).await;
    run_automatic_movie_metadata_linking(
        &db,
        &settings,
        Some(library_id),
        true,
        automatch_activity,
    )
    .await;
    (db, settings, library_name)
}

/// Force-refresh every linked metadata item within one library.
#[openapi(tag = "Media")]
#[post("/api/v1/libraries/<library_id>/metadata/refresh")]
pub async fn refresh_library_metadata(
    db: DbConn,
    library_id: i32,
) -> Result<Json<PersistedLibrarySummary>, Status> {
    let settings = current_settings();
    let library_summary = load_library_summary(&db, library_id).await?;
    let library_name = library_summary.name.clone();
    if !begin_library_metadata_refresh(library_id).await {
        log::info!(
            "Skipping duplicate library {} metadata refresh request; a refresh is already running",
            library_id
        );
        return Ok(Json(library_summary));
    }

    tokio::spawn(async move {
        let refresh_task = tokio::spawn(run_manual_library_metadata_refresh(
            db,
            settings,
            library_id,
            library_name,
        ));
        let people_refresh = match refresh_task.await {
            Ok(resources) => Some(resources),
            Err(error) => {
                log::error!(
                    "Library {} metadata refresh worker stopped unexpectedly: {}",
                    library_id,
                    error
                );
                None
            }
        };
        finish_library_metadata_refresh(library_id).await;
        if let Some((people_db, people_settings, people_library_name)) = people_refresh {
            tokio::spawn(run_library_metadata_people_refresh(
                people_db,
                people_settings,
                library_id,
                people_library_name,
            ));
        }
    });

    Ok(Json(library_summary))
}

/// Serve poster or backdrop artwork for a linked media item, caching it locally on demand.
#[get("/api/v1/items/<item_id>/artwork?<kind>")]
pub async fn get_item_artwork(
    db: DbConn,
    user_guard: Option<UserGuard>,
    item_id: i32,
    kind: Option<String>,
) -> Result<NamedFile, Status> {
    let artwork_kind = ArtworkKind::from_query_value(kind.as_deref());
    let user_id = current_user_id(user_guard.as_ref())?;
    let data_dir = current_settings().general.data_dir;
    let data_dir_for_local_resolve = data_dir.clone();

    if artwork_kind != ArtworkKind::Logo {
        if let Some(local_path) = db
            .run(move |conn| {
                resolve_local_item_artwork_path(
                    conn,
                    item_id,
                    artwork_kind,
                    &data_dir_for_local_resolve,
                )
            })
            .await
            .map_err(|error| {
                log::error!(
                    "Failed to resolve local artwork for media item {}: {}",
                    item_id,
                    error
                );
                Status::InternalServerError
            })?
        {
            return NamedFile::open(local_path)
                .await
                .map_err(|_| Status::NotFound);
        }
    }

    let link = db
        .run(move |conn| {
            let languages = user_preferred_metadata_languages(conn, user_id)?;
            get_preferred_item_artwork_metadata_link_for_languages(
                conn,
                item_id,
                &languages,
                artwork_kind,
            )
        })
        .await
        .map_err(|error| {
            log::error!(
                "Failed to load linked metadata for media item {} artwork: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?
        .ok_or(Status::NotFound)?;

    let existing_cache = match artwork_kind {
        ArtworkKind::Poster => link.cached_artwork_path.clone(),
        ArtworkKind::Backdrop => link.cached_backdrop_path.clone(),
        ArtworkKind::Logo => link.cached_logo_path.clone(),
    };
    if let Some(existing_cache) = existing_cache {
        let provider_id = MetadataProviderId::from_storage_value(&link.provider_id)
            .unwrap_or(MetadataProviderId::Tmdb);
        let expected_item_asset_dir = managed_metadata_asset_dir(
            &data_dir,
            provider_id.clone(),
            &link.external_id,
            link.media_type.as_deref(),
            &link.locale_key,
        );
        let existing_path = resolve_metadata_asset_db_path(&data_dir, &existing_cache);
        let current_artwork_url = match artwork_kind {
            ArtworkKind::Poster => link.artwork_url.as_deref(),
            ArtworkKind::Backdrop => link.backdrop_url.as_deref(),
            ArtworkKind::Logo => link.logo_url.as_deref(),
        };
        let cache_key = match artwork_kind {
            ArtworkKind::Poster => format!("{}_poster", provider_id.as_storage_value()),
            ArtworkKind::Backdrop => format!("{}_backdrop", provider_id.as_storage_value()),
            ArtworkKind::Logo => format!("{}_logo", provider_id.as_storage_value()),
        };
        if let Some(url) = current_artwork_url {
            let expected_path =
                expected_artwork_cache_path(url, &expected_item_asset_dir, &cache_key);
            if existing_path.is_file() && existing_path == expected_path {
                return NamedFile::open(existing_path)
                    .await
                    .map_err(|_| Status::NotFound);
            }
        }
        log::warn!(
            "Ignoring stale cached artwork path for media item {}: {:?}",
            item_id,
            existing_path
        );
    }

    let provider_id = MetadataProviderId::from_storage_value(&link.provider_id)
        .unwrap_or(MetadataProviderId::Tmdb);
    let item_dir = managed_metadata_asset_dir(
        &data_dir,
        provider_id.clone(),
        &link.external_id,
        link.media_type.as_deref(),
        &link.locale_key,
    );
    let current_artwork_url = match artwork_kind {
        ArtworkKind::Poster => link.artwork_url.as_deref(),
        ArtworkKind::Backdrop => link.backdrop_url.as_deref(),
        ArtworkKind::Logo => link.logo_url.as_deref(),
    }
    .ok_or(Status::NotFound)?;
    let cache_key = match artwork_kind {
        ArtworkKind::Poster => format!("{}_poster", provider_id.as_storage_value()),
        ArtworkKind::Backdrop => format!("{}_backdrop", provider_id.as_storage_value()),
        ArtworkKind::Logo => format!("{}_logo", provider_id.as_storage_value()),
    };
    let cached_path = try_cache_item_artwork(current_artwork_url, &item_dir, &cache_key)
        .await
        .ok_or(Status::BadGateway)?;

    let link_id = link.id;
    let stored_path = cached_path.clone();
    let data_dir_for_update = data_dir.clone();
    db.run(move |conn| {
        update_cached_artwork_path(
            conn,
            link_id,
            artwork_kind,
            &stored_path,
            &data_dir_for_update,
        )
    })
    .await
    .map_err(|error| {
        log::error!(
            "Failed to persist cached artwork path for media item {}: {}",
            item_id,
            error
        );
        Status::InternalServerError
    })?;

    NamedFile::open(cached_path)
        .await
        .map_err(|_| Status::NotFound)
}

/// Serve a discovered theme-song asset for a media item.
#[get("/api/v1/items/<item_id>/theme")]
pub async fn get_item_theme(
    db: DbConn,
    item_id: i32,
) -> Result<NamedFile, Status> {
    let data_dir = current_settings().general.data_dir;
    let theme_path = db
        .run(move |conn| resolve_item_theme_song_path(conn, item_id, &data_dir))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to resolve theme song for media item {}: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?
        .ok_or(Status::NotFound)?;

    NamedFile::open(theme_path)
        .await
        .map_err(|_| Status::NotFound)
}

/// Serve a discovered subtitle sidecar for a media item.
#[get("/api/v1/items/<item_id>/subtitles/<track_index>")]
pub async fn get_item_subtitle(
    db: DbConn,
    item_id: i32,
    track_index: usize,
) -> Result<NamedFile, Status> {
    let data_dir = current_settings().general.data_dir;
    let subtitle_path = db
        .run(move |conn| resolve_item_subtitle_path(conn, item_id, track_index, &data_dir))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to resolve subtitle track {} for media item {}: {}",
                track_index,
                item_id,
                error
            );
            Status::InternalServerError
        })?
        .ok_or(Status::NotFound)?;

    NamedFile::open(subtitle_path)
        .await
        .map_err(|_| Status::NotFound)
}

/// Search browser-facing media items and metadata entities.
#[openapi(tag = "Media")]
#[get("/api/v1/search?<query>")]
pub async fn search_items(
    db: DbConn,
    user_guard: Option<UserGuard>,
    query: Option<&str>,
) -> Result<Json<Vec<MediaSearchResult>>, Status> {
    let query = query.unwrap_or_default().to_string();
    let user_id = current_user_id(user_guard.as_ref())?;
    let results = db
        .run(move |conn| {
            let languages = user_preferred_metadata_languages(conn, user_id)?;
            let normalized_query = query.trim().to_ascii_lowercase();
            let mut results = search_media_items_for_user_with_preferred_languages(
                conn, user_id, &query, None, &languages,
            )?
            .into_iter()
            .map(|item| MediaSearchResult::Item { item })
            .collect::<Vec<_>>();
            if !normalized_query.is_empty() {
                results.extend(
                    list_metadata_collection_summaries_with_preferred_languages(
                        conn,
                        None,
                        &languages,
                        &[],
                    )?
                    .into_iter()
                    .filter(|collection| collection_matches_query(collection, &normalized_query))
                    .map(|collection| MediaSearchResult::Collection { collection }),
                );
                results.extend(
                    search_metadata_people_with_preferred_languages(conn, &query, &languages)?
                        .into_iter()
                        .map(|person| MediaSearchResult::Person { person }),
                );
            }
            Ok::<_, diesel::result::Error>(results)
        })
        .await
        .map_err(|error| {
            log::error!("Failed to search media items: {}", error);
            Status::InternalServerError
        })?;

    Ok(Json(results))
}

fn collection_matches_query(
    collection: &MetadataCollectionSummary,
    query: &str,
) -> bool {
    collection.name.to_ascii_lowercase().contains(query)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search_result(
        provider_id: MetadataProviderId,
        title: &str,
    ) -> MetadataSearchResult {
        MetadataSearchResult {
            provider_id,
            external_id: "1".into(),
            media_type: "movie".into(),
            title: title.into(),
            overview: None,
            artwork_url: None,
            backdrop_url: None,
            release_year: None,
            score: None,
        }
    }

    #[test]
    fn metadata_search_score_penalizes_tmdb_case_mismatch() {
        let exact = search_result(MetadataProviderId::Tmdb, "The Matrix");
        let mismatched_case = search_result(MetadataProviderId::Tmdb, "THE MATRIX");

        assert!(
            metadata_search_score("The Matrix", None, &mismatched_case)
                < metadata_search_score("The Matrix", None, &exact)
        );
    }

    #[test]
    fn metadata_search_score_penalizes_case_mismatch_after_year_bonus() {
        let mismatched_case = MetadataSearchResult {
            release_year: Some(2021),
            ..search_result(MetadataProviderId::Tmdb, "Free Guy")
        };

        assert!(metadata_search_score("free guy", Some(2021), &mismatched_case) < 1.0);
    }

    #[test]
    fn metadata_search_score_penalizes_tvdb_case_mismatch() {
        let exact = search_result(MetadataProviderId::Tvdb, "The Matrix");
        let mismatched_case = search_result(MetadataProviderId::Tvdb, "THE MATRIX");

        assert!(
            metadata_search_score("The Matrix", None, &mismatched_case)
                < metadata_search_score("The Matrix", None, &exact)
        );
    }

    #[test]
    fn metadata_search_score_keeps_case_penalty_provider_scoped() {
        let tmdb_mismatched_case = search_result(MetadataProviderId::Tmdb, "THE MATRIX");
        let musicbrainz_mismatched_case =
            search_result(MetadataProviderId::MusicBrainz, "THE MATRIX");

        assert!(
            metadata_search_score("The Matrix", None, &tmdb_mismatched_case)
                < metadata_search_score("The Matrix", None, &musicbrainz_mismatched_case)
        );
    }

    #[test]
    fn collection_search_matches_title_only() {
        let collection = MetadataCollectionSummary {
            id: "collection:tmdb:1".into(),
            provider_id: MetadataProviderId::Tmdb,
            external_id: "james-bond-external-id".into(),
            name: "James Bond Collection".into(),
            overview: Some("A spy franchise overview.".into()),
            artwork_url: None,
            backdrop_url: None,
            theme_song_url: None,
            item_ids: vec![1],
            item_count: 1,
        };

        assert!(collection_matches_query(&collection, "james bond"));
        assert!(!collection_matches_query(&collection, "spy"));
        assert!(!collection_matches_query(&collection, "external"));
    }
}
