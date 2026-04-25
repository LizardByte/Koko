//! Media and system discovery routes.

// lib imports
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use once_cell::sync::Lazy;
use rocket::fs::NamedFile;
use rocket::get;
use rocket::http::Status;
use rocket::post;
use rocket::serde::Deserialize;
use rocket::serde::json::Json;
use rocket_okapi::openapi;
use schemars::JsonSchema;
use serde::Serialize;
use strsim::normalized_levenshtein;

// local imports
use crate::auth::UserGuard;
use crate::config::{MetadataProviderId, Settings, current_settings};
use crate::db::DbConn;
use crate::db::models::ItemMetadataLink;
use crate::globals;
use crate::media::{
    MediaHome, MediaItemDetail, MediaItemSummary, PersistedLibrarySummary,
    PersistedMediaFileSummary, PlaybackDecision, TranscodingCapability,
    get_item_theme_song_themerr_references, get_library_files, get_library_metadata_providers,
    get_media_home_with_preferred_languages, get_media_item, get_media_item_summary,
    get_media_item_with_preferred_languages, get_persisted_library_summaries,
    get_playback_decision, get_preferred_item_metadata_link, infer_episode_number,
    inspect_transcoding_capability, library_exists, list_automatic_metadata_candidates,
    list_library_settings, list_media_item_children, list_media_items,
    list_media_items_with_preferred_languages,
    mark_metadata_match_attempted, resolve_item_subtitle_path, resolve_item_theme_song_path,
    resolve_local_item_artwork_path, resolve_media_item_source_path,
    search_media_items_with_preferred_languages, sync_persisted_library_catalog,
    upsert_playback_progress,
};
use crate::metadata::{
    ArtworkKind, ItemMetadataSummary, MetadataProviderStatus, MetadataSearchResult,
    StoredMetadataSnapshot, expected_artwork_cache_path, fetch_provider_episode_metadata_snapshot,
    fetch_provider_metadata_snapshot, fetch_provider_metadata_snapshot_for_locale,
    fetch_provider_season_metadata_snapshot, fetch_themerr_youtube_theme_url_for_database,
    get_item_metadata_summaries, get_primary_item_metadata_link, get_stored_metadata_snapshot,
    guess_provider_movie_match, guess_provider_show_match, list_due_item_metadata_links,
    list_pending_item_metadata_links, list_provider_statuses, load_tvdb_show_descendant_targets,
    managed_metadata_asset_dir, normalize_locale_key, persist_item_metadata_assets,
    search_provider, set_item_metadata_refresh_state, update_cached_artwork_path,
    upsert_item_metadata_snapshot_with_refresh_interval,
};
use crate::utils::current_timestamp;

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

static BACKGROUND_LIBRARY_MONITOR_RUNNING: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));
static NEXT_SYSTEM_ACTIVITY_ID: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(1));
static ACTIVE_SYSTEM_ACTIVITIES: Lazy<
    tokio::sync::RwLock<HashMap<String, MetadataRefreshActivityRecord>>,
> = Lazy::new(|| tokio::sync::RwLock::new(HashMap::new()));
static ACTIVE_METADATA_REFRESH_ITEMS: Lazy<tokio::sync::RwLock<HashMap<i32, String>>> =
    Lazy::new(|| tokio::sync::RwLock::new(HashMap::new()));
const LIBRARY_MONITOR_INTERVAL_SECONDS: u64 = 20;

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

fn metadata_refresh_interval_seconds(settings: &Settings) -> Option<i64> {
    settings
        .metadata
        .refresh_interval_days
        .and_then(|days| i64::from(days).checked_mul(24 * 60 * 60))
}

async fn persist_snapshot_for_item(
    db: &DbConn,
    item_id: i32,
    snapshot: &StoredMetadataSnapshot,
    settings: &Settings,
) -> Result<ItemMetadataSummary, Status> {
    let (poster_path, backdrop_path, logo_path) =
        persist_item_metadata_assets(snapshot, item_id, &settings.general.data_dir)
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
        let poster_path_string = poster_path.to_string_lossy().to_string();
        db.run(move |conn| {
            update_cached_artwork_path(conn, summary_id, ArtworkKind::Poster, &poster_path)
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
        let backdrop_path_string = backdrop_path.to_string_lossy().to_string();
        db.run(move |conn| {
            update_cached_artwork_path(conn, summary_id, ArtworkKind::Backdrop, &backdrop_path)
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
        let logo_path_string = logo_path.to_string_lossy().to_string();
        db.run(move |conn| {
            update_cached_artwork_path(conn, summary_id, ArtworkKind::Logo, &logo_path)
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
    let casing_score = if query_title.eq_ignore_ascii_case(result_title)
        && query_title != result_title
    {
        -0.04
    } else if query_title == result_title {
        0.03
    } else {
        0.0
    };
    (title_score + year_score + casing_score).clamp(0.0, 1.0)
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

async fn register_metadata_refresh_activity(
    scope: &str,
    source: &str,
    label: String,
    library_id: Option<i32>,
    root_item_id: Option<i32>,
    targets: Vec<MetadataRefreshTarget>,
) -> Option<(String, Vec<MetadataRefreshTarget>)> {
    let mut item_registry = ACTIVE_METADATA_REFRESH_ITEMS.write().await;
    let queued_targets = targets
        .into_iter()
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
    if provider_id == MetadataProviderId::Tvdb {
        return load_tvdb_show_descendant_refresh_targets(
            db,
            settings,
            show_item_id,
            show_external_id,
        )
        .await;
    }

    let seasons = db
        .run(move |conn| list_media_item_children(conn, show_item_id))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to load show children for automatic metadata propagation on item {}: {}",
                show_item_id,
                error
            );
            Status::InternalServerError
        })?;

    let mut targets = Vec::new();
    for season in seasons
        .into_iter()
        .filter(|item| item.item_type == "season")
    {
        let Some(season_number) = season.season_number else {
            continue;
        };
        let season_id = season.id;
        targets.push(MetadataRefreshTarget {
            item_id: season_id,
            library_id: season.library_id,
            provider_id: provider_id.clone(),
            item_type: season.item_type.clone(),
            display_title: season.display_title.clone(),
            relative_path: season.relative_path.clone(),
            external_id: format!("tv:{show_external_id}:season:{season_number}"),
            media_type: "tv_season".into(),
            fetch_kind: MetadataRefreshFetchKind::TmdbShowSeason {
                show_external_id: show_external_id.to_string(),
                season_number,
            },
        });

        let episodes = db
            .run(move |conn| list_media_item_children(conn, season_id))
            .await
            .map_err(|error| {
                log::error!(
                    "Failed to load season children for automatic metadata propagation on item {}: {}",
                    season_id,
                    error
                );
                Status::InternalServerError
            })?;

        for episode in episodes
            .into_iter()
            .filter(|item| item.item_type == "episode")
        {
            let Some(episode_number) = episode.episode_number.or_else(|| {
                infer_episode_number(&episode.relative_path)
                    .or_else(|| infer_episode_number(&episode.display_title))
            }) else {
                log::warn!(
                    "Skipping TMDB episode metadata propagation for media item {} because no episode number could be inferred from {:?}",
                    episode.id,
                    episode.relative_path
                );
                continue;
            };
            targets.push(MetadataRefreshTarget {
                item_id: episode.id,
                library_id: episode.library_id,
                provider_id: provider_id.clone(),
                item_type: episode.item_type.clone(),
                display_title: episode.display_title.clone(),
                relative_path: episode.relative_path.clone(),
                external_id: format!(
                    "tv:{show_external_id}:season:{season_number}:episode:{episode_number}"
                ),
                media_type: "tv_episode".into(),
                fetch_kind: MetadataRefreshFetchKind::TmdbShowEpisode {
                    show_external_id: show_external_id.to_string(),
                    season_number,
                    episode_number,
                },
            });
        }
    }

    Ok(targets)
}

async fn load_tvdb_show_descendant_refresh_targets(
    db: &DbConn,
    settings: &crate::config::Settings,
    show_item_id: i32,
    show_external_id: &str,
) -> Result<Vec<MetadataRefreshTarget>, Status> {
    let lookup = load_tvdb_show_descendant_targets(&settings.metadata, show_external_id)
        .await
        .map_err(|error| {
            log::error!(
                "Failed to load TheTVDB descendant metadata for show item {}: {}",
                show_item_id,
                error
            );
            Status::ServiceUnavailable
        })?;

    let seasons = db
        .run(move |conn| list_media_item_children(conn, show_item_id))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to load show children for TheTVDB metadata propagation on item {}: {}",
                show_item_id,
                error
            );
            Status::InternalServerError
        })?;

    let mut targets = Vec::new();
    for season in seasons
        .into_iter()
        .filter(|item| item.item_type == "season")
    {
        let Some(season_number) = season.season_number else {
            continue;
        };
        let Some(first_episode) = lookup
            .iter()
            .find(|target| target.season_number == season_number)
        else {
            continue;
        };

        targets.push(MetadataRefreshTarget {
            item_id: season.id,
            library_id: season.library_id,
            provider_id: MetadataProviderId::Tvdb,
            item_type: season.item_type.clone(),
            display_title: season.display_title.clone(),
            relative_path: season.relative_path.clone(),
            external_id: format!(
                "series:{show_external_id}:season:{}",
                first_episode.season_external_id
            ),
            media_type: "season".into(),
            fetch_kind: MetadataRefreshFetchKind::TvdbSeason {
                show_external_id: show_external_id.to_string(),
                season_number,
                season_external_id: first_episode.season_external_id.clone(),
            },
        });

        let season_id = season.id;
        let episodes = db
            .run(move |conn| list_media_item_children(conn, season_id))
            .await
            .map_err(|error| {
                log::error!(
                    "Failed to load season children for TheTVDB metadata propagation on item {}: {}",
                    season_id,
                    error
                );
                Status::InternalServerError
            })?;

        for episode in episodes
            .into_iter()
            .filter(|item| item.item_type == "episode")
        {
            let Some(episode_number) = episode.episode_number.or_else(|| {
                infer_episode_number(&episode.relative_path)
                    .or_else(|| infer_episode_number(&episode.display_title))
            }) else {
                continue;
            };
            let Some(target) = lookup.iter().find(|target| {
                target.season_number == season_number && target.episode_number == episode_number
            }) else {
                continue;
            };
            targets.push(MetadataRefreshTarget {
                item_id: episode.id,
                library_id: episode.library_id,
                provider_id: MetadataProviderId::Tvdb,
                item_type: episode.item_type.clone(),
                display_title: episode.display_title.clone(),
                relative_path: episode.relative_path.clone(),
                external_id: format!(
                    "series:{show_external_id}:season:{season_number}:episode:{}",
                    target.episode_external_id
                ),
                media_type: "episode".into(),
                fetch_kind: MetadataRefreshFetchKind::TvdbEpisode {
                    show_external_id: show_external_id.to_string(),
                    season_number,
                    episode_number,
                    episode_external_id: target.episode_external_id.clone(),
                },
            });
        }
    }

    Ok(targets)
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

async fn execute_metadata_refresh_target(
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
    let snapshot_result = match &target.fetch_kind {
        MetadataRefreshFetchKind::Direct => {
            match db
                .run(all_user_preferred_metadata_languages)
                .await
                .map_err(|error| error.to_string())
            {
                Ok(languages) => {
                    let mut snapshots = Vec::new();
                    let mut error = None;
                    for language in languages {
                        match fetch_provider_metadata_snapshot_for_locale(
                            &settings.metadata,
                            target.provider_id.clone(),
                            &target.external_id,
                            &target.media_type,
                            &language,
                        )
                        .await
                        {
                            Ok(snapshot) => snapshots.push(snapshot),
                            Err(fetch_error) => {
                                error = Some(fetch_error);
                                break;
                            }
                        }
                    }
                    if let Some(error) = error { Err(error) } else { Ok(snapshots) }
                }
                Err(error) => Err(error),
            }
        }
        MetadataRefreshFetchKind::TmdbShowSeason {
            show_external_id,
            season_number,
        } => fetch_provider_season_metadata_snapshot(
            &settings.metadata,
            target.provider_id.clone(),
            show_external_id,
            *season_number,
            None,
        )
        .await
        .map(|snapshot| vec![snapshot]),
        MetadataRefreshFetchKind::TmdbShowEpisode {
            show_external_id,
            season_number,
            episode_number,
        } => fetch_provider_episode_metadata_snapshot(
            &settings.metadata,
            target.provider_id.clone(),
            show_external_id,
            *season_number,
            *episode_number,
            None,
        )
        .await
        .map(|snapshot| vec![snapshot]),
        MetadataRefreshFetchKind::TvdbSeason {
            show_external_id,
            season_number,
            season_external_id,
        } => fetch_provider_season_metadata_snapshot(
            &settings.metadata,
            target.provider_id.clone(),
            show_external_id,
            *season_number,
            Some(season_external_id),
        )
        .await
        .map(|snapshot| vec![snapshot]),
        MetadataRefreshFetchKind::TvdbEpisode {
            show_external_id,
            season_number,
            episode_number,
            episode_external_id,
        } => fetch_provider_episode_metadata_snapshot(
            &settings.metadata,
            target.provider_id.clone(),
            show_external_id,
            *season_number,
            *episode_number,
            Some(episode_external_id),
        )
        .await
        .map(|snapshot| vec![snapshot]),
    };

    match snapshot_result {
        Ok(snapshots) => {
            for snapshot in snapshots {
                if let Err(status) =
                    persist_snapshot_for_item(db, target.item_id, &snapshot, settings).await
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

async fn recover_pending_metadata_refreshes(
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

async fn build_metadata_refresh_job(
    db: &DbConn,
    settings: &crate::config::Settings,
    item: &MediaItemSummary,
    provider_id: MetadataProviderId,
    external_id: &str,
    media_type: &str,
) -> Result<MetadataRefreshJob, Status> {
    let descendants = if item.item_type == "show"
        && ((provider_id == MetadataProviderId::Tmdb && media_type == "tv")
            || (provider_id == MetadataProviderId::Tvdb && media_type == "series"))
    {
        load_show_descendant_refresh_targets(
            db,
            settings,
            item.id,
            provider_id.clone(),
            external_id,
        )
        .await?
    } else {
        Vec::new()
    };

    Ok(MetadataRefreshJob {
        root: MetadataRefreshTarget {
            item_id: item.id,
            library_id: item.library_id,
            provider_id,
            item_type: item.item_type.clone(),
            display_title: item.display_title.clone(),
            relative_path: item.relative_path.clone(),
            external_id: external_id.to_string(),
            media_type: media_type.to_string(),
            fetch_kind: MetadataRefreshFetchKind::Direct,
        },
        descendants,
    })
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

async fn persist_snapshot_tree_for_item(
    db: &DbConn,
    item_id: i32,
    snapshot: &StoredMetadataSnapshot,
    settings: &crate::config::Settings,
) -> Result<ItemMetadataSummary, Status> {
    let descendants = if (snapshot.provider_id == MetadataProviderId::Tmdb
        && snapshot.media_type.as_deref() == Some("tv"))
        || (snapshot.provider_id == MetadataProviderId::Tvdb
            && snapshot.media_type.as_deref() == Some("series"))
    {
        load_show_descendant_refresh_targets(
            db,
            settings,
            item_id,
            snapshot.provider_id.clone(),
            &snapshot.external_id,
        )
        .await?
    } else {
        Vec::new()
    };
    if !descendants.is_empty() {
        mark_metadata_refresh_targets_pending(db, &descendants).await?;
    }

    let summary = persist_snapshot_for_item(db, item_id, snapshot, settings).await?;
    if !descendants.is_empty() {
        execute_metadata_refresh_targets(db, &descendants, settings).await;
    }

    Ok(summary)
}

async fn fetch_snapshots_for_all_user_languages(
    db: &DbConn,
    settings: &crate::config::Settings,
    provider_id: MetadataProviderId,
    external_id: &str,
    media_type: &str,
) -> Result<Vec<StoredMetadataSnapshot>, Status> {
    let languages = db
        .run(all_user_preferred_metadata_languages)
        .await
        .map_err(|error| {
            log::error!("Failed to load preferred metadata languages: {}", error);
            Status::InternalServerError
        })?;
    let mut snapshots = Vec::new();
    for language in languages {
        snapshots.push(
            fetch_provider_metadata_snapshot_for_locale(
                &settings.metadata,
                provider_id.clone(),
                external_id,
                media_type,
                &language,
            )
            .await
            .map_err(|error| {
                log::error!(
                    "Failed to fetch {} metadata snapshot for locale {}: {}",
                    provider_id.as_storage_value(),
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
) -> Result<ItemMetadataSummary, Status> {
    let mut summary = None;
    for snapshot in snapshots {
        summary = Some(persist_snapshot_tree_for_item(db, item_id, snapshot, settings).await?);
    }
    summary.ok_or(Status::ServiceUnavailable)
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
) {
    let ready_providers = list_provider_statuses(&settings.metadata)
        .into_iter()
        .filter(|provider| provider.enabled && provider.configured && provider.implemented)
        .map(|provider| provider.id)
        .collect::<std::collections::HashSet<_>>();

    let candidates = match db
        .run(move |conn| list_automatic_metadata_candidates(conn, library_id, 8))
        .await
    {
        Ok(candidates) => candidates,
        Err(error) => {
            log::warn!("Failed to load automatic metadata candidates: {}", error);
            return;
        }
    };

    for candidate in candidates {
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
            }
            match fetch_provider_metadata_snapshot(
                &settings.metadata,
                provider_id.clone(),
                &result.external_id,
                &result.media_type,
            )
            .await
            {
                Ok(snapshot) => {
                    if let Err(status) =
                        persist_snapshot_tree_for_item(db, candidate.item_id, &snapshot, settings)
                            .await
                    {
                        log::warn!(
                            "Failed to persist automatic metadata snapshot for item {}: {:?}",
                            candidate.item_id,
                            status
                        );
                        if let Err(error) = db
                            .run({
                                let external_id = snapshot.external_id.clone();
                                let media_type = snapshot.media_type.clone();
                                let status_message = format!("{status:?}");
                                move |conn| {
                                    set_item_metadata_refresh_state(
                                        conn,
                                        candidate.item_id,
                                        snapshot.provider_id,
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
                        continue;
                    }
                }
                Err(error) => {
                    log::warn!(
                        "Failed to fetch automatic TMDB snapshot for item {}: {}",
                        candidate.item_id,
                        error
                    );
                    if let Err(persist_error) = db
                        .run({
                            let external_id = result.external_id.clone();
                            let media_type = result.media_type.clone();
                            let error_message = error.clone();
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
                    continue;
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
            }
        }
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
    use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl};

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

fn all_user_preferred_metadata_languages(
    conn: &mut rocket_sync_db_pools::diesel::SqliteConnection
) -> Result<Vec<String>, diesel::result::Error> {
    use crate::db::schema::users::dsl as users_dsl;
    use diesel::{QueryDsl, RunQueryDsl};

    let rows = users_dsl::users
        .select(users_dsl::preferred_metadata_languages_json)
        .load::<String>(conn)?;
    let mut languages = Vec::new();
    for row in rows {
        for language in crate::web::routes::user::parse_preferred_metadata_languages(&row) {
            let language = normalize_locale_key(&language);
            if !languages.contains(&language) {
                languages.push(language);
            }
        }
    }
    if languages.is_empty() {
        languages.push(crate::metadata::DEFAULT_METADATA_LOCALE.to_string());
    }
    Ok(languages)
}

async fn load_item_library_metadata_providers(
    db: &DbConn,
    settings: &Settings,
    library_id: i32,
) -> Result<Vec<MetadataProviderId>, Status> {
    let legacy_libraries = settings.media.libraries.clone();
    let providers = db
        .run({
            let legacy_libraries = legacy_libraries.clone();
            move |conn| get_library_metadata_providers(conn, library_id, &legacy_libraries)
        })
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
    settings: &Settings,
    library_id: i32,
) -> Result<PersistedLibrarySummary, Status> {
    let legacy_libraries = settings.media.libraries.clone();
    let libraries = db
        .run(move |conn| get_persisted_library_summaries(conn, &legacy_libraries))
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

pub fn start_library_monitor(db: DbConn) {
    if !BACKGROUND_LIBRARY_MONITOR_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        return;
    }

    tokio::spawn(async move {
        loop {
            let settings = current_settings();
            let legacy_libraries = settings.media.libraries.clone();
            let ffmpeg_settings = settings.ffmpeg.clone();
            let sync_result = db
                .run(move |conn| {
                    sync_persisted_library_catalog(conn, &legacy_libraries, &ffmpeg_settings)
                })
                .await;

            if let Err(error) = sync_result {
                log::error!(
                    "Failed to sync media library catalog in monitor loop: {}",
                    error
                );
            }

            recover_pending_metadata_refreshes(&db, &settings).await;
            run_due_metadata_refreshes(&db, &settings).await;
            tokio::time::sleep(tokio::time::Duration::from_secs(
                LIBRARY_MONITOR_INTERVAL_SECONDS,
            ))
            .await;
        }
    });
}

/// Return server bootstrap information for future browser and native clients.
#[openapi(tag = "Media")]
#[get("/api/v1/system/capabilities")]
pub async fn get_server_capabilities(
    db: DbConn
) -> Result<Json<ServerCapabilitiesResponse>, Status> {
    let settings = current_settings();
    let transcoding = inspect_transcoding_capability(&settings.ffmpeg);
    let legacy_libraries = settings.media.libraries.clone();
    let libraries_configured = db
        .run(move |conn| {
            list_library_settings(conn, &legacy_libraries).map(|libraries| libraries.len())
        })
        .await
        .map_err(|error| {
            log::error!("Failed to count persisted libraries: {}", error);
            Status::InternalServerError
        })?;
    start_library_monitor(db);

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
    let legacy_libraries = settings.media.libraries.clone();
    let ffmpeg_settings = settings.ffmpeg.clone();

    let exists = db
        .run(move |conn| library_exists(conn, library_id))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to inspect library {} before manual scan: {}",
                library_id,
                error
            );
            Status::InternalServerError
        })?;
    if !exists {
        return Err(Status::NotFound);
    }

    let summary = load_library_summary(&db, &settings, library_id).await?;
    tokio::spawn(async move {
        if let Err(error) = db
            .run(move |conn| {
                sync_persisted_library_catalog(conn, &legacy_libraries, &ffmpeg_settings)
            })
            .await
        {
            log::error!(
                "Failed to run manual library scan for library {}: {}",
                library_id,
                error
            );
        }
    });

    Ok(Json(summary))
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
pub async fn get_libraries(db: DbConn) -> Result<Json<Vec<PersistedLibrarySummary>>, Status> {
    let settings = current_settings();
    let legacy_libraries = settings.media.libraries.clone();

    let libraries = db
        .run(move |conn| get_persisted_library_summaries(conn, &legacy_libraries))
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

    let items = db
        .run(move |conn| {
            let languages = user_preferred_metadata_languages(conn, user_id)?;
            list_media_items_with_preferred_languages(conn, library_id, &languages)
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
    let data_dir = current_settings().general.data_dir;
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
    if item.theme_song_url.is_none() {
        let theme_references = db
            .run(move |conn| get_item_theme_song_themerr_references(conn, item_id))
            .await
            .map_err(|error| {
                log::error!(
                    "Failed to resolve ThemerrDB theme-song reference for media item {}: {}",
                    item_id,
                    error
                );
                Status::InternalServerError
            })?;

        for (media_type, database_id, external_id) in theme_references {
            match fetch_themerr_youtube_theme_url_for_database(
                &media_type,
                &database_id,
                &external_id,
            )
            .await
            {
                Ok(Some(url)) => {
                    item.theme_song_youtube_url = Some(url);
                    break;
                }
                Ok(None) => {}
                Err(error) => {
                    log::warn!(
                        "Failed to load ThemerrDB theme song for media item {} ({} {} {}): {}",
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

    Ok(Json(item))
}

/// Return direct-play versus transcode information for a media item.
#[openapi(tag = "Media")]
#[get("/api/v1/items/<item_id>/playback")]
pub async fn get_item_playback(
    db: DbConn,
    item_id: i32,
) -> Result<Json<PlaybackDecision>, Status> {
    let decision = db
        .run(move |conn| get_playback_decision(conn, item_id))
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
    item_id: i32,
) -> Result<NamedFile, Status> {
    let decision = db
        .run(move |conn| get_playback_decision(conn, item_id))
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

    NamedFile::open(source_path)
        .await
        .map_err(|_| Status::NotFound)
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
    item_id: i32,
) -> Result<Json<ItemMetadataResponse>, Status> {
    let data_dir = current_settings().general.data_dir;

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
        .run(move |conn| get_item_metadata_summaries(conn, item_id))
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

    let library_providers =
        load_item_library_metadata_providers(&db, &settings, item.library_id).await?;
    let requested_providers = parse_metadata_provider_selection(providers);
    let providers =
        if requested_providers.is_empty() { library_providers } else { requested_providers };
    let fallback_query = item.display_title.clone();
    let search_title = query
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_query);
    let mut effective_query = search_title.clone();
    if let Some(year) = year {
        effective_query = format!("{effective_query} {year}");
    }
    let requested_language = language
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
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
        if !status.enabled || !status.configured || !status.implemented {
            continue;
        }

        if let Some(language) = requested_language.as_deref() {
            if let Some(provider) = search_metadata_settings
                .providers
                .iter_mut()
                .find(|provider| provider.id == provider_id)
            {
                provider.language = crate::metadata::provider_locale_key(
                    provider_id.clone(),
                    &normalize_locale_key(language),
                );
            }
        }

        match search_provider(
            &search_metadata_settings,
            provider_id.clone(),
            &effective_query,
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

/// Link a media item to a provider match and persist the fetched metadata snapshot.
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
    let library_providers =
        load_item_library_metadata_providers(&db, &settings, item.library_id).await?;
    let provider_status = list_provider_statuses(&settings.metadata)
        .into_iter()
        .find(|status| status.id == request.provider_id)
        .ok_or(Status::BadRequest)?;
    if !provider_status.enabled || !provider_status.configured || !provider_status.implemented {
        return Err(Status::BadRequest);
    }
    let _library_default_provider = library_providers.contains(&request.provider_id);
    if Some(request.media_type.as_str())
        != provider_search_media_type(request.provider_id.clone(), &item)
    {
        return Err(Status::BadRequest);
    }

    let provider_id = request.provider_id.clone();
    let external_id = request.external_id.clone();
    let media_type = request.media_type.clone();
    let stored_snapshot = db
        .run(move |conn| {
            get_stored_metadata_snapshot(conn, provider_id, &external_id, Some(&media_type))
        })
        .await
        .map_err(|error| {
            log::error!(
                "Failed to inspect stored metadata snapshot for media item {}: {}",
                item_id,
                error
            );
            Status::InternalServerError
        })?;

    let snapshots = if let Some(stored_snapshot) = stored_snapshot {
        vec![stored_snapshot]
    } else {
        fetch_snapshots_for_all_user_languages(
            &db,
            &settings,
            request.provider_id,
            &request.external_id,
            &request.media_type,
        )
        .await?
    };

    let summary = persist_snapshot_tree_for_languages(&db, item_id, &snapshots, &settings).await?;

    Ok(Json(summary))
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

/// Force-refresh every linked metadata item within one library.
#[openapi(tag = "Media")]
#[post("/api/v1/libraries/<library_id>/metadata/refresh")]
pub async fn refresh_library_metadata(
    db: DbConn,
    library_id: i32,
) -> Result<Json<PersistedLibrarySummary>, Status> {
    let settings = current_settings();
    let library_summary = load_library_summary(&db, &settings, library_id).await?;

    let refresh_jobs = load_library_refresh_jobs(&db, &settings, library_id).await?;
    let refresh_targets = refresh_jobs
        .iter()
        .flat_map(flatten_metadata_refresh_job)
        .collect::<Vec<_>>();

    let Some((activity_id, queued_targets)) = register_metadata_refresh_activity(
        "library",
        "manual_library_refresh",
        format!("Refresh library metadata for {}", library_summary.name),
        Some(library_id),
        None,
        refresh_targets,
    )
    .await
    else {
        let summary = load_library_summary(&db, &settings, library_id).await?;
        tokio::spawn(async move {
            run_automatic_movie_metadata_linking(&db, &settings, Some(library_id)).await;
        });
        return Ok(Json(summary));
    };

    if let Err(status) = mark_metadata_refresh_targets_pending(&db, &queued_targets).await {
        cancel_metadata_refresh_activity(&activity_id).await;
        return Err(status);
    }

    let pending_summary = load_library_summary(&db, &settings, library_id).await?;
    tokio::spawn(async move {
        mark_metadata_refresh_activity_running(&activity_id).await;
        for target in queued_targets {
            let failed = execute_metadata_refresh_target(&db, &target, &settings).await;
            record_metadata_refresh_activity_progress(&activity_id, failed).await;
        }
        complete_metadata_refresh_activity(&activity_id).await;
        run_automatic_movie_metadata_linking(&db, &settings, Some(library_id)).await;
    });

    Ok(Json(pending_summary))
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
            load_item_artwork_metadata_link(conn, item_id, &languages)
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
        let existing_path = std::path::PathBuf::from(existing_cache);
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

    let snapshot = StoredMetadataSnapshot {
        provider_id: MetadataProviderId::from_storage_value(&link.provider_id)
            .unwrap_or(MetadataProviderId::Tmdb),
        external_id: link.external_id.clone(),
        media_type: link.media_type.clone(),
        title: link.title.clone(),
        overview: link.overview.clone(),
        artwork_url: link.artwork_url.clone(),
        backdrop_url: link.backdrop_url.clone(),
        release_year: link.release_year,
        locale_key: link.locale_key.clone(),
        provider_locale_key: link.provider_locale_key.clone(),
        provider_payload_json: link.provider_payload_json.clone(),
    };
    let data_dir = current_settings().general.data_dir;
    let (poster_path, backdrop_path, logo_path) =
        persist_item_metadata_assets(&snapshot, item_id, &data_dir)
            .await
            .map_err(|error| {
                log::error!(
                    "Failed to cache artwork for media item {}: {}",
                    item_id,
                    error
                );
                Status::BadGateway
            })?;
    let cached_path = match artwork_kind {
        ArtworkKind::Poster => poster_path,
        ArtworkKind::Backdrop => backdrop_path,
        ArtworkKind::Logo => logo_path,
    }
    .ok_or(Status::NotFound)?;

    let link_id = link.id;
    let stored_path = cached_path.clone();
    db.run(move |conn| update_cached_artwork_path(conn, link_id, artwork_kind, &stored_path))
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

fn load_item_artwork_metadata_link(
    conn: &mut diesel::SqliteConnection,
    item_id: i32,
    preferred_languages: &[String],
) -> Result<Option<ItemMetadataLink>, diesel::result::Error> {
    let mut current_item_id = Some(item_id);
    while let Some(current_id) = current_item_id {
        let Some(item) = get_media_item_summary(conn, current_id)? else {
            return Ok(None);
        };
        if let Some(link) = crate::metadata::get_preferred_item_metadata_link_for_languages(
            conn,
            current_id,
            preferred_languages,
        )? {
            return Ok(Some(link));
        }
        current_item_id = item.parent_id;
    }

    Ok(None)
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

/// Search browser-facing media items by title or path.
#[openapi(tag = "Media")]
#[get("/api/v1/search?<query>&<library_id>")]
pub async fn search_items(
    db: DbConn,
    user_guard: Option<UserGuard>,
    query: Option<&str>,
    library_id: Option<i32>,
) -> Result<Json<Vec<MediaItemSummary>>, Status> {
    let query = query.unwrap_or_default().to_string();
    let user_id = current_user_id(user_guard.as_ref())?;
    let items = db
        .run(move |conn| {
            let languages = user_preferred_metadata_languages(conn, user_id)?;
            search_media_items_with_preferred_languages(conn, &query, library_id, &languages)
        })
        .await
        .map_err(|error| {
            log::error!("Failed to search media items: {}", error);
            Status::InternalServerError
        })?;

    Ok(Json(items))
}
