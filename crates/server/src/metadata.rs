//! Metadata-provider registry and persistence helpers.

// standard imports
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

// lib imports
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl, SelectableHelper, SqliteConnection};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::StatusCode;
use reqwest::header::RETRY_AFTER;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use strsim::normalized_levenshtein;

// local imports
use crate::config::{
    MediaLibraryKind, MetadataProviderId, MetadataProviderSettings, MetadataSettings,
};
use crate::db::configure_sqlite_connection;
use crate::db::models::{
    ItemMetadataCollection, ItemMetadataLink, MediaItem, NewItemMetadataCollection,
    NewItemMetadataLink,
};

const TMDB_API_BASE: &str = "https://api.themoviedb.org/3";
const TMDB_IMAGE_BASE: &str = "https://image.tmdb.org/t/p";
const THEMERR_API_BASE: &str = "https://app.lizardbyte.dev/ThemerrDB";

static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent(format!("Koko/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("Failed to build shared HTTP client")
});
static TMDB_RATE_LIMITER: Lazy<tokio::sync::Mutex<Instant>> = Lazy::new(|| tokio::sync::Mutex::new(Instant::now()));

/// High-level descriptor for a metadata provider.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct MetadataProviderDescriptor {
    /// Stable identifier for the provider.
    pub id: MetadataProviderId,
    /// Human-friendly provider name.
    pub display_name: String,
    /// Short description of the provider's purpose.
    pub description: String,
    /// Supported media-library kinds.
    pub supported_kinds: Vec<MediaLibraryKind>,
    /// Whether an API key is required.
    pub requires_api_key: bool,
    /// Whether the provider is implemented in the current build.
    pub implemented: bool,
}

/// Runtime status for a metadata provider after applying user settings.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct MetadataProviderStatus {
    /// Stable identifier for the provider.
    pub id: MetadataProviderId,
    /// Human-friendly provider name.
    pub display_name: String,
    /// Short description of the provider's purpose.
    pub description: String,
    /// Supported media-library kinds.
    pub supported_kinds: Vec<MediaLibraryKind>,
    /// Whether an API key is required.
    pub requires_api_key: bool,
    /// Whether the provider is implemented in the current build.
    pub implemented: bool,
    /// Whether the provider is enabled in configuration.
    pub enabled: bool,
    /// Whether the provider has enough configuration to be used.
    pub configured: bool,
    /// Configured language preference for the provider.
    pub language: String,
}

/// Stored metadata match summary for one media item.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ItemMetadataSummary {
    /// Stable row identifier for the metadata link.
    pub id: i32,
    /// Provider identifier for the linked metadata.
    pub provider_id: MetadataProviderId,
    /// Provider-side external identifier.
    pub external_id: String,
    /// Linked title, if available.
    pub title: Option<String>,
    /// Linked overview, if available.
    pub overview: Option<String>,
    /// Poster or artwork URL, if available.
    pub artwork_url: Option<String>,
    /// Backdrop artwork URL, if available.
    pub backdrop_url: Option<String>,
    /// Release year, if available.
    pub release_year: Option<i32>,
    /// Provider-specific media type such as `movie` or `tv`.
    pub media_type: Option<String>,
    /// Current match state.
    pub match_state: String,
    /// Raw stored provider payload, when available.
    pub provider_payload_json: Option<String>,
    /// Cached poster path, when available.
    pub cached_artwork_path: Option<String>,
    /// Cached backdrop path, when available.
    pub cached_backdrop_path: Option<String>,
    /// Current refresh state such as fresh, pending, or error.
    pub refresh_state: String,
    /// Last successful refresh timestamp as Unix seconds, if available.
    pub last_refreshed_at: Option<i64>,
    /// Scheduled next refresh time as Unix seconds, if available.
    pub next_refresh_at: Option<i64>,
    /// Last refresh error, when available.
    pub refresh_error: Option<String>,
    /// Last update timestamp as Unix seconds, if available.
    pub updated_at: Option<i64>,
}

/// Search result returned by a metadata provider.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct MetadataSearchResult {
    /// Provider identifier.
    pub provider_id: MetadataProviderId,
    /// Provider-side external identifier.
    pub external_id: String,
    /// Provider-specific media type.
    pub media_type: String,
    /// Candidate title.
    pub title: String,
    /// Candidate overview, if available.
    pub overview: Option<String>,
    /// Candidate poster URL, if available.
    pub artwork_url: Option<String>,
    /// Candidate backdrop URL, if available.
    pub backdrop_url: Option<String>,
    /// Candidate release year, if available.
    pub release_year: Option<i32>,
}

/// Collection summary aggregated across linked metadata rows.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct MetadataCollectionSummary {
    /// Stable client-facing identifier.
    pub id: String,
    /// Provider identifier.
    pub provider_id: MetadataProviderId,
    /// Provider-side external identifier.
    pub external_id: String,
    /// Collection name.
    pub name: String,
    /// Collection overview when available.
    pub overview: Option<String>,
    /// Collection poster or artwork URL when available.
    pub artwork_url: Option<String>,
    /// Collection backdrop URL when available.
    pub backdrop_url: Option<String>,
    /// Root media item identifiers that belong to the collection.
    pub item_ids: Vec<i32>,
    /// Number of unique root items in the collection.
    pub item_count: usize,
}

/// Stored metadata snapshot fetched from a provider.
#[derive(Debug, Clone)]
pub struct StoredMetadataSnapshot {
    /// Provider identifier.
    pub provider_id: MetadataProviderId,
    /// Provider-side external identifier.
    pub external_id: String,
    /// Provider-specific media type.
    pub media_type: Option<String>,
    /// Canonical title.
    pub title: Option<String>,
    /// Canonical overview.
    pub overview: Option<String>,
    /// Poster URL.
    pub artwork_url: Option<String>,
    /// Backdrop URL.
    pub backdrop_url: Option<String>,
    /// Release year.
    pub release_year: Option<i32>,
    /// Raw provider payload.
    pub provider_payload_json: Option<String>,
}

/// Presentation fields derived from one stored metadata link.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkedMetadataPresentation {
    /// Tagline or short promotional line.
    pub tagline: Option<String>,
    /// Long-form description or overview.
    pub overview: Option<String>,
    /// Genre labels from the provider payload.
    pub genres: Vec<String>,
    /// Release year, when known.
    pub release_year: Option<i32>,
    /// Provider media type such as movie or tv.
    pub media_type: Option<String>,
    /// Whether poster artwork is available either locally or remotely.
    pub poster_available: bool,
    /// Whether backdrop artwork is available either locally or remotely.
    pub backdrop_available: bool,
    /// Human-friendly trailer title, when available.
    pub trailer_title: Option<String>,
    /// Browser-embeddable trailer URL, when available.
    pub trailer_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedMovieName {
    title: String,
    year: Option<i32>,
    tmdb_id: Option<String>,
}

static BRACED_TAG_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{([^}]*)}").unwrap());
static YEAR_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(19\d{2}|20\d{2}|21\d{2})\b").unwrap());
static SPLIT_SUFFIX_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\s*[-–]\s*(cd|disc|disk|dvd|part|pt)\s*\d+\s*$").unwrap()
});
static NOISE_TOKEN_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)\b(2160p|1080p|720p|480p|x264|x265|h264|h265|hevc|hdr|dv|webrip|web[- ]dl|bluray|brrip|dvdrip|remux|proper|repack|extended|unrated|criterion|aac|dts|truehd|atmos)\b",
    )
    .unwrap()
});

/// Provider contract for metadata implementations.
pub trait MetadataProvider {
    /// Return the provider descriptor.
    fn descriptor(&self) -> MetadataProviderDescriptor;
}

/// Registry of known metadata providers.
pub struct MetadataRegistry {
    providers: Vec<Box<dyn MetadataProvider + Send + Sync>>,
}

impl MetadataRegistry {
    /// Create a new registry containing the built-in providers.
    pub fn new() -> Self {
        Self {
            providers: vec![
                Box::new(TmdbMetadataProvider),
                Box::new(MusicBrainzMetadataProvider),
                Box::new(OpenLibraryMetadataProvider),
                Box::new(LocalNfoMetadataProvider),
            ],
        }
    }

    /// Return all built-in provider descriptors.
    pub fn descriptors(&self) -> Vec<MetadataProviderDescriptor> {
        self.providers.iter().map(|provider| provider.descriptor()).collect()
    }
}

impl Default for MetadataRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Return provider statuses after applying the current settings.
pub fn list_provider_statuses(settings: &MetadataSettings) -> Vec<MetadataProviderStatus> {
    let configured_settings: std::collections::HashMap<MetadataProviderId, MetadataProviderSettings> = settings
        .providers
        .iter()
        .cloned()
        .map(|provider| (provider.id.clone(), provider))
        .collect();

    MetadataRegistry::new()
        .descriptors()
        .into_iter()
        .map(|descriptor| {
            let setting = configured_settings.get(&descriptor.id).cloned();
            let enabled = setting.as_ref().map(|provider| provider.enabled).unwrap_or(false);
            let language = setting
                .as_ref()
                .map(|provider| provider.language.clone())
                .unwrap_or_else(|| "en-US".into());
            let configured = if descriptor.requires_api_key {
                setting
                    .and_then(|provider| provider.api_key)
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false)
            } else {
                true
            };

            MetadataProviderStatus {
                id: descriptor.id,
                display_name: descriptor.display_name,
                description: descriptor.description,
                supported_kinds: descriptor.supported_kinds,
                requires_api_key: descriptor.requires_api_key,
                implemented: descriptor.implemented,
                enabled,
                configured,
                language,
            }
        })
        .collect()
}

/// Return stored metadata links for one media item.
pub fn get_item_metadata_summaries(
    conn: &mut SqliteConnection,
    item_id: i32,
) -> Result<Vec<ItemMetadataSummary>, diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as metadata_links_dsl;

    let rows = metadata_links_dsl::item_metadata_links
        .filter(metadata_links_dsl::media_item_id.eq(item_id))
        .order(metadata_links_dsl::provider_id.asc())
        .select(ItemMetadataLink::as_select())
        .load::<ItemMetadataLink>(conn)?;

    Ok(rows.into_iter().map(to_item_metadata_summary).collect())
}

/// Search TMDB for metadata candidates using the current provider configuration.
pub async fn search_tmdb(
    settings: &MetadataSettings,
    query: &str,
) -> Result<Vec<MetadataSearchResult>, String> {
    let provider = tmdb_provider_settings(settings)?;
    let payload = tmdb_get_json::<TmdbSearchResponse>(
        &provider,
        "search/multi",
        vec![("query", query.to_string())],
        &format!("search query {:?}", query),
    )
    .await?;
    Ok(payload
        .results
        .into_iter()
        .filter_map(|item| {
            let media_type = item.media_type.unwrap_or_default();
            if media_type != "movie" && media_type != "tv" {
                return None;
            }

            let title = item.title.or(item.name)?;
            Some(MetadataSearchResult {
                provider_id: MetadataProviderId::Tmdb,
                external_id: item.id.to_string(),
                media_type,
                title,
                overview: item.overview,
                artwork_url: item.poster_path.map(|path| tmdb_image_url(&path, "w500")),
                backdrop_url: item.backdrop_path.map(|path| tmdb_image_url(&path, "w1280")),
                release_year: extract_release_year(item.release_date.or(item.first_air_date)),
            })
        })
        .collect())
}

/// Fetch and normalize a TMDB metadata snapshot for one provider item.
pub async fn fetch_tmdb_metadata_snapshot(
    settings: &MetadataSettings,
    external_id: &str,
    media_type: &str,
) -> Result<StoredMetadataSnapshot, String> {
    let provider = tmdb_provider_settings(settings)?;
    let payload = tmdb_get_text(
        &provider,
        &format!("{}/{}", media_type, external_id),
        vec![("append_to_response", "videos".to_string())],
        &format!("details lookup for {media_type}:{external_id}"),
    )
    .await?;
    let parsed: Value = serde_json::from_str(&payload).map_err(|error| error.to_string())?;
    let title = parsed
        .get("title")
        .or_else(|| parsed.get("name"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let overview = parsed
        .get("overview")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .filter(|value| !value.trim().is_empty());
    let artwork_url = parsed
        .get("poster_path")
        .and_then(Value::as_str)
        .map(|path| tmdb_image_url(path, "w500"));
    let backdrop_url = parsed
        .get("backdrop_path")
        .and_then(Value::as_str)
        .map(|path| tmdb_image_url(path, "w1280"));
    let release_year = parsed
        .get("release_date")
        .or_else(|| parsed.get("first_air_date"))
        .and_then(Value::as_str)
        .map(|value| value.to_string())
        .and_then(|value| extract_release_year(Some(value)));

    Ok(StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tmdb,
        external_id: external_id.to_string(),
        media_type: Some(media_type.to_string()),
        title,
        overview,
        artwork_url,
        backdrop_url,
        release_year,
        provider_payload_json: Some(payload),
    })
}

/// Guess the best TMDB movie match for one library item using filename cleanup and fuzzy scoring.
pub async fn guess_tmdb_movie_match(
    settings: &MetadataSettings,
    relative_path: &str,
    display_title: &str,
) -> Result<Option<MetadataSearchResult>, String> {
    let parsed = parse_movie_name(relative_path, display_title);
    if parsed.title.trim().is_empty() {
        return Ok(None);
    }

    if let Some(tmdb_id) = parsed.tmdb_id.clone() {
        let snapshot = fetch_tmdb_metadata_snapshot(settings, &tmdb_id, "movie").await?;
        return Ok(Some(MetadataSearchResult {
            provider_id: MetadataProviderId::Tmdb,
            external_id: tmdb_id,
            media_type: "movie".into(),
            title: snapshot.title.unwrap_or(parsed.title),
            overview: snapshot.overview,
            artwork_url: snapshot.artwork_url,
            backdrop_url: snapshot.backdrop_url,
            release_year: snapshot.release_year,
        }));
    }

    let mut best_result = None;
    let mut best_score = 0.0;
    for result in search_tmdb(settings, &parsed.title).await? {
        if result.media_type != "movie" {
            continue;
        }

        let score = movie_match_score(&parsed, &result);
        if score > best_score {
            best_score = score;
            best_result = Some(result);
        }
    }

    Ok((best_score >= 0.78).then_some(best_result).flatten())
}

/// Guess the best TMDB television match for one show item using the show title and path.
pub async fn guess_tmdb_show_match(
    settings: &MetadataSettings,
    relative_path: &str,
    display_title: &str,
) -> Result<Option<MetadataSearchResult>, String> {
    let query = show_search_query(relative_path, display_title);
    if query.trim().is_empty() {
        return Ok(None);
    }

    let mut best_result = None;
    let mut best_score = 0.0;
    for result in search_tmdb(settings, &query).await? {
        if result.media_type != "tv" {
            continue;
        }

        let score = normalized_levenshtein(
            &cleanup_movie_title(&query).to_ascii_lowercase(),
            &cleanup_movie_title(&result.title).to_ascii_lowercase(),
        );
        if score > best_score {
            best_score = score;
            best_result = Some(result);
        }
    }

    Ok((best_score >= 0.78).then_some(best_result).flatten())
}

/// Fetch TMDB metadata for one season of a linked show.
pub async fn fetch_tmdb_season_metadata_snapshot(
    settings: &MetadataSettings,
    show_external_id: &str,
    season_number: i32,
) -> Result<StoredMetadataSnapshot, String> {
    let provider = tmdb_provider_settings(settings)?;
    let payload = tmdb_get_text(
        &provider,
        &format!("tv/{}/season/{}", show_external_id, season_number),
        Vec::new(),
        &format!("season lookup for tv:{show_external_id}:season:{season_number}"),
    )
    .await?;
    let parsed: Value = serde_json::from_str(&payload).map_err(|error| error.to_string())?;

    Ok(StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tmdb,
        external_id: tmdb_season_external_id(show_external_id, season_number),
        media_type: Some("tv_season".into()),
        title: parsed.get("name").and_then(Value::as_str).map(ToOwned::to_owned),
        overview: parsed
            .get("overview")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        artwork_url: parsed
            .get("poster_path")
            .and_then(Value::as_str)
            .map(|path| tmdb_image_url(path, "w500")),
        backdrop_url: None,
        release_year: parsed
            .get("air_date")
            .and_then(Value::as_str)
            .map(|value| value.to_string())
            .and_then(|value| extract_release_year(Some(value))),
        provider_payload_json: Some(payload),
    })
}

/// Fetch TMDB metadata for one episode of a linked show.
pub async fn fetch_tmdb_episode_metadata_snapshot(
    settings: &MetadataSettings,
    show_external_id: &str,
    season_number: i32,
    episode_number: i32,
) -> Result<StoredMetadataSnapshot, String> {
    let provider = tmdb_provider_settings(settings)?;
    let payload = tmdb_get_text(
        &provider,
        &format!(
            "tv/{}/season/{}/episode/{}",
            show_external_id, season_number, episode_number
        ),
        Vec::new(),
        &format!(
            "episode lookup for tv:{show_external_id}:season:{season_number}:episode:{episode_number}"
        ),
    )
    .await?;
    let parsed: Value = serde_json::from_str(&payload).map_err(|error| error.to_string())?;

    Ok(StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tmdb,
        external_id: tmdb_episode_external_id(show_external_id, season_number, episode_number),
        media_type: Some("tv_episode".into()),
        title: parsed.get("name").and_then(Value::as_str).map(ToOwned::to_owned),
        overview: parsed
            .get("overview")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        artwork_url: parsed
            .get("still_path")
            .and_then(Value::as_str)
            .map(|path| tmdb_image_url(path, "w780")),
        backdrop_url: None,
        release_year: parsed
            .get("air_date")
            .and_then(Value::as_str)
            .map(|value| value.to_string())
            .and_then(|value| extract_release_year(Some(value))),
        provider_payload_json: Some(payload),
    })
}

/// Resolve a ThemerrDB YouTube theme-song URL for one linked TMDB movie or show.
pub async fn fetch_themerr_youtube_theme_url(
    tmdb_media_type: &str,
    external_id: &str,
) -> Result<Option<String>, String> {
    let Some(database_path) = themerr_database_path_for_tmdb_media_type(tmdb_media_type) else {
        return Ok(None);
    };
    let normalized_external_id = external_id.trim();
    if normalized_external_id.is_empty() {
        return Ok(None);
    }

    let response = reqwest::Client::new()
        .get(format!(
            "{}/{}/{}.json",
            THEMERR_API_BASE, database_path, normalized_external_id
        ))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(format!(
            "ThemerrDB lookup failed with status {}",
            response.status()
        ));
    }

    let payload = response.text().await.map_err(|error| error.to_string())?;
    Ok(parse_themerr_youtube_theme_url(&payload))
}

/// Upsert a stored metadata snapshot for one media item.
pub fn upsert_item_metadata_snapshot(
    conn: &mut SqliteConnection,
    item_id: i32,
    snapshot: &StoredMetadataSnapshot,
) -> Result<ItemMetadataSummary, diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as metadata_links_dsl;
    configure_sqlite_connection(conn)?;
    retry_sqlite_write(|| {
        let existing = metadata_links_dsl::item_metadata_links
            .filter(metadata_links_dsl::media_item_id.eq(item_id))
            .filter(metadata_links_dsl::provider_id.eq(snapshot.provider_id.as_storage_value()))
            .select(ItemMetadataLink::as_select())
            .first(conn)
            .optional()?;

        let payload = NewItemMetadataLink {
            media_item_id: item_id,
            provider_id: snapshot.provider_id.as_storage_value().to_string(),
            external_id: snapshot.external_id.clone(),
            title: snapshot.title.clone(),
            overview: snapshot.overview.clone(),
            tagline: snapshot
                .provider_payload_json
                .as_deref()
                .and_then(|payload| serde_json::from_str::<Value>(payload).ok())
                .and_then(|payload| payload.get("tagline").and_then(Value::as_str).map(str::to_string)),
            artwork_url: snapshot.artwork_url.clone(),
            backdrop_url: snapshot.backdrop_url.clone(),
            release_year: snapshot.release_year,
            media_type: snapshot.media_type.clone(),
            relation_kind: "primary".into(),
            match_state: "linked".into(),
            provider_payload_json: snapshot.provider_payload_json.clone(),
            cached_artwork_path: existing.as_ref().and_then(|row| row.cached_artwork_path.clone()),
            cached_backdrop_path: existing.as_ref().and_then(|row| row.cached_backdrop_path.clone()),
            refresh_state: "fresh".into(),
            refresh_interval_seconds: 7 * 24 * 60 * 60,
            last_refreshed_at: Some(current_timestamp()),
            next_refresh_at: Some(current_timestamp() + (7 * 24 * 60 * 60)),
            refresh_error: None,
            updated_at: Some(current_timestamp()),
        };

        if let Some(existing) = existing {
            diesel::update(metadata_links_dsl::item_metadata_links.filter(metadata_links_dsl::id.eq(existing.id)))
                .set(&payload)
                .execute(conn)?;
        } else {
            diesel::insert_into(metadata_links_dsl::item_metadata_links)
                .values(&payload)
                .execute(conn)?;
        }

        let row = metadata_links_dsl::item_metadata_links
            .filter(metadata_links_dsl::media_item_id.eq(item_id))
            .filter(metadata_links_dsl::provider_id.eq(snapshot.provider_id.as_storage_value()))
            .select(ItemMetadataLink::as_select())
            .first(conn)?;

        sync_item_metadata_collections(conn, row.id, snapshot)?;

        Ok(to_item_metadata_summary(row))
    })
}

/// Create or update one metadata-link refresh state for asynchronous work tracking.
pub fn set_item_metadata_refresh_state(
    conn: &mut SqliteConnection,
    item_id: i32,
    provider_id: MetadataProviderId,
    external_id: &str,
    media_type: Option<&str>,
    refresh_state: &str,
    refresh_error: Option<&str>,
) -> Result<ItemMetadataSummary, diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as metadata_links_dsl;
    configure_sqlite_connection(conn)?;
    retry_sqlite_write(|| {
        let existing = metadata_links_dsl::item_metadata_links
            .filter(metadata_links_dsl::media_item_id.eq(item_id))
            .filter(metadata_links_dsl::provider_id.eq(provider_id.as_storage_value()))
            .select(ItemMetadataLink::as_select())
            .first(conn)
            .optional()?;

        let payload = NewItemMetadataLink {
            media_item_id: item_id,
            provider_id: provider_id.as_storage_value().to_string(),
            external_id: external_id.to_string(),
            title: existing.as_ref().and_then(|row| row.title.clone()),
            overview: existing.as_ref().and_then(|row| row.overview.clone()),
            tagline: existing.as_ref().and_then(|row| row.tagline.clone()),
            artwork_url: existing.as_ref().and_then(|row| row.artwork_url.clone()),
            backdrop_url: existing.as_ref().and_then(|row| row.backdrop_url.clone()),
            release_year: existing.as_ref().and_then(|row| row.release_year),
            media_type: media_type
                .map(str::to_string)
                .or_else(|| existing.as_ref().and_then(|row| row.media_type.clone())),
            relation_kind: existing
                .as_ref()
                .map(|row| row.relation_kind.clone())
                .unwrap_or_else(|| "primary".into()),
            match_state: existing
                .as_ref()
                .map(|row| row.match_state.clone())
                .unwrap_or_else(|| "linked".into()),
            provider_payload_json: existing.as_ref().and_then(|row| row.provider_payload_json.clone()),
            cached_artwork_path: existing.as_ref().and_then(|row| row.cached_artwork_path.clone()),
            cached_backdrop_path: existing.as_ref().and_then(|row| row.cached_backdrop_path.clone()),
            refresh_state: refresh_state.to_string(),
            refresh_interval_seconds: existing
                .as_ref()
                .map(|row| row.refresh_interval_seconds)
                .unwrap_or(7 * 24 * 60 * 60),
            last_refreshed_at: existing.as_ref().and_then(|row| row.last_refreshed_at),
            next_refresh_at: existing.as_ref().and_then(|row| row.next_refresh_at),
            refresh_error: refresh_error.map(str::to_string),
            updated_at: Some(current_timestamp()),
        };

        if let Some(existing) = existing {
            diesel::update(metadata_links_dsl::item_metadata_links.filter(metadata_links_dsl::id.eq(existing.id)))
                .set(&payload)
                .execute(conn)?;
        } else {
            diesel::insert_into(metadata_links_dsl::item_metadata_links)
                .values(&payload)
                .execute(conn)?;
        }

        let row = metadata_links_dsl::item_metadata_links
            .filter(metadata_links_dsl::media_item_id.eq(item_id))
            .filter(metadata_links_dsl::provider_id.eq(provider_id.as_storage_value()))
            .select(ItemMetadataLink::as_select())
            .first(conn)?;

        Ok(to_item_metadata_summary(row))
    })
}

/// Return collection summaries derived from stored metadata for the requested library scope.
pub fn list_metadata_collection_summaries(
    conn: &mut SqliteConnection,
    library_id: Option<i32>,
) -> Result<Vec<MetadataCollectionSummary>, diesel::result::Error> {
    use crate::db::schema::item_metadata_collections::dsl as collection_dsl;
    use crate::db::schema::item_metadata_links::dsl as link_dsl;
    use crate::db::schema::media_items::dsl as media_items_dsl;

    let mut item_query = media_items_dsl::media_items.into_boxed();
    if let Some(library_id) = library_id {
        item_query = item_query.filter(media_items_dsl::library_id.eq(library_id));
    }
    let items = item_query.select(MediaItem::as_select()).load::<MediaItem>(conn)?;
    if items.is_empty() {
        return Ok(Vec::new());
    }

    let items_by_id = items
        .iter()
        .cloned()
        .map(|item| (item.id, item))
        .collect::<HashMap<_, _>>();
    let link_rows = link_dsl::item_metadata_links
        .filter(link_dsl::media_item_id.eq_any(items_by_id.keys().copied().collect::<Vec<_>>()))
        .select(ItemMetadataLink::as_select())
        .load::<ItemMetadataLink>(conn)?;
    if link_rows.is_empty() {
        return Ok(Vec::new());
    }

    let links_by_id = link_rows
        .into_iter()
        .map(|link| (link.id, link))
        .collect::<HashMap<_, _>>();
    let collection_rows = collection_dsl::item_metadata_collections
        .filter(collection_dsl::metadata_link_id.eq_any(links_by_id.keys().copied().collect::<Vec<_>>()))
        .select(ItemMetadataCollection::as_select())
        .load::<ItemMetadataCollection>(conn)?;

    let mut grouped = HashMap::<String, (ItemMetadataCollection, HashSet<i32>)>::new();
    for collection in collection_rows {
        let Some(link) = links_by_id.get(&collection.metadata_link_id) else {
            continue;
        };
        let Some(root_id) = root_media_item_id(link.media_item_id, &items_by_id) else {
            continue;
        };

        grouped
            .entry(format!("{}:{}", collection.provider_id, collection.external_id))
            .and_modify(|(_, item_ids)| {
                item_ids.insert(root_id);
            })
            .or_insert_with(|| {
                let mut item_ids = HashSet::new();
                item_ids.insert(root_id);
                (collection, item_ids)
            });
    }

    let mut summaries = grouped
        .into_values()
        .map(|(collection, item_ids)| {
            let mut item_ids = item_ids.into_iter().collect::<Vec<_>>();
            item_ids.sort_unstable();
            MetadataCollectionSummary {
                id: format!("{}:{}", collection.provider_id, collection.external_id),
                provider_id: metadata_provider_id_from_db(&collection.provider_id),
                external_id: collection.external_id,
                name: collection.name,
                overview: collection.overview,
                artwork_url: collection.artwork_url,
                backdrop_url: collection.backdrop_url,
                item_count: item_ids.len(),
                item_ids,
            }
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(summaries)
}

/// Return the first stored metadata link for a media item.
pub fn get_primary_item_metadata_link(
    conn: &mut SqliteConnection,
    item_id: i32,
) -> Result<Option<ItemMetadataLink>, diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as metadata_links_dsl;

    metadata_links_dsl::item_metadata_links
        .filter(metadata_links_dsl::media_item_id.eq(item_id))
        .order(metadata_links_dsl::updated_at.desc())
        .select(ItemMetadataLink::as_select())
        .first(conn)
        .optional()
}

/// Return an already stored metadata snapshot matching one provider item.
pub fn get_stored_metadata_snapshot(
    conn: &mut SqliteConnection,
    provider_id: MetadataProviderId,
    external_id: &str,
    media_type: Option<&str>,
) -> Result<Option<StoredMetadataSnapshot>, diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as metadata_links_dsl;

    let mut query = metadata_links_dsl::item_metadata_links
        .filter(metadata_links_dsl::provider_id.eq(provider_id.as_storage_value()))
        .filter(metadata_links_dsl::external_id.eq(external_id))
        .into_boxed();
    if let Some(media_type) = media_type {
        query = query.filter(metadata_links_dsl::media_type.eq(media_type));
    }

    let row = query
        .order(metadata_links_dsl::updated_at.desc())
        .select(ItemMetadataLink::as_select())
        .first(conn)
        .optional()?;

    Ok(row.and_then(stored_snapshot_from_link))
}

/// Extract presentation-ready metadata from a stored link payload.
pub fn presentation_from_metadata_link(link: &ItemMetadataLink) -> LinkedMetadataPresentation {
    let parsed_payload = link
        .provider_payload_json
        .as_deref()
        .and_then(|payload| serde_json::from_str::<Value>(payload).ok());

    let tagline = parsed_payload
        .as_ref()
        .and_then(|payload| payload.get("tagline"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let overview = parsed_payload
        .as_ref()
        .and_then(|payload| payload.get("overview"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| link.overview.clone());
    let genres = parsed_payload
        .as_ref()
        .and_then(|payload| payload.get("genres"))
        .and_then(Value::as_array)
        .map(|genres| {
            genres
                .iter()
                .filter_map(|genre| genre.get("name").and_then(Value::as_str))
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let release_year = parsed_payload
        .as_ref()
        .and_then(|payload| {
            payload
                .get("release_date")
                .or_else(|| payload.get("first_air_date"))
                .and_then(Value::as_str)
                .map(|value| value.to_string())
        })
        .and_then(|value| extract_release_year(Some(value)))
        .or(link.release_year);

    LinkedMetadataPresentation {
        tagline,
        overview,
        genres,
        release_year,
        media_type: link.media_type.clone(),
        poster_available: link.cached_artwork_path.is_some() || link.artwork_url.is_some(),
        backdrop_available: link.cached_backdrop_path.is_some() || link.backdrop_url.is_some(),
        trailer_title: parsed_payload
            .as_ref()
            .and_then(tmdb_trailer_entry)
            .and_then(|entry| entry.get("name"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        trailer_url: parsed_payload
            .as_ref()
            .and_then(tmdb_trailer_entry)
            .and_then(|entry| entry.get("site").and_then(Value::as_str).zip(entry.get("key").and_then(Value::as_str)))
            .and_then(|(site, key)| youtube_embed_url(site, key)),
    }
}

/// Persist stored metadata payload and cached artwork into the managed item asset structure.
pub async fn persist_item_metadata_assets(
    snapshot: &StoredMetadataSnapshot,
    item_id: i32,
    data_dir: &str,
) -> Result<(Option<PathBuf>, Option<PathBuf>), String> {
    let item_dir = managed_item_asset_dir(data_dir, item_id);
    fs::create_dir_all(&item_dir).map_err(|error| error.to_string())?;

    if let Some(payload_json) = &snapshot.provider_payload_json {
        let metadata_file_name = format!("{}.json", snapshot.provider_id.as_storage_value());
        fs::write(item_dir.join(metadata_file_name), payload_json).map_err(|error| error.to_string())?;
    }

    let poster_path = if let Some(url) = &snapshot.artwork_url {
        try_cache_item_artwork(
            url,
            &item_dir,
            &format!("{}_poster", snapshot.provider_id.as_storage_value()),
        )
        .await
    } else {
        None
    };
    let backdrop_path = if let Some(url) = &snapshot.backdrop_url {
        try_cache_item_artwork(
            url,
            &item_dir,
            &format!("{}_backdrop", snapshot.provider_id.as_storage_value()),
        )
        .await
    } else {
        None
    };

    Ok((poster_path, backdrop_path))
}

/// Persist a cached artwork path for a metadata link.
pub fn update_cached_artwork_path(
    conn: &mut SqliteConnection,
    link_id: i32,
    kind: ArtworkKind,
    cache_path: &Path,
) -> Result<(), diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as metadata_links_dsl;
    configure_sqlite_connection(conn)?;
    retry_sqlite_write(|| {
        match kind {
            ArtworkKind::Poster => {
                diesel::update(metadata_links_dsl::item_metadata_links.filter(metadata_links_dsl::id.eq(link_id)))
                    .set(metadata_links_dsl::cached_artwork_path.eq(cache_path.to_string_lossy().to_string()))
                    .execute(conn)?;
            }
            ArtworkKind::Backdrop => {
                diesel::update(metadata_links_dsl::item_metadata_links.filter(metadata_links_dsl::id.eq(link_id)))
                    .set(metadata_links_dsl::cached_backdrop_path.eq(cache_path.to_string_lossy().to_string()))
                    .execute(conn)?;
            }
        }

        Ok(())
    })
}

/// Poster or backdrop artwork kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtworkKind {
    /// Poster or cover art.
    Poster,
    /// Background or hero artwork.
    Backdrop,
}

impl ArtworkKind {
    /// Parse an artwork kind from a query parameter.
    pub fn from_query_value(value: Option<&str>) -> Self {
        match value.unwrap_or_default() {
            "backdrop" => ArtworkKind::Backdrop,
            _ => ArtworkKind::Poster,
        }
    }
}

/// Download and cache one artwork asset to disk.
pub async fn cache_artwork(
    url: &str,
    cache_dir: &Path,
    cache_key: &str,
) -> Result<PathBuf, String> {
    fs::create_dir_all(cache_dir).map_err(|error| error.to_string())?;

    let extension = Path::new(url)
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("jpg");
    let cache_path = cache_dir.join(format!("{}.{}", sanitize_cache_key(cache_key), extension));
    if cache_path.is_file() {
        return Ok(cache_path);
    }

    let bytes = reqwest::get(url)
        .await
        .map_err(|error| error.to_string())?
        .bytes()
        .await
        .map_err(|error| error.to_string())?;
    fs::write(&cache_path, bytes).map_err(|error| error.to_string())?;

    Ok(cache_path)
}

struct TmdbMetadataProvider;
struct MusicBrainzMetadataProvider;
struct OpenLibraryMetadataProvider;
struct LocalNfoMetadataProvider;

impl MetadataProvider for TmdbMetadataProvider {
    fn descriptor(&self) -> MetadataProviderDescriptor {
        MetadataProviderDescriptor {
            id: MetadataProviderId::Tmdb,
            display_name: "TheMovieDB".into(),
            description: "Primary movie and television metadata provider for Koko.".into(),
            supported_kinds: vec![MediaLibraryKind::Movies, MediaLibraryKind::Shows],
            requires_api_key: true,
            implemented: true,
        }
    }
}

impl MetadataProvider for MusicBrainzMetadataProvider {
    fn descriptor(&self) -> MetadataProviderDescriptor {
        MetadataProviderDescriptor {
            id: MetadataProviderId::MusicBrainz,
            display_name: "MusicBrainz".into(),
            description: "Planned music metadata provider for albums, artists, and tracks.".into(),
            supported_kinds: vec![MediaLibraryKind::Music],
            requires_api_key: false,
            implemented: false,
        }
    }
}

impl MetadataProvider for OpenLibraryMetadataProvider {
    fn descriptor(&self) -> MetadataProviderDescriptor {
        MetadataProviderDescriptor {
            id: MetadataProviderId::OpenLibrary,
            display_name: "Open Library".into(),
            description: "Planned book metadata provider for ebooks, PDFs, and comics.".into(),
            supported_kinds: vec![MediaLibraryKind::Books],
            requires_api_key: false,
            implemented: false,
        }
    }
}

impl MetadataProvider for LocalNfoMetadataProvider {
    fn descriptor(&self) -> MetadataProviderDescriptor {
        MetadataProviderDescriptor {
            id: MetadataProviderId::LocalNfo,
            display_name: "Local NFO".into(),
            description: "Planned sidecar metadata provider for locally curated libraries.".into(),
            supported_kinds: vec![
                MediaLibraryKind::Movies,
                MediaLibraryKind::Shows,
                MediaLibraryKind::Music,
                MediaLibraryKind::Books,
                MediaLibraryKind::HomeVideos,
            ],
            requires_api_key: false,
            implemented: false,
        }
    }
}

#[derive(Debug, Deserialize)]
struct TmdbSearchResponse {
    results: Vec<TmdbSearchItem>,
}

#[derive(Debug, Deserialize)]
struct TmdbSearchItem {
    id: i64,
    media_type: Option<String>,
    title: Option<String>,
    name: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    release_date: Option<String>,
    first_air_date: Option<String>,
}

fn tmdb_provider_settings(settings: &MetadataSettings) -> Result<MetadataProviderSettings, String> {
    let provider = settings
        .providers
        .iter()
        .find(|provider| provider.id == MetadataProviderId::Tmdb && provider.enabled)
        .cloned()
        .ok_or_else(|| "TMDB is not enabled in the current configuration.".to_string())?;

    let api_key = provider
        .api_key
        .clone()
        .unwrap_or_default()
        .trim()
        .to_string();
    if api_key.is_empty() {
        return Err("TMDB is enabled but no API key is configured.".into());
    }

    Ok(provider)
}

async fn wait_for_tmdb_rate_limit(provider: &MetadataProviderSettings) {
    let requests_per_second = provider.rate_limit_per_second.max(1);
    let interval = Duration::from_secs_f64(1.0 / f64::from(requests_per_second));
    let mut next_available_at = TMDB_RATE_LIMITER.lock().await;
    let now = Instant::now();
    if *next_available_at > now {
        tokio::time::sleep((*next_available_at).saturating_duration_since(now)).await;
    }
    let base = Instant::now();
    *next_available_at = base.checked_add(interval).unwrap_or(base);
}

async fn tmdb_get_text(
    provider: &MetadataProviderSettings,
    path: &str,
    mut query: Vec<(&'static str, String)>,
    context: &str,
) -> Result<String, String> {
    let api_key = provider.api_key.as_deref().unwrap_or_default().to_string();
    query.push(("api_key", api_key));
    query.push(("language", provider.language.clone()));

    let retry_attempts = usize::try_from(provider.retry_attempts).unwrap_or(0);
    let base_backoff = Duration::from_millis(u64::from(provider.retry_backoff_ms.max(1)));

    for attempt in 0..=retry_attempts {
        wait_for_tmdb_rate_limit(provider).await;
        let request_url = format!("{}/{}", TMDB_API_BASE, path.trim_start_matches('/'));
        let response = HTTP_CLIENT.get(&request_url).query(&query).send().await;

        match response {
            Ok(response) => {
                let status = response.status();
                let retry_after = response
                    .headers()
                    .get(RETRY_AFTER)
                    .and_then(|value| value.to_str().ok())
                    .and_then(parse_retry_after_seconds)
                    .map(Duration::from_secs);
                let payload = response.text().await.map_err(|error| error.to_string())?;
                if status.is_success() {
                    return Ok(payload);
                }

                let rate_limited = status == StatusCode::TOO_MANY_REQUESTS
                    || retry_after.is_some()
                    || payload.to_ascii_lowercase().contains("rate limit");
                let payload_snippet = format_payload_snippet(&payload);
                let retryable = rate_limited || status.is_server_error();
                if retryable && attempt < retry_attempts {
                    let attempt_number = attempt + 1;
                    let multiplier = 1_u32.checked_shl(u32::try_from(attempt).unwrap_or(0)).unwrap_or(u32::MAX);
                    let backoff = retry_after.unwrap_or_else(|| base_backoff.saturating_mul(multiplier));
                    log::warn!(
                        "TMDB request retry scheduled for {} after status {}{}{} (attempt {}/{} in {} ms)",
                        context,
                        status,
                        if rate_limited { " [rate limited]" } else { "" },
                        payload_snippet,
                        attempt_number,
                        retry_attempts + 1,
                        backoff.as_millis()
                    );
                    tokio::time::sleep(backoff).await;
                    continue;
                }

                return Err(format!(
                    "TMDB {} failed with status {}{}{}",
                    context,
                    status,
                    if rate_limited { " [rate limited]" } else { "" },
                    payload_snippet
                ));
            }
            Err(error) => {
                if attempt < retry_attempts {
                    let attempt_number = attempt + 1;
                    let multiplier = 1_u32.checked_shl(u32::try_from(attempt).unwrap_or(0)).unwrap_or(u32::MAX);
                    let backoff = base_backoff.saturating_mul(multiplier);
                    log::warn!(
                        "TMDB request retry scheduled for {} after transport error: {} (attempt {}/{} in {} ms)",
                        context,
                        error,
                        attempt_number,
                        retry_attempts + 1,
                        backoff.as_millis()
                    );
                    tokio::time::sleep(backoff).await;
                    continue;
                }

                return Err(format!("TMDB {} request failed: {}", context, error));
            }
        }
    }

    Err(format!("TMDB {} request failed after retries", context))
}

async fn tmdb_get_json<T>(
    provider: &MetadataProviderSettings,
    path: &str,
    query: Vec<(&'static str, String)>,
    context: &str,
) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let payload = tmdb_get_text(provider, path, query, context).await?;
    serde_json::from_str::<T>(&payload).map_err(|error| format!("TMDB {} returned invalid JSON: {}", context, error))
}

fn parse_retry_after_seconds(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok()
}

fn format_payload_snippet(payload: &str) -> String {
    let snippet = payload.split_whitespace().collect::<Vec<_>>().join(" ");
    if snippet.is_empty() {
        return String::new();
    }

    let truncated = if snippet.chars().count() > 180 {
        let prefix = snippet.chars().take(180).collect::<String>();
        format!("{}…", prefix)
    } else {
        snippet
    };
    format!(" | response: {}", truncated)
}

fn retry_sqlite_write<T, F>(mut operation: F) -> Result<T, diesel::result::Error>
where
    F: FnMut() -> Result<T, diesel::result::Error>,
{
    let mut attempts = 0;
    loop {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) if is_sqlite_locked_error(&error) && attempts < 4 => {
                attempts += 1;
                let backoff_ms = 25_u64.saturating_mul(2_u64.saturating_pow(attempts));
                std::thread::sleep(Duration::from_millis(backoff_ms));
            }
            Err(error) => return Err(error),
        }
    }
}

fn is_sqlite_locked_error(error: &diesel::result::Error) -> bool {
    match error {
        diesel::result::Error::DatabaseError(_, info) => info.message().to_ascii_lowercase().contains("database is locked"),
        _ => error.to_string().to_ascii_lowercase().contains("database is locked"),
    }
}

fn to_item_metadata_summary(link: ItemMetadataLink) -> ItemMetadataSummary {
    ItemMetadataSummary {
        id: link.id,
        provider_id: metadata_provider_id_from_db(&link.provider_id),
        external_id: link.external_id,
        title: link.title,
        overview: link.overview,
        artwork_url: link.artwork_url,
        backdrop_url: link.backdrop_url,
        release_year: link.release_year,
        media_type: link.media_type,
        match_state: link.match_state,
        provider_payload_json: link.provider_payload_json,
        cached_artwork_path: link.cached_artwork_path,
        cached_backdrop_path: link.cached_backdrop_path,
        refresh_state: link.refresh_state,
        last_refreshed_at: link.last_refreshed_at,
        next_refresh_at: link.next_refresh_at,
        refresh_error: link.refresh_error,
        updated_at: link.updated_at,
    }
}

fn tmdb_trailer_entry(payload: &Value) -> Option<&Value> {
    let results = payload
        .get("videos")
        .and_then(|videos| videos.get("results"))
        .and_then(Value::as_array)?;

    results
        .iter()
        .find(|entry| {
            entry.get("site").and_then(Value::as_str) == Some("YouTube")
                && entry.get("type").and_then(Value::as_str) == Some("Trailer")
                && entry.get("official").and_then(Value::as_bool).unwrap_or(false)
        })
        .or_else(|| {
            results.iter().find(|entry| {
                entry.get("site").and_then(Value::as_str) == Some("YouTube")
                    && matches!(entry.get("type").and_then(Value::as_str), Some("Trailer" | "Teaser"))
            })
        })
}

fn youtube_embed_url(site: &str, key: &str) -> Option<String> {
    if site != "YouTube" || key.trim().is_empty() {
        return None;
    }

    Some(format!("https://www.youtube.com/embed/{}?autoplay=1&rel=0", key.trim()))
}

fn themerr_database_path_for_tmdb_media_type(tmdb_media_type: &str) -> Option<&'static str> {
    match tmdb_media_type.trim() {
        "movie" => Some("movies/themoviedb"),
        "tv" => Some("tv_shows/themoviedb"),
        _ => None,
    }
}

fn parse_themerr_youtube_theme_url(payload_json: &str) -> Option<String> {
    serde_json::from_str::<Value>(payload_json)
        .ok()?
        .get("youtube_theme_url")?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn stored_snapshot_from_link(link: ItemMetadataLink) -> Option<StoredMetadataSnapshot> {
    link.provider_payload_json.as_ref()?;

    Some(StoredMetadataSnapshot {
        provider_id: metadata_provider_id_from_db(&link.provider_id),
        external_id: link.external_id,
        media_type: link.media_type,
        title: link.title,
        overview: link.overview,
        artwork_url: link.artwork_url,
        backdrop_url: link.backdrop_url,
        release_year: link.release_year,
        provider_payload_json: link.provider_payload_json,
    })
}

fn managed_item_asset_dir(data_dir: &str, item_id: i32) -> PathBuf {
    let item_hex = format!("{:08x}", item_id.max(0));
    let shard = &item_hex[0..2];
    Path::new(data_dir).join("item_assets").join(shard).join(item_hex)
}

async fn try_cache_item_artwork(
    url: &str,
    item_dir: &Path,
    cache_key: &str,
) -> Option<PathBuf> {
    match cache_artwork(url, item_dir, cache_key).await {
        Ok(path) => Some(path),
        Err(error) => {
            log::warn!("Failed to cache managed artwork asset from {}: {}", url, error);
            None
        }
    }
}

fn tmdb_image_url(path: &str, size: &str) -> String {
    format!("{}/{}/{}", TMDB_IMAGE_BASE, size, path.trim_start_matches('/'))
}

fn tmdb_season_external_id(show_external_id: &str, season_number: i32) -> String {
    format!("tv:{show_external_id}:season:{season_number}")
}

fn tmdb_episode_external_id(show_external_id: &str, season_number: i32, episode_number: i32) -> String {
    format!("tv:{show_external_id}:season:{season_number}:episode:{episode_number}")
}

fn extract_release_year(value: Option<String>) -> Option<i32> {
    value
        .as_deref()
        .and_then(|value| value.split('-').next())
        .and_then(|value| value.parse::<i32>().ok())
}

fn parse_movie_name(relative_path: &str, display_title: &str) -> ParsedMovieName {
    let relative_path = relative_path.replace('\\', "/");
    let path = Path::new(&relative_path);
    let file_stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(display_title);
    let parent_name = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    let preferred_source = if parent_name.eq_ignore_ascii_case(file_stem) || YEAR_REGEX.is_match(parent_name) {
        parent_name
    } else {
        file_stem
    };

    let tmdb_id = BRACED_TAG_REGEX
        .captures_iter(preferred_source)
        .chain(BRACED_TAG_REGEX.captures_iter(file_stem))
        .find_map(|captures| {
            let value = captures.get(1)?.as_str().trim();
            value
                .strip_prefix("tmdb-")
                .map(|id| id.trim().to_string())
                .filter(|id| !id.is_empty())
        });
    let year = YEAR_REGEX
        .captures(preferred_source)
        .or_else(|| YEAR_REGEX.captures(file_stem))
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse::<i32>().ok());

    let cleaned = cleanup_movie_title(preferred_source);
    let fallback = cleanup_movie_title(display_title);
    ParsedMovieName {
        title: if cleaned.is_empty() { fallback } else { cleaned },
        year,
        tmdb_id,
    }
}

fn show_search_query(relative_path: &str, display_title: &str) -> String {
    let normalized_path = relative_path.replace('\\', "/");
    let first_segment = normalized_path
        .split('/')
        .find(|segment| !segment.trim().is_empty())
        .unwrap_or_default()
        .to_string();
    let folder_name = Path::new(&normalized_path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();

    [display_title.to_string(), first_segment, folder_name]
        .into_iter()
        .map(|value| cleanup_movie_title(&value))
        .find(|value| !value.trim().is_empty())
        .unwrap_or_default()
}

fn cleanup_movie_title(value: &str) -> String {
    let without_tags = BRACED_TAG_REGEX.replace_all(value, " ");
    let without_split_suffix = SPLIT_SUFFIX_REGEX.replace(&without_tags, " ");
    let mut normalized = without_split_suffix.replace(['.', '_'], " ");
    if let Some(year_match) = YEAR_REGEX.find(&normalized) {
        normalized = normalized[..year_match.start()].to_string();
    }
    normalized = NOISE_TOKEN_REGEX.replace_all(&normalized, " ").to_string();

    normalized
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|character: char| !character.is_ascii_alphanumeric())
        .to_string()
}

fn movie_match_score(parsed: &ParsedMovieName, result: &MetadataSearchResult) -> f64 {
    let candidate_title = cleanup_movie_title(&result.title);
    if candidate_title.is_empty() || parsed.title.is_empty() {
        return 0.0;
    }

    let mut score = normalized_levenshtein(
        &parsed.title.to_ascii_lowercase(),
        &candidate_title.to_ascii_lowercase(),
    );
    if let Some(expected_year) = parsed.year {
        match result.release_year {
            Some(candidate_year) if candidate_year == expected_year => {
                score += 0.18;
            }
            Some(candidate_year) if (candidate_year - expected_year).abs() == 1 => {
                score += 0.05;
            }
            Some(_) => {
                score -= 0.2;
            }
            None => {
                score -= 0.04;
            }
        }
    }

    score.clamp(0.0, 1.0)
}

fn sanitize_cache_key(value: &str) -> String {
    value
        .chars()
        .map(|character| if character.is_ascii_alphanumeric() { character } else { '_' })
        .collect()
}

fn root_media_item_id(item_id: i32, items_by_id: &HashMap<i32, MediaItem>) -> Option<i32> {
    let mut current_id = item_id;
    let mut seen = HashSet::new();

    loop {
        let item = items_by_id.get(&current_id)?;
        let Some(parent_id) = item.parent_id else {
            return Some(item.id);
        };
        if !seen.insert(parent_id) {
            return Some(item.id);
        }
        current_id = parent_id;
    }
}

fn sync_item_metadata_collections(
    conn: &mut SqliteConnection,
    metadata_link_id: i32,
    snapshot: &StoredMetadataSnapshot,
) -> Result<(), diesel::result::Error> {
    use crate::db::schema::item_metadata_collections::dsl as collection_dsl;

    diesel::delete(
        collection_dsl::item_metadata_collections
            .filter(collection_dsl::metadata_link_id.eq(metadata_link_id)),
    )
    .execute(conn)?;

    let Some(collection) = snapshot
        .provider_payload_json
        .as_deref()
        .and_then(parse_tmdb_collection_payload)
    else {
        return Ok(());
    };

    diesel::insert_into(collection_dsl::item_metadata_collections)
        .values(&NewItemMetadataCollection {
            metadata_link_id,
            provider_id: snapshot.provider_id.as_storage_value().to_string(),
            external_id: collection.external_id,
            name: collection.name,
            overview: collection.overview,
            artwork_url: collection.artwork_url,
            backdrop_url: collection.backdrop_url,
            provider_payload_json: collection.provider_payload_json,
            updated_at: Some(current_timestamp()),
        })
        .execute(conn)?;

    Ok(())
}

fn parse_tmdb_collection_payload(payload_json: &str) -> Option<ParsedTmdbCollection> {
    let payload = serde_json::from_str::<Value>(payload_json).ok()?;
    let collection = payload.get("belongs_to_collection")?;
    let external_id = collection.get("id")?.as_i64()?.to_string();
    let name = collection.get("name")?.as_str()?.trim().to_string();
    if name.is_empty() {
        return None;
    }

    Some(ParsedTmdbCollection {
        external_id,
        name,
        overview: collection
            .get("overview")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        artwork_url: collection
            .get("poster_path")
            .and_then(Value::as_str)
            .map(|path| tmdb_image_url(path, "w500")),
        backdrop_url: collection
            .get("backdrop_path")
            .and_then(Value::as_str)
            .map(|path| tmdb_image_url(path, "w1280")),
        provider_payload_json: Some(collection.to_string()),
    })
}

#[derive(Debug, Clone)]
struct ParsedTmdbCollection {
    external_id: String,
    name: String,
    overview: Option<String>,
    artwork_url: Option<String>,
    backdrop_url: Option<String>,
    provider_payload_json: Option<String>,
}

fn current_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or_default()
}

fn metadata_provider_id_from_db(value: &str) -> MetadataProviderId {
    MetadataProviderId::from_storage_value(value).unwrap_or_else(|| {
        log::warn!("Ignoring unexpected stored metadata provider id: {}", value);
        MetadataProviderId::Tmdb
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_movie_title_strips_tags_and_noise() {
        assert_eq!(
            cleanup_movie_title("Blade.Runner.1982.{edition-Final Cut}.1080p.BluRay.x264"),
            "Blade Runner"
        );
        assert_eq!(
            parse_movie_name(
                "Blade Runner (1982) {tmdb-78}/Blade Runner (1982) {edition-Final Cut}.mkv",
                "Blade Runner (1982) {edition-Final Cut}"
            ),
            ParsedMovieName {
                title: "Blade Runner".into(),
                year: Some(1982),
                tmdb_id: Some("78".into()),
            }
        );
    }

    #[test]
    fn movie_match_score_prefers_matching_year() {
        let parsed = ParsedMovieName {
            title: "The Matrix".into(),
            year: Some(1999),
            tmdb_id: None,
        };
        let matching_year = MetadataSearchResult {
            provider_id: MetadataProviderId::Tmdb,
            external_id: "603".into(),
            media_type: "movie".into(),
            title: "The Matrix".into(),
            overview: None,
            artwork_url: None,
            backdrop_url: None,
            release_year: Some(1999),
        };
        let wrong_year = MetadataSearchResult {
            release_year: Some(2003),
            ..matching_year.clone()
        };

        assert!(movie_match_score(&parsed, &matching_year) > movie_match_score(&parsed, &wrong_year));
    }

    #[test]
    fn parse_themerr_youtube_theme_url_extracts_watch_url() {
        let payload = serde_json::json!({
            "id": 603,
            "title": "The Matrix",
            "youtube_theme_url": "https://www.youtube.com/watch?v=SLBACEP6LsI"
        })
        .to_string();

        assert_eq!(
            parse_themerr_youtube_theme_url(&payload).as_deref(),
            Some("https://www.youtube.com/watch?v=SLBACEP6LsI")
        );
    }

    #[test]
    fn parse_themerr_youtube_theme_url_rejects_missing_url() {
        let payload = serde_json::json!({
            "id": 1399,
            "name": "Game of Thrones"
        })
        .to_string();

        assert_eq!(parse_themerr_youtube_theme_url(&payload), None);
    }
}

