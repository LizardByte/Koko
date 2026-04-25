//! Metadata-provider registry and persistence helpers.

// standard imports
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

// lib imports
use diesel::{
    ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl, SelectableHelper, SqliteConnection,
};
use once_cell::sync::Lazy;
use regex::Regex;
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use strsim::normalized_levenshtein;

mod providers;

// local imports
use crate::config::{
    MediaLibraryKind, MetadataProviderId, MetadataProviderSettings, MetadataSettings,
};
use crate::db::configure_sqlite_connection;
use crate::db::models::{
    ItemMetadataCollection, ItemMetadataLink, MediaItem, NewItemMetadataCollection,
    NewItemMetadataLink,
};
use crate::utils::current_timestamp;

const TMDB_IMAGE_BASE: &str = "https://image.tmdb.org/t/p";
const TVDB_API_BASE: &str = "https://api4.thetvdb.com/v4";
const THEMERR_API_BASE: &str = "https://app.lizardbyte.dev/ThemerrDB";
const DEFAULT_METADATA_REFRESH_INTERVAL_SECONDS: i64 = 30 * 24 * 60 * 60;
/// Default Koko metadata locale used when no user preference is available.
pub const DEFAULT_METADATA_LOCALE: &str = "en-US";

static TVDB_RATE_LIMITER: Lazy<tokio::sync::Mutex<Instant>> =
    Lazy::new(|| tokio::sync::Mutex::new(Instant::now()));
static TVDB_AUTH_TOKEN: Lazy<tokio::sync::Mutex<Option<TvdbCachedToken>>> =
    Lazy::new(|| tokio::sync::Mutex::new(None));

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
    /// Provider attribution text for UI display.
    pub attribution_text: String,
    /// Provider attribution link.
    pub attribution_url: String,
    /// Provider logo suitable for light backgrounds.
    pub logo_light_url: Option<String>,
    /// Provider logo suitable for dark backgrounds.
    pub logo_dark_url: Option<String>,
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
    /// Provider attribution text for UI display.
    pub attribution_text: String,
    /// Provider attribution link.
    pub attribution_url: String,
    /// Provider logo suitable for light backgrounds.
    pub logo_light_url: Option<String>,
    /// Provider logo suitable for dark backgrounds.
    pub logo_dark_url: Option<String>,
}

/// Stored metadata match summary for one media item.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
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
    /// Provider-supplied title logo URL, when available.
    pub logo_url: Option<String>,
    /// Cached title logo path, when available.
    pub cached_logo_path: Option<String>,
    /// Provider genre labels stored directly for querying and UI use.
    pub genres: Vec<String>,
    /// Provider-supplied user/community rating, when available.
    pub rating: Option<f32>,
    /// Provider-supplied content rating such as PG-13 or TV-MA, when available.
    pub content_rating: Option<String>,
    /// Human-friendly trailer title, when available.
    pub trailer_title: Option<String>,
    /// Browser-embeddable trailer URL, when available.
    pub trailer_url: Option<String>,
    /// Koko locale key for this stored metadata row.
    pub locale_key: String,
    /// Provider-specific locale key used to fetch this row.
    pub provider_locale_key: Option<String>,
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
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
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
    /// Match score from 0.0 to 1.0, when Koko can compute one.
    pub score: Option<f64>,
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

/// Koko's provider-neutral metadata item kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataItemKind {
    /// A feature-length movie.
    Movie,
    /// A television or episodic series.
    Show,
    /// A season within a show.
    Season,
    /// An episode within a season.
    Episode,
    /// A provider collection or list that groups items.
    Collection,
    /// A person, actor, creator, or crew member.
    Person,
    /// A production or distribution company.
    Company,
    /// A provider award record.
    Award,
    /// A generic metadata item when no narrower Koko kind applies.
    Item,
}

impl MetadataItemKind {
    fn asset_directory(self) -> &'static str {
        match self {
            MetadataItemKind::Movie => "movies",
            MetadataItemKind::Show => "shows",
            MetadataItemKind::Season => "seasons",
            MetadataItemKind::Episode => "episodes",
            MetadataItemKind::Collection => "collections",
            MetadataItemKind::Person => "people",
            MetadataItemKind::Company => "companies",
            MetadataItemKind::Award => "awards",
            MetadataItemKind::Item => "items",
        }
    }
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
    /// Koko locale key for this snapshot.
    pub locale_key: String,
    /// Provider-specific locale key used to fetch this snapshot.
    pub provider_locale_key: Option<String>,
    /// Raw provider payload.
    pub provider_payload_json: Option<String>,
}

/// Presentation fields derived from one stored metadata link.
#[derive(Debug, Clone, Default, PartialEq)]
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
    /// Provider-supplied title logo URL, when available.
    pub logo_url: Option<String>,
    /// Provider-supplied user/community rating, when available.
    pub rating: Option<f32>,
    /// Provider-supplied content rating such as PG-13 or TV-MA, when available.
    pub content_rating: Option<String>,
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
    tvdb_id: Option<String>,
    imdb_id: Option<String>,
}

static BRACED_TAG_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"[\{\[]([^\}\]]*)[\}\]]").unwrap());
static YEAR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(19\d{2}|20\d{2}|21\d{2})\b").unwrap());
static PARENTHETICAL_YEAR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[\(\[]\s*(19\d{2}|20\d{2}|21\d{2})\s*[\)\]]").unwrap());
static SPLIT_SUFFIX_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\s*[-–]\s*(cd|disc|disk|dvd|part|pt)\s*\d+\s*$").unwrap());
static TITLE_COLON_DASH_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s*-\s+").unwrap());
static NOISE_TOKEN_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)\b(2160p|1080p|720p|480p|x264|x265|h264|h265|hevc|hdr|dv|webrip|web[- ]dl|bluray|brrip|dvdrip|remux|proper|repack|extended|unrated|criterion|aac|dts|truehd|atmos)\b",
    )
    .unwrap()
});

/// Normalize a Koko locale key into the canonical storage format.
pub fn normalize_locale_key(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return DEFAULT_METADATA_LOCALE.to_string();
    }

    let mut parts = trimmed.split(['-', '_']);
    let language = parts.next().unwrap_or("en").to_ascii_lowercase();
    if let Some(region) = parts.next().filter(|region| !region.is_empty()) {
        format!("{}-{}", language, region.to_ascii_uppercase())
    } else if language == "en" {
        DEFAULT_METADATA_LOCALE.to_string()
    } else {
        language
    }
}

/// Map a Koko locale key to a provider-specific locale key.
pub fn provider_locale_key(
    provider_id: MetadataProviderId,
    locale_key: &str,
) -> String {
    let normalized = normalize_locale_key(locale_key);
    match provider_id {
        MetadataProviderId::Tvdb => match normalized.as_str() {
            "en-GB" | "en-US" => "eng",
            "es" => "spa",
            "es-ES" => "spa",
            "fr" => "fra",
            "fr-FR" => "fra",
            "de" => "deu",
            "de-DE" => "deu",
            "it" => "ita",
            "it-IT" => "ita",
            "ja" => "jpn",
            "ja-JP" => "jpn",
            "pt" => "por",
            "pt-BR" => "por",
            _ => "eng",
        }
        .to_string(),
        _ => normalized,
    }
}

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
                Box::new(TvdbMetadataProvider),
                Box::new(MusicBrainzMetadataProvider),
                Box::new(OpenLibraryMetadataProvider),
                Box::new(LocalNfoMetadataProvider),
            ],
        }
    }

    /// Return all built-in provider descriptors.
    pub fn descriptors(&self) -> Vec<MetadataProviderDescriptor> {
        self.providers
            .iter()
            .map(|provider| provider.descriptor())
            .collect()
    }
}

impl Default for MetadataRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Return provider statuses after applying the current settings.
pub fn list_provider_statuses(settings: &MetadataSettings) -> Vec<MetadataProviderStatus> {
    let configured_settings: std::collections::HashMap<
        MetadataProviderId,
        MetadataProviderSettings,
    > = settings
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
            let enabled = setting
                .as_ref()
                .map(|provider| provider.enabled)
                .unwrap_or(false);
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
                attribution_text: descriptor.attribution_text,
                attribution_url: descriptor.attribution_url,
                logo_light_url: descriptor.logo_light_url,
                logo_dark_url: descriptor.logo_dark_url,
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

/// Return primary metadata links that were left pending without an active in-memory worker.
pub fn list_pending_item_metadata_links(
    conn: &mut SqliteConnection
) -> Result<Vec<ItemMetadataLink>, diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as metadata_links_dsl;

    metadata_links_dsl::item_metadata_links
        .filter(metadata_links_dsl::relation_kind.eq("primary"))
        .filter(metadata_links_dsl::refresh_state.eq("pending"))
        .order(metadata_links_dsl::updated_at.asc())
        .select(ItemMetadataLink::as_select())
        .load::<ItemMetadataLink>(conn)
}

/// Return primary metadata links whose automatic refresh interval has elapsed.
pub fn list_due_item_metadata_links(
    conn: &mut SqliteConnection,
    now: i64,
    limit: i64,
) -> Result<Vec<ItemMetadataLink>, diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as metadata_links_dsl;

    metadata_links_dsl::item_metadata_links
        .filter(metadata_links_dsl::relation_kind.eq("primary"))
        .filter(metadata_links_dsl::refresh_state.ne("pending"))
        .filter(metadata_links_dsl::next_refresh_at.le(now))
        .order(metadata_links_dsl::next_refresh_at.asc())
        .limit(limit)
        .select(ItemMetadataLink::as_select())
        .load::<ItemMetadataLink>(conn)
}

/// Search TMDB for metadata candidates using the current provider configuration.
pub async fn search_provider(
    settings: &MetadataSettings,
    provider_id: MetadataProviderId,
    query: &str,
) -> Result<Vec<MetadataSearchResult>, String> {
    match provider_id {
        MetadataProviderId::Tmdb => search_tmdb(settings, query).await,
        MetadataProviderId::Tvdb => search_tvdb(settings, query).await,
        _ => Err(format!(
            "{} search is not implemented.",
            provider_display_name(&provider_id)
        )),
    }
}

/// Fetch and normalize one provider metadata snapshot.
pub async fn fetch_provider_metadata_snapshot(
    settings: &MetadataSettings,
    provider_id: MetadataProviderId,
    external_id: &str,
    media_type: &str,
) -> Result<StoredMetadataSnapshot, String> {
    fetch_provider_metadata_snapshot_for_locale(
        settings,
        provider_id,
        external_id,
        media_type,
        DEFAULT_METADATA_LOCALE,
    )
    .await
}

/// Fetch and normalize one provider metadata snapshot for a specific Koko locale.
pub async fn fetch_provider_metadata_snapshot_for_locale(
    settings: &MetadataSettings,
    provider_id: MetadataProviderId,
    external_id: &str,
    media_type: &str,
    locale_key: &str,
) -> Result<StoredMetadataSnapshot, String> {
    let locale_key = normalize_locale_key(locale_key);
    let mut last_error = None;
    for fetch_locale in locale_fallback_chain(&locale_key) {
        let provider_locale = provider_locale_key(provider_id.clone(), &fetch_locale);
        let mut localized_settings = settings.clone();
        if let Some(provider) = localized_settings
            .providers
            .iter_mut()
            .find(|provider| provider.id == provider_id)
        {
            provider.language = provider_locale.clone();
        }

        let result = match provider_id {
            MetadataProviderId::Tmdb => {
                fetch_tmdb_metadata_snapshot(&localized_settings, external_id, media_type).await
            }
            MetadataProviderId::Tvdb => {
                fetch_tvdb_metadata_snapshot(&localized_settings, external_id, media_type).await
            }
            _ => Err(format!(
                "{} metadata fetch is not implemented.",
                provider_display_name(&provider_id)
            )),
        };

        match result {
            Ok(mut snapshot) if snapshot_has_presentable_metadata(&snapshot) => {
                snapshot.locale_key = locale_key;
                snapshot.provider_locale_key = Some(provider_locale);
                return Ok(snapshot);
            }
            Ok(mut snapshot) => {
                snapshot.locale_key = locale_key.clone();
                snapshot.provider_locale_key = Some(provider_locale);
                last_error = Some("metadata provider returned an empty localized snapshot".into());
            }
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error.unwrap_or_else(|| {
        format!(
            "{} metadata fetch is not implemented.",
            provider_display_name(&provider_id)
        )
    }))
}

fn locale_fallback_chain(locale_key: &str) -> Vec<String> {
    let normalized = normalize_locale_key(locale_key);
    let mut locales = vec![normalized.clone()];
    if let Some(language) = normalized
        .split('-')
        .next()
        .filter(|language| !language.is_empty() && *language != normalized && *language != "en")
    {
        locales.push(language.to_string());
    }
    if !locales
        .iter()
        .any(|locale| locale == DEFAULT_METADATA_LOCALE)
    {
        locales.push(DEFAULT_METADATA_LOCALE.to_string());
    }
    locales
}

fn snapshot_has_presentable_metadata(snapshot: &StoredMetadataSnapshot) -> bool {
    snapshot
        .title
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        || snapshot
            .overview
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        || snapshot.artwork_url.is_some()
        || snapshot.backdrop_url.is_some()
        || snapshot.provider_payload_json.is_some()
}

/// Fetch one provider season snapshot for a linked show descendant.
pub async fn fetch_provider_season_metadata_snapshot(
    settings: &MetadataSettings,
    provider_id: MetadataProviderId,
    show_external_id: &str,
    season_number: i32,
    season_external_id: Option<&str>,
) -> Result<StoredMetadataSnapshot, String> {
    match provider_id {
        MetadataProviderId::Tmdb => {
            fetch_tmdb_season_metadata_snapshot(settings, show_external_id, season_number).await
        }
        MetadataProviderId::Tvdb => {
            let season_external_id = season_external_id.ok_or_else(|| {
                "TheTVDB season refresh is missing a season external id.".to_string()
            })?;
            fetch_tvdb_season_metadata_snapshot(
                settings,
                show_external_id,
                season_number,
                season_external_id,
            )
            .await
        }
        _ => Err(format!(
            "{} season metadata fetch is not implemented.",
            provider_display_name(&provider_id)
        )),
    }
}

/// Fetch one provider episode snapshot for a linked show descendant.
pub async fn fetch_provider_episode_metadata_snapshot(
    settings: &MetadataSettings,
    provider_id: MetadataProviderId,
    show_external_id: &str,
    season_number: i32,
    episode_number: i32,
    episode_external_id: Option<&str>,
) -> Result<StoredMetadataSnapshot, String> {
    match provider_id {
        MetadataProviderId::Tmdb => {
            fetch_tmdb_episode_metadata_snapshot(
                settings,
                show_external_id,
                season_number,
                episode_number,
            )
            .await
        }
        MetadataProviderId::Tvdb => {
            let episode_external_id = episode_external_id.ok_or_else(|| {
                "TheTVDB episode refresh is missing an episode external id.".to_string()
            })?;
            fetch_tvdb_episode_metadata_snapshot(
                settings,
                show_external_id,
                season_number,
                episode_number,
                episode_external_id,
            )
            .await
        }
        _ => Err(format!(
            "{} episode metadata fetch is not implemented.",
            provider_display_name(&provider_id)
        )),
    }
}

/// Guess the best provider movie match for one library item.
pub async fn guess_provider_movie_match(
    settings: &MetadataSettings,
    provider_id: MetadataProviderId,
    relative_path: &str,
    display_title: &str,
) -> Result<Option<MetadataSearchResult>, String> {
    match provider_id {
        MetadataProviderId::Tmdb => {
            guess_tmdb_movie_match(settings, relative_path, display_title).await
        }
        MetadataProviderId::Tvdb => {
            guess_tvdb_movie_match(settings, relative_path, display_title).await
        }
        _ => Ok(None),
    }
}

/// Guess the best provider show match for one show item.
pub async fn guess_provider_show_match(
    settings: &MetadataSettings,
    provider_id: MetadataProviderId,
    relative_path: &str,
    display_title: &str,
) -> Result<Option<MetadataSearchResult>, String> {
    match provider_id {
        MetadataProviderId::Tmdb => {
            guess_tmdb_show_match(settings, relative_path, display_title).await
        }
        MetadataProviderId::Tvdb => {
            guess_tvdb_show_match(settings, relative_path, display_title).await
        }
        _ => Ok(None),
    }
}
/// Search TMDB for metadata candidates using the current provider configuration.
pub async fn search_tmdb(
    settings: &MetadataSettings,
    query: &str,
) -> Result<Vec<MetadataSearchResult>, String> {
    providers::tmdb::search(settings, query).await
}

/// Search TheTVDB for metadata candidates using the current provider configuration.
pub async fn search_tvdb(
    settings: &MetadataSettings,
    query: &str,
) -> Result<Vec<MetadataSearchResult>, String> {
    providers::tvdb::search(settings, query).await
}

/// Fetch and normalize a TMDB metadata snapshot for one provider item.
pub async fn fetch_tmdb_metadata_snapshot(
    settings: &MetadataSettings,
    external_id: &str,
    media_type: &str,
) -> Result<StoredMetadataSnapshot, String> {
    providers::tmdb::fetch_snapshot(settings, external_id, media_type).await
}

/// Fetch and normalize a TheTVDB metadata snapshot for one provider item.
pub async fn fetch_tvdb_metadata_snapshot(
    settings: &MetadataSettings,
    external_id: &str,
    media_type: &str,
) -> Result<StoredMetadataSnapshot, String> {
    providers::tvdb::fetch_snapshot(settings, external_id, media_type).await
}

/// Guess the best TMDB movie match for one library item using filename cleanup and fuzzy scoring.
pub async fn guess_tmdb_movie_match(
    settings: &MetadataSettings,
    relative_path: &str,
    display_title: &str,
) -> Result<Option<MetadataSearchResult>, String> {
    providers::tmdb::guess_movie_match(settings, relative_path, display_title).await
}

/// Guess the best TheTVDB movie match for one library item using filename cleanup and fuzzy scoring.
pub async fn guess_tvdb_movie_match(
    settings: &MetadataSettings,
    relative_path: &str,
    display_title: &str,
) -> Result<Option<MetadataSearchResult>, String> {
    providers::tvdb::guess_movie_match(settings, relative_path, display_title).await
}

/// Guess the best TMDB television match for one show item using the show title and path.
pub async fn guess_tmdb_show_match(
    settings: &MetadataSettings,
    relative_path: &str,
    display_title: &str,
) -> Result<Option<MetadataSearchResult>, String> {
    providers::tmdb::guess_show_match(settings, relative_path, display_title).await
}

/// Guess the best TheTVDB television match for one show item using the show title and path.
pub async fn guess_tvdb_show_match(
    settings: &MetadataSettings,
    relative_path: &str,
    display_title: &str,
) -> Result<Option<MetadataSearchResult>, String> {
    providers::tvdb::guess_show_match(settings, relative_path, display_title).await
}

/// Fetch TMDB metadata for one season of a linked show.
pub async fn fetch_tmdb_season_metadata_snapshot(
    settings: &MetadataSettings,
    show_external_id: &str,
    season_number: i32,
) -> Result<StoredMetadataSnapshot, String> {
    providers::tmdb::fetch_season_snapshot(settings, show_external_id, season_number).await
}

/// Fetch TheTVDB metadata for one season of a linked show.
pub async fn fetch_tvdb_season_metadata_snapshot(
    settings: &MetadataSettings,
    show_external_id: &str,
    season_number: i32,
    season_external_id: &str,
) -> Result<StoredMetadataSnapshot, String> {
    providers::tvdb::fetch_season_snapshot(
        settings,
        show_external_id,
        season_number,
        season_external_id,
    )
    .await
}

/// Fetch TMDB metadata for one episode of a linked show.
pub async fn fetch_tmdb_episode_metadata_snapshot(
    settings: &MetadataSettings,
    show_external_id: &str,
    season_number: i32,
    episode_number: i32,
) -> Result<StoredMetadataSnapshot, String> {
    providers::tmdb::fetch_episode_snapshot(
        settings,
        show_external_id,
        season_number,
        episode_number,
    )
    .await
}

/// Fetch TheTVDB metadata for one episode of a linked show.
pub async fn fetch_tvdb_episode_metadata_snapshot(
    settings: &MetadataSettings,
    show_external_id: &str,
    season_number: i32,
    episode_number: i32,
    episode_external_id: &str,
) -> Result<StoredMetadataSnapshot, String> {
    providers::tvdb::fetch_episode_snapshot(
        settings,
        show_external_id,
        season_number,
        episode_number,
        episode_external_id,
    )
    .await
}

/// Load TheTVDB descendant metadata targets for one linked show.
pub async fn load_tvdb_show_descendant_targets(
    settings: &MetadataSettings,
    show_external_id: &str,
) -> Result<Vec<TvdbDescendantTarget>, String> {
    providers::tvdb::load_show_descendant_targets(settings, show_external_id).await
}

/// Resolve a ThemerrDB YouTube theme-song URL for one linked TMDB movie or show.
pub async fn fetch_themerr_youtube_theme_url(
    tmdb_media_type: &str,
    external_id: &str,
) -> Result<Option<String>, String> {
    fetch_themerr_youtube_theme_url_for_database(tmdb_media_type, "themoviedb", external_id).await
}

/// Resolve a ThemerrDB YouTube theme-song URL for one media item using a specific external id database.
pub async fn fetch_themerr_youtube_theme_url_for_database(
    tmdb_media_type: &str,
    database_id: &str,
    external_id: &str,
) -> Result<Option<String>, String> {
    let Some(database_path) = themerr_database_path_for_tmdb_media_type(tmdb_media_type) else {
        return Ok(None);
    };
    let Some(database_id) = themerr_database_id(database_id) else {
        return Ok(None);
    };
    let normalized_external_id = external_id.trim();
    if normalized_external_id.is_empty() {
        return Ok(None);
    }

    let response = reqwest::Client::new()
        .get(format!(
            "{}/{}/{}/{}.json",
            THEMERR_API_BASE, database_path, database_id, normalized_external_id
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
    upsert_item_metadata_snapshot_with_refresh_interval(
        conn,
        item_id,
        snapshot,
        Some(DEFAULT_METADATA_REFRESH_INTERVAL_SECONDS),
    )
}

/// Upsert a stored metadata snapshot using an explicit automatic refresh interval.
pub fn upsert_item_metadata_snapshot_with_refresh_interval(
    conn: &mut SqliteConnection,
    item_id: i32,
    snapshot: &StoredMetadataSnapshot,
    refresh_interval_seconds: Option<i64>,
) -> Result<ItemMetadataSummary, diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as metadata_links_dsl;
    configure_sqlite_connection(conn)?;
    retry_sqlite_write(|| {
        let existing = metadata_links_dsl::item_metadata_links
            .filter(metadata_links_dsl::media_item_id.eq(item_id))
            .filter(metadata_links_dsl::provider_id.eq(snapshot.provider_id.as_storage_value()))
            .filter(metadata_links_dsl::relation_kind.eq("primary"))
            .filter(metadata_links_dsl::locale_key.eq(&snapshot.locale_key))
            .select(ItemMetadataLink::as_select())
            .first(conn)
            .optional()?;
        let keep_cached_artwork = existing
            .as_ref()
            .map(|row| {
                !metadata_refresh_target_changed(
                    row,
                    snapshot.provider_id.as_storage_value(),
                    &snapshot.external_id,
                    snapshot.media_type.as_deref(),
                ) && row.artwork_url == snapshot.artwork_url
            })
            .unwrap_or(false);
        let keep_cached_backdrop = existing
            .as_ref()
            .map(|row| {
                !metadata_refresh_target_changed(
                    row,
                    snapshot.provider_id.as_storage_value(),
                    &snapshot.external_id,
                    snapshot.media_type.as_deref(),
                ) && row.backdrop_url == snapshot.backdrop_url
            })
            .unwrap_or(false);
        let parsed_payload = snapshot
            .provider_payload_json
            .as_deref()
            .and_then(|payload| serde_json::from_str::<Value>(payload).ok());
        let logo_url = parsed_payload.as_ref().and_then(provider_logo_url);
        let keep_cached_logo = existing
            .as_ref()
            .map(|row| {
                !metadata_refresh_target_changed(
                    row,
                    snapshot.provider_id.as_storage_value(),
                    &snapshot.external_id,
                    snapshot.media_type.as_deref(),
                ) && row.logo_url == logo_url
            })
            .unwrap_or(false);
        let genres = parsed_payload
            .as_ref()
            .map(provider_genres)
            .unwrap_or_default();

        let trailer = parsed_payload.as_ref().and_then(tmdb_trailer_entry);
        let payload = NewItemMetadataLink {
            media_item_id: item_id,
            provider_id: snapshot.provider_id.as_storage_value().to_string(),
            external_id: snapshot.external_id.clone(),
            title: snapshot.title.clone(),
            overview: snapshot.overview.clone(),
            tagline: parsed_payload.as_ref().and_then(provider_tagline),
            artwork_url: snapshot.artwork_url.clone(),
            backdrop_url: snapshot.backdrop_url.clone(),
            release_year: snapshot.release_year,
            media_type: snapshot.media_type.clone(),
            relation_kind: "primary".into(),
            match_state: "linked".into(),
            provider_payload_json: snapshot.provider_payload_json.clone(),
            logo_url,
            cached_logo_path: if keep_cached_logo {
                existing
                    .as_ref()
                    .and_then(|row| row.cached_logo_path.clone())
            } else {
                None
            },
            genres_json: serde_json::to_string(&genres).ok(),
            rating: parsed_payload.as_ref().and_then(provider_rating),
            content_rating: parsed_payload.as_ref().and_then(provider_content_rating),
            trailer_title: trailer
                .and_then(|entry| entry.get("name"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned),
            trailer_url: trailer
                .and_then(|entry| {
                    entry
                        .get("site")
                        .and_then(Value::as_str)
                        .zip(entry.get("key").and_then(Value::as_str))
                })
                .and_then(|(site, key)| youtube_embed_url(site, key)),
            locale_key: snapshot.locale_key.clone(),
            provider_locale_key: snapshot.provider_locale_key.clone(),
            cached_artwork_path: if keep_cached_artwork {
                existing
                    .as_ref()
                    .and_then(|row| row.cached_artwork_path.clone())
            } else {
                None
            },
            cached_backdrop_path: if keep_cached_backdrop {
                existing
                    .as_ref()
                    .and_then(|row| row.cached_backdrop_path.clone())
            } else {
                None
            },
            refresh_state: "fresh".into(),
            refresh_interval_seconds: refresh_interval_seconds.unwrap_or(0),
            last_refreshed_at: Some(current_timestamp()),
            next_refresh_at: refresh_interval_seconds
                .map(|interval| current_timestamp() + interval),
            refresh_error: None,
            updated_at: Some(current_timestamp()),
        };

        if let Some(existing) = existing {
            diesel::update(
                metadata_links_dsl::item_metadata_links
                    .filter(metadata_links_dsl::id.eq(existing.id)),
            )
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
            .filter(metadata_links_dsl::relation_kind.eq("primary"))
            .filter(metadata_links_dsl::locale_key.eq(&snapshot.locale_key))
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
        let keep_cached_paths = existing
            .as_ref()
            .map(|row| {
                !metadata_refresh_target_changed(
                    row,
                    provider_id.as_storage_value(),
                    external_id,
                    media_type,
                )
            })
            .unwrap_or(false);

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
            provider_payload_json: existing
                .as_ref()
                .and_then(|row| row.provider_payload_json.clone()),
            logo_url: existing.as_ref().and_then(|row| row.logo_url.clone()),
            cached_logo_path: existing
                .as_ref()
                .and_then(|row| row.cached_logo_path.clone()),
            genres_json: existing.as_ref().and_then(|row| row.genres_json.clone()),
            rating: existing.as_ref().and_then(|row| row.rating),
            content_rating: existing.as_ref().and_then(|row| row.content_rating.clone()),
            trailer_title: existing.as_ref().and_then(|row| row.trailer_title.clone()),
            trailer_url: existing.as_ref().and_then(|row| row.trailer_url.clone()),
            locale_key: existing
                .as_ref()
                .map(|row| row.locale_key.clone())
                .unwrap_or_else(|| DEFAULT_METADATA_LOCALE.to_string()),
            provider_locale_key: existing
                .as_ref()
                .and_then(|row| row.provider_locale_key.clone()),
            cached_artwork_path: if keep_cached_paths {
                existing
                    .as_ref()
                    .and_then(|row| row.cached_artwork_path.clone())
            } else {
                None
            },
            cached_backdrop_path: if keep_cached_paths {
                existing
                    .as_ref()
                    .and_then(|row| row.cached_backdrop_path.clone())
            } else {
                None
            },
            refresh_state: refresh_state.to_string(),
            refresh_interval_seconds: existing
                .as_ref()
                .map(|row| row.refresh_interval_seconds)
                .unwrap_or(DEFAULT_METADATA_REFRESH_INTERVAL_SECONDS),
            last_refreshed_at: existing.as_ref().and_then(|row| row.last_refreshed_at),
            next_refresh_at: existing.as_ref().and_then(|row| row.next_refresh_at),
            refresh_error: refresh_error.map(str::to_string),
            updated_at: Some(current_timestamp()),
        };

        if let Some(existing) = existing {
            diesel::update(
                metadata_links_dsl::item_metadata_links
                    .filter(metadata_links_dsl::id.eq(existing.id)),
            )
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

fn metadata_refresh_target_changed(
    existing: &ItemMetadataLink,
    provider_id: &str,
    external_id: &str,
    media_type: Option<&str>,
) -> bool {
    existing.provider_id != provider_id
        || existing.external_id != external_id
        || existing.media_type.as_deref() != media_type
}

/// Return collection summaries derived from stored metadata for the requested library scope.
pub fn list_metadata_collection_summaries(
    conn: &mut SqliteConnection,
    library_id: Option<i32>,
) -> Result<Vec<MetadataCollectionSummary>, diesel::result::Error> {
    list_metadata_collection_summaries_with_preferred_languages(conn, library_id, &[])
}

/// Return collection summaries using only each item's preferred metadata locale.
pub fn list_metadata_collection_summaries_with_preferred_languages(
    conn: &mut SqliteConnection,
    library_id: Option<i32>,
    preferred_languages: &[String],
) -> Result<Vec<MetadataCollectionSummary>, diesel::result::Error> {
    use crate::db::schema::item_metadata_collections::dsl as collection_dsl;
    use crate::db::schema::item_metadata_links::dsl as link_dsl;
    use crate::db::schema::media_items::dsl as media_items_dsl;

    let mut item_query = media_items_dsl::media_items.into_boxed();
    if let Some(library_id) = library_id {
        item_query = item_query.filter(media_items_dsl::library_id.eq(library_id));
    }
    let items = item_query
        .select(MediaItem::as_select())
        .load::<MediaItem>(conn)?;
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
        .filter(link_dsl::relation_kind.eq("primary"))
        .select(ItemMetadataLink::as_select())
        .load::<ItemMetadataLink>(conn)?;
    if link_rows.is_empty() {
        return Ok(Vec::new());
    }

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
    let language_rank = languages
        .iter()
        .enumerate()
        .map(|(index, language)| (language.clone(), index))
        .collect::<HashMap<_, _>>();
    let fallback_rank = languages.len();
    let mut preferred_links_by_item_id = HashMap::<i32, (usize, ItemMetadataLink)>::new();
    for link in link_rows {
        let rank = language_rank
            .get(&normalize_locale_key(&link.locale_key))
            .copied()
            .unwrap_or(fallback_rank);
        match preferred_links_by_item_id.entry(link.media_item_id) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert((rank, link));
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                if rank < entry.get().0 {
                    entry.insert((rank, link));
                }
            }
        }
    }

    let links_by_id = preferred_links_by_item_id
        .into_values()
        .map(|(_rank, link)| link)
        .map(|link| (link.id, link))
        .collect::<HashMap<_, _>>();
    let collection_rows = collection_dsl::item_metadata_collections
        .filter(
            collection_dsl::metadata_link_id
                .eq_any(links_by_id.keys().copied().collect::<Vec<_>>()),
        )
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
            .entry(format!(
                "{}:{}",
                collection.provider_id, collection.external_id
            ))
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
    get_preferred_item_metadata_link_for_languages(
        conn,
        item_id,
        &[DEFAULT_METADATA_LOCALE.to_string()],
    )
}

/// Return the best stored primary metadata link for the requested language order.
pub fn get_preferred_item_metadata_link_for_languages(
    conn: &mut SqliteConnection,
    item_id: i32,
    preferred_languages: &[String],
) -> Result<Option<ItemMetadataLink>, diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as metadata_links_dsl;

    let rows = metadata_links_dsl::item_metadata_links
        .filter(metadata_links_dsl::media_item_id.eq(item_id))
        .filter(metadata_links_dsl::relation_kind.eq("primary"))
        .order(metadata_links_dsl::updated_at.desc())
        .select(ItemMetadataLink::as_select())
        .load::<ItemMetadataLink>(conn)?;

    if rows.is_empty() {
        return Ok(None);
    }

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

    Ok(languages
        .iter()
        .find_map(|language| rows.iter().find(|row| row.locale_key == *language).cloned())
        .or_else(|| rows.into_iter().next()))
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

    let tagline = link.tagline.clone().or_else(|| {
        parsed_payload
            .as_ref()
            .and_then(|payload| payload.get("tagline"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    });
    let overview = link
        .overview
        .clone()
        .or_else(|| parsed_payload.as_ref().and_then(provider_overview));
    let genres = link
        .genres_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .filter(|genres| !genres.is_empty())
        .or_else(|| parsed_payload.as_ref().map(provider_genres))
        .unwrap_or_default();
    let genres = if genres.is_empty() {
        parsed_payload
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
            .unwrap_or_default()
    } else {
        genres
    };
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
    let logo_url = link
        .logo_url
        .clone()
        .or_else(|| parsed_payload.as_ref().and_then(provider_logo_url));
    let rating = link
        .rating
        .or_else(|| parsed_payload.as_ref().and_then(provider_rating));
    let content_rating = link
        .content_rating
        .clone()
        .or_else(|| parsed_payload.as_ref().and_then(provider_content_rating));

    LinkedMetadataPresentation {
        tagline,
        overview,
        genres,
        release_year,
        media_type: link.media_type.clone(),
        poster_available: link.cached_artwork_path.is_some() || link.artwork_url.is_some(),
        backdrop_available: link.cached_backdrop_path.is_some() || link.backdrop_url.is_some(),
        logo_url,
        rating,
        content_rating,
        trailer_title: link.trailer_title.clone().or_else(|| {
            parsed_payload
                .as_ref()
                .and_then(tmdb_trailer_entry)
                .and_then(|entry| entry.get("name"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        }),
        trailer_url: link.trailer_url.clone().or_else(|| {
            parsed_payload
                .as_ref()
                .and_then(tmdb_trailer_entry)
                .and_then(|entry| {
                    entry
                        .get("site")
                        .and_then(Value::as_str)
                        .zip(entry.get("key").and_then(Value::as_str))
                })
                .and_then(|(site, key)| youtube_embed_url(site, key))
        }),
    }
}

fn provider_logo_url(payload: &Value) -> Option<String> {
    payload
        .get("images")
        .and_then(|images| images.get("logos"))
        .and_then(Value::as_array)
        .and_then(|logos| {
            logos.iter().find_map(|logo| {
                logo.get("file_path")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|path| !path.is_empty())
                    .map(|path| tmdb_image_url(path, "w500"))
                    .or_else(|| {
                        logo.get("image")
                            .or_else(|| logo.get("image_url"))
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|url| !url.is_empty())
                            .map(ToOwned::to_owned)
                    })
            })
        })
        .or_else(|| provider_tvdb_logo_artwork(payload))
        .or_else(|| {
            payload
                .get("data")
                .and_then(provider_tvdb_logo_artwork)
        })
}

fn provider_tagline(payload: &Value) -> Option<String> {
    text_field(payload, &["tagline"])
        .or_else(|| {
            payload
                .get("data")
                .and_then(|data| text_field(data, &["tagline"]))
        })
        .or_else(|| {
            payload
                .get("koko_translation")
                .and_then(|translation| text_field(translation, &["tagline"]))
        })
        .or_else(|| {
            payload
                .get("data")
                .and_then(|data| data.get("koko_translation"))
                .and_then(|translation| text_field(translation, &["tagline"]))
        })
}

fn provider_tvdb_logo_artwork(payload: &Value) -> Option<String> {
    let expected_language = payload
        .get("koko_provider_language")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|language| !language.is_empty())?;
    provider_tvdb_artwork_with_language(payload, &[23, 25], expected_language)
}

fn provider_tvdb_artwork_with_language(
    payload: &Value,
    preferred_types: &[i64],
    expected_language: &str,
) -> Option<String> {
    let artworks = payload
        .get("artworks")
        .or_else(|| payload.get("artwork"))
        .and_then(Value::as_array)?;
    preferred_types.iter().find_map(|preferred_type| {
        artworks
            .iter()
            .filter(|artwork| artwork.get("type").and_then(Value::as_i64) == Some(*preferred_type))
            .filter(|artwork| tvdb_artwork_language_matches(artwork, expected_language))
            .max_by(|left, right| {
                let left_score = left.get("score").and_then(Value::as_f64).unwrap_or(0.0);
                let right_score = right.get("score").and_then(Value::as_f64).unwrap_or(0.0);
                left_score
                    .partial_cmp(&right_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .and_then(|artwork| {
                artwork
                    .get("image")
                    .or_else(|| artwork.get("thumbnail"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|url| !url.is_empty())
                    .map(ToOwned::to_owned)
            })
    })
}

fn tvdb_artwork_language_matches(
    artwork: &Value,
    expected_language: &str,
) -> bool {
    let expected = expected_language.trim();
    artwork
        .get("language")
        .or_else(|| artwork.get("languageCode"))
        .or_else(|| artwork.get("iso_639_1"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|language| language.eq_ignore_ascii_case(expected))
}

fn provider_genres(payload: &Value) -> Vec<String> {
    let mut genres = Vec::new();
    collect_provider_genres(payload.get("genres"), &mut genres);
    if let Some(data) = payload.get("data") {
        collect_provider_genres(data.get("genres"), &mut genres);
    }
    genres
}

fn collect_provider_genres(
    value: Option<&Value>,
    genres: &mut Vec<String>,
) {
    let Some(value) = value else {
        return;
    };
    match value {
        Value::Array(entries) => {
            for entry in entries {
                let genre = entry
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .or_else(|| {
                        entry
                            .get("name")
                            .or_else(|| entry.get("label"))
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(ToOwned::to_owned)
                    });
                if let Some(genre) = genre {
                    push_unique_genre(genres, genre);
                }
            }
        }
        Value::Object(map) => {
            for value in map.values() {
                if let Some(genre) = value
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                {
                    push_unique_genre(genres, genre);
                }
            }
        }
        Value::String(value) => push_unique_genre(genres, value.trim().to_string()),
        _ => {}
    }
}

fn push_unique_genre(
    genres: &mut Vec<String>,
    genre: String,
) {
    if !genre.is_empty()
        && !genres
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&genre))
    {
        genres.push(genre);
    }
}

fn provider_rating(payload: &Value) -> Option<f32> {
    payload
        .get("vote_average")
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        .or_else(|| {
            payload
                .get("score")
                .and_then(Value::as_f64)
                .map(|value| value as f32)
        })
}

fn provider_content_rating(payload: &Value) -> Option<String> {
    payload
        .get("release_dates")
        .and_then(|release_dates| release_dates.get("results"))
        .and_then(Value::as_array)
        .and_then(|countries| {
            countries
                .iter()
                .find(|country| country.get("iso_3166_1").and_then(Value::as_str) == Some("US"))
                .or_else(|| countries.first())
        })
        .and_then(|country| country.get("release_dates"))
        .and_then(Value::as_array)
        .and_then(|dates| {
            dates.iter().find_map(|date| {
                date.get("certification")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
            })
        })
        .or_else(|| {
            payload
                .get("content_ratings")
                .and_then(|ratings| ratings.get("results"))
                .and_then(Value::as_array)
                .and_then(|ratings| {
                    ratings
                        .iter()
                        .find(|rating| {
                            rating.get("iso_3166_1").and_then(Value::as_str) == Some("US")
                        })
                        .or_else(|| ratings.first())
                })
                .and_then(|rating| rating.get("rating"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            payload
                .get("contentRatings")
                .or_else(|| payload.get("content_ratings"))
                .and_then(Value::as_array)
                .and_then(|ratings| {
                    ratings
                        .iter()
                        .find(|rating| {
                            rating.get("country").and_then(Value::as_str) == Some("usa")
                                || rating.get("country").and_then(Value::as_str) == Some("us")
                                || rating.get("country").and_then(Value::as_str) == Some("US")
                        })
                        .or_else(|| ratings.first())
                })
                .and_then(|rating| rating.get("name").or_else(|| rating.get("fullName")))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
}

/// Persist stored metadata payload and cached artwork into the managed item asset structure.
pub async fn persist_item_metadata_assets(
    snapshot: &StoredMetadataSnapshot,
    _item_id: i32,
    data_dir: &str,
) -> Result<(Option<PathBuf>, Option<PathBuf>, Option<PathBuf>), String> {
    let item_dir = managed_metadata_asset_dir(
        data_dir,
        snapshot.provider_id.clone(),
        &snapshot.external_id,
        snapshot.media_type.as_deref(),
        &snapshot.locale_key,
    );
    fs::create_dir_all(&item_dir).map_err(|error| error.to_string())?;

    if let Some(payload_json) = &snapshot.provider_payload_json {
        let metadata_file_name = format!("{}.json", snapshot.provider_id.as_storage_value());
        fs::write(item_dir.join(metadata_file_name), payload_json)
            .map_err(|error| error.to_string())?;
    }

    let parsed_payload = snapshot
        .provider_payload_json
        .as_deref()
        .and_then(|payload| serde_json::from_str::<Value>(payload).ok());
    let logo_url = parsed_payload.as_ref().and_then(provider_logo_url);

    let poster_cache_key = format!("{}_poster", snapshot.provider_id.as_storage_value());
    let poster_path = if let Some(url) = &snapshot.artwork_url {
        try_cache_item_artwork(url, &item_dir, &poster_cache_key).await
    } else {
        purge_stale_cached_artwork_files(&item_dir, &poster_cache_key, None)?;
        None
    };
    let backdrop_cache_key = format!("{}_backdrop", snapshot.provider_id.as_storage_value());
    let backdrop_path = if let Some(url) = &snapshot.backdrop_url {
        try_cache_item_artwork(url, &item_dir, &backdrop_cache_key).await
    } else {
        purge_stale_cached_artwork_files(&item_dir, &backdrop_cache_key, None)?;
        None
    };
    let logo_cache_key = format!("{}_logo", snapshot.provider_id.as_storage_value());
    let logo_path = if let Some(url) = logo_url {
        try_cache_item_artwork(&url, &item_dir, &logo_cache_key).await
    } else {
        purge_stale_cached_artwork_files(&item_dir, &logo_cache_key, None)?;
        None
    };

    Ok((poster_path, backdrop_path, logo_path))
}

/// Return the deterministic provider-uuid based asset path for metadata payloads.
pub fn managed_metadata_asset_dir(
    data_dir: &str,
    provider_id: MetadataProviderId,
    external_id: &str,
    media_type: Option<&str>,
    locale_key: &str,
) -> PathBuf {
    let item_kind = providers::metadata_item_kind(provider_id.clone(), media_type);
    let uuid = metadata_asset_uuid(provider_id, external_id, locale_key);
    let full_hash = Sha256::digest(uuid.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let (shard, directory_name) = full_hash.split_at(1);

    Path::new(data_dir)
        .join("metadata")
        .join(metadata_asset_type_directory(item_kind))
        .join(shard)
        .join(directory_name)
}

/// Return the stable provider UUID used to derive metadata paths.
pub fn metadata_asset_uuid(
    provider_id: MetadataProviderId,
    external_id: &str,
    locale_key: &str,
) -> String {
    format!(
        "{}:{}:{}",
        provider_id.as_storage_value(),
        external_id.trim(),
        normalize_locale_key(locale_key)
    )
}

fn metadata_asset_type_directory(item_kind: MetadataItemKind) -> &'static str {
    item_kind.asset_directory()
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
                diesel::update(
                    metadata_links_dsl::item_metadata_links
                        .filter(metadata_links_dsl::id.eq(link_id)),
                )
                .set(
                    metadata_links_dsl::cached_artwork_path
                        .eq(cache_path.to_string_lossy().to_string()),
                )
                .execute(conn)?;
            }
            ArtworkKind::Backdrop => {
                diesel::update(
                    metadata_links_dsl::item_metadata_links
                        .filter(metadata_links_dsl::id.eq(link_id)),
                )
                .set(
                    metadata_links_dsl::cached_backdrop_path
                        .eq(cache_path.to_string_lossy().to_string()),
                )
                .execute(conn)?;
            }
            ArtworkKind::Logo => {
                diesel::update(
                    metadata_links_dsl::item_metadata_links
                        .filter(metadata_links_dsl::id.eq(link_id)),
                )
                .set(
                    metadata_links_dsl::cached_logo_path
                        .eq(cache_path.to_string_lossy().to_string()),
                )
                .execute(conn)?;
            }
        }

        Ok(())
    })
}

/// Poster, backdrop, or title logo artwork kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtworkKind {
    /// Poster or cover art.
    Poster,
    /// Background or hero artwork.
    Backdrop,
    /// Title logo artwork.
    Logo,
}

impl ArtworkKind {
    /// Parse an artwork kind from a query parameter.
    pub fn from_query_value(value: Option<&str>) -> Self {
        match value.unwrap_or_default() {
            "backdrop" => ArtworkKind::Backdrop,
            "logo" => ArtworkKind::Logo,
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

    let cache_path = expected_artwork_cache_path(url, cache_dir, cache_key);
    let file_name = cache_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Invalid artwork cache file name".to_string())?
        .to_string();
    purge_stale_cached_artwork_files(cache_dir, cache_key, Some(&file_name))?;
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

/// Return the deterministic on-disk path for a cached artwork URL.
pub fn expected_artwork_cache_path(
    url: &str,
    cache_dir: &Path,
    cache_key: &str,
) -> PathBuf {
    let cache_file_name = format!(
        "{}-{:016x}.{}",
        sanitize_cache_key(cache_key),
        stable_artwork_url_hash(url),
        artwork_url_extension(url)
    );
    cache_dir.join(cache_file_name)
}

struct TmdbMetadataProvider;
struct TvdbMetadataProvider;
struct MusicBrainzMetadataProvider;
struct OpenLibraryMetadataProvider;
struct LocalNfoMetadataProvider;

impl MetadataProvider for TmdbMetadataProvider {
    fn descriptor(&self) -> MetadataProviderDescriptor {
        providers::tmdb::descriptor()
    }
}

impl MetadataProvider for TvdbMetadataProvider {
    fn descriptor(&self) -> MetadataProviderDescriptor {
        providers::tvdb::descriptor()
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
            attribution_text: "MusicBrainz metadata is provided by MusicBrainz.".into(),
            attribution_url: "https://musicbrainz.org/".into(),
            logo_light_url: None,
            logo_dark_url: None,
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
            attribution_text: "Book metadata is provided by Open Library.".into(),
            attribution_url: "https://openlibrary.org/".into(),
            logo_light_url: None,
            logo_dark_url: None,
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
            attribution_text: "Local metadata is provided by files in your library.".into(),
            attribution_url: String::new(),
            logo_light_url: None,
            logo_dark_url: None,
        }
    }
}

#[derive(Debug, Clone)]
struct TvdbCachedToken {
    token: String,
    expires_at: Instant,
}

/// Provider-side season and episode identifiers resolved for one show descendant.
#[derive(Debug, Clone)]
pub struct TvdbDescendantTarget {
    /// Local season number for matching persisted items.
    pub season_number: i32,
    /// Local episode number for matching persisted items.
    pub episode_number: i32,
    /// Provider-side season identifier.
    pub season_external_id: String,
    /// Provider-side episode identifier.
    pub episode_external_id: String,
}

fn tmdb_provider_settings(settings: &MetadataSettings) -> Result<MetadataProviderSettings, String> {
    provider_settings(settings, MetadataProviderId::Tmdb).map_err(|error| format!("TMDB {}", error))
}

fn provider_settings(
    settings: &MetadataSettings,
    provider_id: MetadataProviderId,
) -> Result<MetadataProviderSettings, String> {
    let provider = settings
        .providers
        .iter()
        .find(|provider| provider.id == provider_id && provider.enabled)
        .cloned()
        .ok_or_else(|| "is not enabled in the current configuration.".to_string())?;

    let api_key = provider
        .api_key
        .clone()
        .unwrap_or_default()
        .trim()
        .to_string();
    if api_key.is_empty() {
        return Err("is enabled but no API key is configured.".into());
    }

    Ok(provider)
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
        diesel::result::Error::DatabaseError(_, info) => info
            .message()
            .to_ascii_lowercase()
            .contains("database is locked"),
        _ => error
            .to_string()
            .to_ascii_lowercase()
            .contains("database is locked"),
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
        logo_url: link.logo_url,
        cached_logo_path: link.cached_logo_path,
        genres: link
            .genres_json
            .as_deref()
            .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
            .unwrap_or_default(),
        rating: link.rating,
        content_rating: link.content_rating,
        trailer_title: link.trailer_title,
        trailer_url: link.trailer_url,
        locale_key: link.locale_key,
        provider_locale_key: link.provider_locale_key,
        cached_artwork_path: link.cached_artwork_path,
        cached_backdrop_path: link.cached_backdrop_path,
        refresh_state: link.refresh_state,
        last_refreshed_at: link.last_refreshed_at,
        next_refresh_at: link.next_refresh_at,
        refresh_error: link.refresh_error,
        updated_at: link.updated_at,
    }
}

fn provider_overview(payload: &Value) -> Option<String> {
    text_field(
        payload,
        &[
            "overview",
            "description",
            "shortDescription",
            "longDescription",
        ],
    )
    .or_else(|| payload.get("data").and_then(provider_overview))
    .or_else(|| translated_text(payload.get("overviews"), &["eng", "en", "english"]))
    .or_else(|| {
        translated_text(
            payload.get("overviewTranslations"),
            &["eng", "en", "english"],
        )
    })
    .or_else(|| translated_text(payload.get("translations"), &["eng", "en", "english"]))
}

fn text_field(
    payload: &Value,
    keys: &[&str],
) -> Option<String> {
    keys.iter().find_map(|key| {
        payload
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn translated_text(
    value: Option<&Value>,
    preferred_keys: &[&str],
) -> Option<String> {
    let value = value?;
    if let Some(text) = value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        return Some(text.to_string());
    }

    if let Some(map) = value.as_object() {
        return preferred_keys
            .iter()
            .find_map(|key| map.get(*key).and_then(translated_text_value))
            .or_else(|| map.values().find_map(translated_text_value));
    }

    value.as_array().and_then(|entries| {
        preferred_keys
            .iter()
            .find_map(|key| {
                entries.iter().find_map(|entry| {
                    let language = entry
                        .get("language")
                        .or_else(|| entry.get("languageCode"))
                        .or_else(|| entry.get("iso_639_1"))
                        .and_then(Value::as_str)?;
                    language
                        .eq_ignore_ascii_case(key)
                        .then(|| translated_text_value(entry))
                        .flatten()
                })
            })
            .or_else(|| entries.iter().find_map(translated_text_value))
    })
}

fn translated_text_value(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| text_field(value, &["overview", "description"]))
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
                && entry
                    .get("official")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
        })
        .or_else(|| {
            results.iter().find(|entry| {
                entry.get("site").and_then(Value::as_str) == Some("YouTube")
                    && matches!(
                        entry.get("type").and_then(Value::as_str),
                        Some("Trailer" | "Teaser")
                    )
            })
        })
}

fn youtube_embed_url(
    site: &str,
    key: &str,
) -> Option<String> {
    if site != "YouTube" || key.trim().is_empty() {
        return None;
    }

    Some(format!(
        "https://www.youtube.com/embed/{}?autoplay=1&rel=0",
        key.trim()
    ))
}

fn themerr_database_path_for_tmdb_media_type(tmdb_media_type: &str) -> Option<&'static str> {
    match tmdb_media_type.trim() {
        "movie" => Some("movies/themoviedb"),
        "tv" => Some("tv_shows/themoviedb"),
        _ => None,
    }
}

fn themerr_database_id(database_id: &str) -> Option<&'static str> {
    match database_id.trim() {
        "themoviedb" => Some("themoviedb"),
        "imdb" => Some("imdb"),
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
        locale_key: link.locale_key,
        provider_locale_key: link.provider_locale_key,
        provider_payload_json: link.provider_payload_json,
    })
}

async fn try_cache_item_artwork(
    url: &str,
    item_dir: &Path,
    cache_key: &str,
) -> Option<PathBuf> {
    match cache_artwork(url, item_dir, cache_key).await {
        Ok(path) => Some(path),
        Err(error) => {
            log::warn!(
                "Failed to cache managed artwork asset from {}: {}",
                url,
                error
            );
            None
        }
    }
}

fn tmdb_image_url(
    path: &str,
    size: &str,
) -> String {
    format!(
        "{}/{}/{}",
        TMDB_IMAGE_BASE,
        size,
        path.trim_start_matches('/')
    )
}

fn provider_display_name(provider_id: &MetadataProviderId) -> &'static str {
    match provider_id {
        MetadataProviderId::Tmdb => "TMDB",
        MetadataProviderId::Tvdb => "TheTVDB",
        MetadataProviderId::MusicBrainz => "MusicBrainz",
        MetadataProviderId::OpenLibrary => "Open Library",
        MetadataProviderId::LocalNfo => "Local NFO",
    }
}

fn tmdb_season_external_id(
    show_external_id: &str,
    season_number: i32,
) -> String {
    format!("tv:{show_external_id}:season:{season_number}")
}

fn tmdb_episode_external_id(
    show_external_id: &str,
    season_number: i32,
    episode_number: i32,
) -> String {
    format!("tv:{show_external_id}:season:{season_number}:episode:{episode_number}")
}

fn extract_release_year(value: Option<String>) -> Option<i32> {
    value
        .as_deref()
        .and_then(|value| value.split('-').next())
        .and_then(|value| value.parse::<i32>().ok())
}

fn parse_movie_name(
    relative_path: &str,
    display_title: &str,
) -> ParsedMovieName {
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

    let preferred_source =
        if parent_name.eq_ignore_ascii_case(file_stem) || has_title_and_year(parent_name) {
            parent_name
        } else {
            file_stem
        };

    let tag_values = BRACED_TAG_REGEX
        .captures_iter(preferred_source)
        .chain(BRACED_TAG_REGEX.captures_iter(file_stem))
        .filter_map(|captures| {
            captures
                .get(1)
                .map(|value| value.as_str().trim().to_string())
        })
        .collect::<Vec<_>>();
    let tmdb_id = provider_tag_value(&tag_values, "tmdb");
    let tvdb_id = provider_tag_value(&tag_values, "tvdb");
    let imdb_id = provider_tag_value(&tag_values, "imdb");
    let year = movie_year_from_name(preferred_source)
        .or_else(|| movie_year_from_name(file_stem))
        .or_else(|| movie_year_from_name(display_title));

    let cleaned = cleanup_movie_title(preferred_source);
    let fallback = cleanup_movie_title(display_title);
    ParsedMovieName {
        title: if cleaned.is_empty() { fallback } else { cleaned },
        year,
        tmdb_id,
        tvdb_id,
        imdb_id,
    }
}

fn movie_year_from_name(value: &str) -> Option<i32> {
    PARENTHETICAL_YEAR_REGEX
        .captures(value)
        .or_else(|| {
            YEAR_REGEX.captures(value).filter(|captures| {
                captures
                    .get(1)
                    .map(|year| !value[..year.start()].trim().is_empty())
                    .unwrap_or(false)
            })
        })
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse::<i32>().ok())
}

fn has_title_and_year(value: &str) -> bool {
    movie_year_from_name(value).is_some()
}

fn provider_tag_value(
    tags: &[String],
    provider: &str,
) -> Option<String> {
    tags.iter().flat_map(|tag| tag.split(':')).find_map(|part| {
        let part = part.trim();
        let normalized = part.to_ascii_lowercase();
        for separator in ["-", ":", "_"] {
            let prefix = format!("{provider}{separator}");
            if normalized.starts_with(&prefix) {
                return Some(part[prefix.len()..].trim().to_string()).filter(|id| !id.is_empty());
            }
        }
        None
    })
}

fn show_search_query(
    relative_path: &str,
    display_title: &str,
) -> String {
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

    [
        display_title.to_string(),
        first_segment,
        folder_name,
    ]
    .into_iter()
    .map(|value| cleanup_movie_title(&value))
    .find(|value| !value.trim().is_empty())
    .unwrap_or_default()
}

fn cleanup_movie_title(value: &str) -> String {
    let without_tags = BRACED_TAG_REGEX.replace_all(value, " ");
    let without_split_suffix = SPLIT_SUFFIX_REGEX.replace(&without_tags, " ");
    let without_parenthetical_year = PARENTHETICAL_YEAR_REGEX.replace(&without_split_suffix, " ");
    let mut normalized = without_parenthetical_year.replace(['.', '_'], " ");
    if let Some(year_match) = YEAR_REGEX.find(&normalized) {
        if !normalized[..year_match.start()].trim().is_empty() {
            normalized = normalized[..year_match.start()].to_string();
        }
    }
    normalized = TITLE_COLON_DASH_REGEX
        .replace_all(&normalized, ": ")
        .to_string();
    normalized = NOISE_TOKEN_REGEX.replace_all(&normalized, " ").to_string();

    normalized
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|character: char| !character.is_ascii_alphanumeric())
        .to_string()
}

fn movie_match_score(
    parsed: &ParsedMovieName,
    result: &MetadataSearchResult,
) -> f64 {
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

fn artwork_url_extension(url: &str) -> String {
    let normalized = url.split(['?', '#']).next().unwrap_or(url);
    Path::new(normalized)
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("jpg")
        .to_ascii_lowercase()
}

fn stable_artwork_url_hash(url: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    hasher.finish()
}

fn purge_stale_cached_artwork_files(
    cache_dir: &Path,
    cache_key: &str,
    keep_file_name: Option<&str>,
) -> Result<(), String> {
    if !cache_dir.is_dir() {
        return Ok(());
    }

    let prefix = sanitize_cache_key(cache_key);
    for entry in fs::read_dir(cache_dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !file_name.starts_with(&prefix) {
            continue;
        }
        if keep_file_name == Some(file_name) {
            continue;
        }

        fs::remove_file(path).map_err(|error| error.to_string())?;
    }

    Ok(())
}

fn root_media_item_id(
    item_id: i32,
    items_by_id: &HashMap<i32, MediaItem>,
) -> Option<i32> {
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
                tvdb_id: None,
                imdb_id: None,
            }
        );
        assert_eq!(
            parse_movie_name(
                "Beyond The Sky (2018) - Bluray-1080p [tmdb-332718:tvdb-12345].mkv",
                "Beyond The Sky (2018) - Bluray-1080p"
            ),
            ParsedMovieName {
                title: "Beyond The Sky".into(),
                year: Some(2018),
                tmdb_id: Some("332718".into()),
                tvdb_id: Some("12345".into()),
                imdb_id: None,
            }
        );
        assert_eq!(
            parse_movie_name("2067 (2020) - 1080p.mkv", "2067 (2020) - 1080p"),
            ParsedMovieName {
                title: "2067".into(),
                year: Some(2020),
                tmdb_id: None,
                tvdb_id: None,
                imdb_id: None,
            }
        );
        assert_eq!(
            parse_movie_name("2067/2067 (2020) - 1080p.mkv", "2067"),
            ParsedMovieName {
                title: "2067".into(),
                year: Some(2020),
                tmdb_id: None,
                tvdb_id: None,
                imdb_id: None,
            }
        );
    }

    #[test]
    fn presentation_extracts_nested_tvdb_translated_description() {
        let link = ItemMetadataLink {
            id: 1,
            media_item_id: 1,
            provider_id: "tvdb".into(),
            external_id: "123".into(),
            title: Some("Example".into()),
            overview: None,
            tagline: None,
            artwork_url: None,
            backdrop_url: None,
            release_year: None,
            media_type: Some("movie".into()),
            relation_kind: "primary".into(),
            match_state: "linked".into(),
            provider_payload_json: Some(
                serde_json::json!({
                    "data": {
                        "translations": [
                            {
                                "language": "spa",
                                "overview": "Descripcion en espanol."
                            },
                            {
                                "language": "eng",
                                "description": "English TVDB description."
                            }
                        ]
                    }
                })
                .to_string(),
            ),
            logo_url: None,
            cached_logo_path: None,
            genres_json: None,
            rating: None,
            content_rating: None,
            trailer_title: None,
            trailer_url: None,
            locale_key: "en-US".into(),
            provider_locale_key: Some("eng".into()),
            cached_artwork_path: None,
            cached_backdrop_path: None,
            refresh_state: "fresh".into(),
            refresh_interval_seconds: 0,
            last_refreshed_at: None,
            next_refresh_at: None,
            refresh_error: None,
            updated_at: None,
        };

        assert_eq!(
            presentation_from_metadata_link(&link).overview.as_deref(),
            Some("English TVDB description.")
        );
    }

    #[test]
    fn presentation_ignores_tvdb_translation_names_when_overview_is_missing() {
        let link = ItemMetadataLink {
            id: 1,
            media_item_id: 1,
            provider_id: "tvdb".into(),
            external_id: "123".into(),
            title: Some("Example".into()),
            overview: Some("Stored overview wins.".into()),
            tagline: None,
            artwork_url: None,
            backdrop_url: None,
            release_year: None,
            media_type: Some("movie".into()),
            relation_kind: "primary".into(),
            match_state: "linked".into(),
            provider_payload_json: Some(
                serde_json::json!({
                    "data": {
                        "translations": [
                            {
                                "language": "rus",
                                "name": "rus"
                            }
                        ]
                    }
                })
                .to_string(),
            ),
            logo_url: None,
            cached_logo_path: None,
            genres_json: Some(serde_json::json!(["Drama", "Mystery"]).to_string()),
            rating: None,
            content_rating: None,
            trailer_title: None,
            trailer_url: None,
            locale_key: "en-US".into(),
            provider_locale_key: Some("eng".into()),
            cached_artwork_path: None,
            cached_backdrop_path: None,
            refresh_state: "fresh".into(),
            refresh_interval_seconds: 0,
            last_refreshed_at: None,
            next_refresh_at: None,
            refresh_error: None,
            updated_at: None,
        };

        let presentation = presentation_from_metadata_link(&link);
        assert_eq!(
            presentation.overview.as_deref(),
            Some("Stored overview wins.")
        );
        assert_eq!(presentation.genres, vec!["Drama", "Mystery"]);
    }

    #[test]
    fn movie_match_score_prefers_matching_year() {
        let parsed = ParsedMovieName {
            title: "The Matrix".into(),
            year: Some(1999),
            tmdb_id: None,
            tvdb_id: None,
            imdb_id: None,
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
            score: None,
        };
        let wrong_year = MetadataSearchResult {
            release_year: Some(2003),
            ..matching_year.clone()
        };

        assert!(
            movie_match_score(&parsed, &matching_year) > movie_match_score(&parsed, &wrong_year)
        );
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
