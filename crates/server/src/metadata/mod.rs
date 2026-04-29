//! Metadata-provider registry and persistence helpers.

// standard imports
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Duration;

// lib imports
use diesel::{
    ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl, SelectableHelper, SqliteConnection,
};
use once_cell::sync::Lazy;
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use strsim::normalized_levenshtein;

mod providers;
pub use providers::{MetadataProvider, MetadataRegistry};

// local imports
use crate::config::{
    MediaLibraryKind, MetadataProviderId, MetadataProviderSettings, MetadataSettings,
};
use crate::db::configure_sqlite_connection;
use crate::db::models::{
    ItemMetadataLink, MediaItem, MetadataCollection, MetadataCollectionItem, MetadataPerson,
    MetadataPersonCredit, NewItemMetadataExternalId, NewItemMetadataLink, NewItemMetadataPerson,
    NewMetadataCollection, NewMetadataCollectionItem, NewMetadataPerson, NewMetadataPersonCredit,
};
use crate::utils::current_timestamp;

const DEFAULT_METADATA_REFRESH_INTERVAL_SECONDS: i64 = 30 * 24 * 60 * 60;
const METADATA_RESPONSE_CACHE_TTL_SECONDS: i64 = 24 * 60 * 60;
/// Default Koko metadata locale used when no user preference is available.
pub const DEFAULT_METADATA_LOCALE: &str = "en-US";

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
    /// Whether this provider can be selected as primary metadata or extends another provider.
    pub role: MetadataProviderRole,
    /// Primary providers this secondary provider can extend.
    pub extends_provider_ids: Vec<MetadataProviderId>,
    /// Provider attribution text for UI display.
    pub attribution_text: String,
    /// Provider attribution link.
    pub attribution_url: String,
    /// Provider logo suitable for light backgrounds.
    pub logo_light_url: Option<String>,
    /// Provider logo suitable for dark backgrounds.
    pub logo_dark_url: Option<String>,
}

/// How a metadata provider participates in metadata acquisition.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetadataProviderRole {
    /// Provider can be the primary source of item metadata.
    Primary,
    /// Provider enriches metadata from one or more primary providers.
    Secondary,
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
    /// Whether this provider can be selected as primary metadata or extends another provider.
    pub role: MetadataProviderRole,
    /// Primary providers this secondary provider can extend.
    pub extends_provider_ids: Vec<MetadataProviderId>,
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
    /// Link relation such as `primary` or `secondary`.
    pub relation_kind: String,
    /// Current match state.
    pub match_state: String,
    /// Provider-supplied title logo URL, when available.
    pub logo_url: Option<String>,
    /// Cached title logo path, when available.
    pub cached_logo_path: Option<String>,
    /// Provider genre labels stored directly for querying and UI use.
    pub genres: Vec<String>,
    /// People credited by the provider, including cast and crew.
    pub people: Vec<ItemMetadataPersonSummary>,
    /// Provider-supplied user/community rating, when available.
    pub rating: Option<f32>,
    /// Provider-supplied content rating such as PG-13 or TV-MA, when available.
    pub content_rating: Option<String>,
    /// Human-friendly trailer title, when available.
    pub trailer_title: Option<String>,
    /// Trailer URL, when available.
    pub trailer_url: Option<String>,
    /// Theme-song URL, when supplied by provider metadata.
    pub theme_song_url: Option<String>,
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

/// Provider-neutral person credit linked to stored metadata.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ItemMetadataPersonSummary {
    /// Stable row identifier for the stored person credit.
    pub id: i32,
    /// Stable database identifier for this normalized person.
    pub person_id: i32,
    /// Provider-side person identifier, when available.
    pub external_id: Option<String>,
    /// Koko locale key for this localized person row.
    pub locale_key: String,
    /// Display name.
    pub name: String,
    /// Job or credit role such as Actor, Director, or Writer.
    pub role: Option<String>,
    /// High-level department such as Cast, Directing, or Writing.
    pub department: Option<String>,
    /// Character name for acting credits.
    pub character_name: Option<String>,
    /// Provider person page URL, when available.
    pub profile_url: Option<String>,
    /// Provider image URL, when available.
    pub image_url: Option<String>,
    /// Cached local image path, when available.
    pub cached_image_path: Option<String>,
    /// Provider/source order for stable presentation.
    pub sort_order: i32,
}

/// Normalized provider-scoped person stored in Koko.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct MetadataPersonSummary {
    /// Stable person identifier.
    pub id: i32,
    /// Provider identifier.
    pub provider_id: MetadataProviderId,
    /// Provider-side person identifier, when available.
    pub external_id: Option<String>,
    /// Koko locale key for this localized person row.
    pub locale_key: String,
    /// Display name.
    pub name: String,
    /// Titles this person is known for.
    pub known_for: Vec<String>,
    /// Provider biography or description.
    pub biography: Option<String>,
    /// Provider-neutral gender label, when known.
    pub gender: Option<String>,
    /// Birth date as provider-supplied ISO date, when known.
    pub birthday: Option<String>,
    /// Death date as provider-supplied ISO date, when known.
    pub deathday: Option<String>,
    /// Birth place, when known.
    pub birth_place: Option<String>,
    /// Provider person page URL, when available.
    pub profile_url: Option<String>,
    /// Provider image URL, when available.
    pub image_url: Option<String>,
    /// Cached local image path, when available.
    pub cached_image_path: Option<String>,
    /// Last update timestamp as Unix seconds, if available.
    pub updated_at: Option<i64>,
}

/// One credit connecting a normalized person to an item metadata link.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct MetadataPersonCreditSummary {
    /// Stable credit identifier.
    pub id: i32,
    /// Linked metadata row identifier.
    pub metadata_link_id: i32,
    /// Media item represented by the metadata row.
    pub media_item_id: i32,
    /// Job or credit role such as Actor, Director, or Writer.
    pub role: Option<String>,
    /// High-level department such as Cast, Directing, or Writing.
    pub department: Option<String>,
    /// Character name for acting credits.
    pub character_name: Option<String>,
    /// Provider/source order for stable presentation.
    pub sort_order: i32,
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
    /// Stable Koko collection identifier.
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
    /// Theme-song URL when available.
    pub theme_song_url: Option<String>,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Provider-normalized metadata fields that are persisted into Koko tables.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProviderMetadataDetails {
    /// External identifiers normalized into Koko's database.
    pub external_ids: Vec<ProviderExternalId>,
    /// Tagline or short promotional line.
    pub tagline: Option<String>,
    /// Provider-supplied title logo URL.
    pub logo_url: Option<String>,
    /// Provider genre labels.
    pub genres: Vec<String>,
    /// Provider-supplied user/community rating.
    pub rating: Option<f32>,
    /// Provider-supplied content rating such as PG-13 or TV-MA.
    pub content_rating: Option<String>,
    /// Human-friendly trailer title.
    pub trailer_title: Option<String>,
    /// Trailer URL.
    pub trailer_url: Option<String>,
    /// Theme-song URL.
    pub theme_song_url: Option<String>,
    /// Collections this metadata item belongs to.
    pub collections: Vec<ProviderMetadataCollection>,
    /// People credited by the provider.
    pub people: Vec<ProviderMetadataPerson>,
}

/// Provider-normalized external identifier for cross-provider lookups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderExternalId {
    /// Stable source/database key such as `imdb`, `tmdb`, or `thetvdb`.
    pub source: String,
    /// External identifier within the source/database.
    pub external_id: String,
}

/// Provider-normalized collection metadata ready for persistence.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderMetadataCollection {
    /// Provider-side collection identifier.
    pub external_id: String,
    /// Collection name.
    pub name: Option<String>,
    /// Collection overview.
    pub overview: Option<String>,
    /// Collection poster or artwork URL.
    pub artwork_url: Option<String>,
    /// Collection backdrop URL.
    pub backdrop_url: Option<String>,
    /// Collection theme-song URL.
    pub theme_song_url: Option<String>,
}

/// Provider-normalized person credit ready for persistence.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderMetadataPerson {
    /// Provider-side person identifier.
    pub external_id: Option<String>,
    /// Display name.
    pub name: String,
    /// Titles this person is known for.
    pub known_for: Vec<String>,
    /// Provider biography.
    pub biography: Option<String>,
    /// Provider-neutral gender label, when known.
    pub gender: Option<String>,
    /// Birth date as provider-supplied ISO date, when known.
    pub birthday: Option<String>,
    /// Death date as provider-supplied ISO date, when known.
    pub deathday: Option<String>,
    /// Birth place, when known.
    pub birth_place: Option<String>,
    /// Job or credit role.
    pub role: Option<String>,
    /// High-level department.
    pub department: Option<String>,
    /// Character name for acting credits.
    pub character_name: Option<String>,
    /// Provider person page URL.
    pub profile_url: Option<String>,
    /// Provider image URL.
    pub image_url: Option<String>,
    /// Cached local image path.
    pub cached_image_path: Option<String>,
    /// Provider/source order for stable presentation.
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetadataSnapshotCacheEntry {
    created_at: i64,
    snapshot: StoredMetadataSnapshot,
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
    /// Trailer URL, when available.
    pub trailer_url: Option<String>,
    /// Theme-song URL, when available.
    pub theme_song_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedMovieName {
    title: String,
    year: Option<i32>,
    provider_ids: HashMap<String, String>,
}

impl ParsedMovieName {
    fn provider_id(
        &self,
        provider: &str,
    ) -> Option<&str> {
        self.provider_ids
            .get(&provider.trim().to_ascii_lowercase())
            .map(String::as_str)
    }
}

static BRACED_TAG_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"[\{\[]([^\}\]]*)[\}\]]").unwrap());
static YEAR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(19\d{2}|20\d{2}|21\d{2})\b").unwrap());
static PARENTHETICAL_YEAR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[\(\[]\s*(19\d{2}|20\d{2}|21\d{2})\s*[\)\]]").unwrap());
static SPLIT_SUFFIX_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\s*[-–]\s*(cd|disc|disk|dvd|part|pt)\s*\d+\s*$").unwrap());
static TITLE_COLON_DASH_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s*-\s+").unwrap());
static YOUTUBE_VIDEO_ID_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[A-Za-z0-9_-]{11}$").unwrap());
static NOISE_TOKEN_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)\b(2160p|1080p|720p|480p|x264|x265|h264|h265|hevc|hdr|dv|webrip|web[- ]dl|bluray|brrip|dvdrip|remux|proper|repack|extended|unrated|criterion|aac|dts|truehd|atmos)\b",
    )
    .unwrap()
});

/// Extract a YouTube video id from a raw id, watch URL, short URL, embed URL, shorts URL, or live URL.
pub fn extract_youtube_video_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if YOUTUBE_VIDEO_ID_REGEX.is_match(trimmed) {
        return Some(trimmed.to_string());
    }

    let parse_target = if trimmed.starts_with("//youtube.com/")
        || trimmed.starts_with("//www.youtube.com/")
        || trimmed.starts_with("//youtu.be/")
    {
        format!("https:{trimmed}")
    } else if trimmed.starts_with("youtube.com/")
        || trimmed.starts_with("www.youtube.com/")
        || trimmed.starts_with("youtu.be/")
    {
        format!("https://{trimmed}")
    } else {
        trimmed.to_string()
    };
    let parsed = reqwest::Url::parse(&parse_target).ok()?;
    let host = parsed
        .host_str()?
        .trim_start_matches("www.")
        .to_ascii_lowercase();
    if host == "youtu.be" {
        return parsed
            .path_segments()
            .and_then(|mut segments| segments.next())
            .map(str::trim)
            .filter(|segment| YOUTUBE_VIDEO_ID_REGEX.is_match(segment))
            .map(ToOwned::to_owned);
    }

    let is_youtube_host = host == "youtube.com"
        || host.ends_with(".youtube.com")
        || host == "youtube-nocookie.com"
        || host.ends_with(".youtube-nocookie.com");
    if !is_youtube_host {
        return None;
    }

    if parsed.path() == "/watch" {
        return parsed
            .query_pairs()
            .find(|(key, _)| key == "v")
            .map(|(_, value)| value.trim().to_string())
            .filter(|video_id| YOUTUBE_VIDEO_ID_REGEX.is_match(video_id));
    }

    let mut segments = parsed.path_segments()?;
    match segments.next()? {
        "embed" | "shorts" | "live" => segments
            .next()
            .map(str::trim)
            .filter(|segment| YOUTUBE_VIDEO_ID_REGEX.is_match(segment))
            .map(ToOwned::to_owned),
        _ => None,
    }
}

/// Return a canonical YouTube watch URL for a raw YouTube id or URL.
pub fn youtube_watch_url(value: &str) -> Option<String> {
    extract_youtube_video_id(value)
        .map(|video_id| format!("https://www.youtube.com/watch?v={video_id}"))
}

/// Return a browser-embeddable YouTube URL for a raw YouTube id or URL.
pub fn youtube_embed_url(
    value: &str,
    autoplay: bool,
) -> Option<String> {
    extract_youtube_video_id(value).map(|video_id| {
        format!(
            "https://www.youtube.com/embed/{video_id}?autoplay={}&rel=0",
            if autoplay { 1 } else { 0 }
        )
    })
}

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
    let registry = MetadataRegistry::new();
    registry
        .provider(&provider_id)
        .map(|provider| provider.provider_locale_key(locale_key))
        .unwrap_or_else(|| normalize_locale_key(locale_key))
}

/// Remove provider metadata response cache files from the configured data directory.
pub fn clear_metadata_response_cache(data_dir: &str) -> Result<usize, String> {
    let cache_dir = metadata_response_cache_dir(data_dir);
    if !cache_dir.exists() {
        return Ok(0);
    }
    let count = count_files_recursive(&cache_dir)?;
    fs::remove_dir_all(&cache_dir).map_err(|error| error.to_string())?;
    Ok(count)
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
                role: descriptor.role,
                extends_provider_ids: descriptor.extends_provider_ids,
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

    rows.into_iter()
        .map(|row| to_item_metadata_summary_with_people(conn, row))
        .collect()
}

/// Sort stored metadata rows so the current user's preferred locale appears first.
pub fn sort_item_metadata_summaries_for_languages(
    summaries: &mut [ItemMetadataSummary],
    preferred_languages: &[String],
) {
    let rank = preferred_language_rank(preferred_languages);
    let fallback_rank = rank.len();
    summaries.sort_by(|left, right| {
        let left_rank = rank
            .get(&normalize_locale_key(&left.locale_key))
            .copied()
            .unwrap_or(fallback_rank);
        let right_rank = rank
            .get(&normalize_locale_key(&right.locale_key))
            .copied()
            .unwrap_or(fallback_rank);
        let left_relation_rank = if left.relation_kind == "primary" { 0 } else { 1 };
        let right_relation_rank = if right.relation_kind == "primary" { 0 } else { 1 };
        left_rank
            .cmp(&right_rank)
            .then_with(|| left_relation_rank.cmp(&right_relation_rank))
            .then_with(|| {
                left.provider_id
                    .as_storage_value()
                    .cmp(right.provider_id.as_storage_value())
            })
            .then_with(|| left.updated_at.cmp(&right.updated_at).reverse())
            .then_with(|| left.id.cmp(&right.id))
    });
}

/// Return one normalized metadata person.
pub fn get_metadata_person(
    conn: &mut SqliteConnection,
    person_id: i32,
) -> Result<Option<MetadataPersonSummary>, diesel::result::Error> {
    use crate::db::schema::metadata_people::dsl as people_dsl;

    people_dsl::metadata_people
        .filter(people_dsl::id.eq(person_id))
        .select(MetadataPerson::as_select())
        .first(conn)
        .optional()
        .map(|person| person.map(to_metadata_person_summary))
}

/// Return the best localized row for the same provider person as `person_id`.
pub fn get_metadata_person_for_languages(
    conn: &mut SqliteConnection,
    person_id: i32,
    preferred_languages: &[String],
) -> Result<Option<MetadataPersonSummary>, diesel::result::Error> {
    use crate::db::schema::metadata_people::dsl as people_dsl;

    let Some(source_person) = people_dsl::metadata_people
        .filter(people_dsl::id.eq(person_id))
        .select(MetadataPerson::as_select())
        .first(conn)
        .optional()?
    else {
        return Ok(None);
    };

    let rows = people_dsl::metadata_people
        .filter(people_dsl::provider_id.eq(&source_person.provider_id))
        .filter(people_dsl::identity_key.eq(&source_person.identity_key))
        .select(MetadataPerson::as_select())
        .load::<MetadataPerson>(conn)?;
    Ok(preferred_person_row(rows, preferred_languages).map(to_metadata_person_summary))
}

/// Return all localized person ids for the same provider person as `person_id`.
pub fn get_metadata_person_locale_peer_ids(
    conn: &mut SqliteConnection,
    person_id: i32,
) -> Result<Vec<i32>, diesel::result::Error> {
    use crate::db::schema::metadata_people::dsl as people_dsl;

    let Some(source_person) = people_dsl::metadata_people
        .filter(people_dsl::id.eq(person_id))
        .select(MetadataPerson::as_select())
        .first(conn)
        .optional()?
    else {
        return Ok(Vec::new());
    };

    people_dsl::metadata_people
        .filter(people_dsl::provider_id.eq(&source_person.provider_id))
        .filter(people_dsl::identity_key.eq(&source_person.identity_key))
        .select(people_dsl::id)
        .load::<i32>(conn)
}

/// Return all item credits for one normalized metadata person.
pub fn list_metadata_person_credit_summaries(
    conn: &mut SqliteConnection,
    person_id: i32,
) -> Result<Vec<MetadataPersonCreditSummary>, diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as link_dsl;
    use crate::db::schema::metadata_person_credits::dsl as credit_dsl;

    let rows = credit_dsl::metadata_person_credits
        .inner_join(link_dsl::item_metadata_links)
        .filter(credit_dsl::person_id.eq(person_id))
        .order((credit_dsl::sort_order.asc(), link_dsl::updated_at.desc()))
        .select((
            MetadataPersonCredit::as_select(),
            ItemMetadataLink::as_select(),
        ))
        .load::<(MetadataPersonCredit, ItemMetadataLink)>(conn)?;

    Ok(rows
        .into_iter()
        .map(|(credit, link)| MetadataPersonCreditSummary {
            id: credit.id,
            metadata_link_id: credit.metadata_link_id,
            media_item_id: link.media_item_id,
            role: credit.role,
            department: credit.department,
            character_name: credit.character_name,
            sort_order: credit.sort_order,
        })
        .collect())
}

/// Return all item credits for localized rows representing the same provider person.
pub fn list_metadata_person_credit_summaries_for_person_ids(
    conn: &mut SqliteConnection,
    person_ids: &[i32],
) -> Result<Vec<MetadataPersonCreditSummary>, diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as link_dsl;
    use crate::db::schema::metadata_person_credits::dsl as credit_dsl;

    if person_ids.is_empty() {
        return Ok(Vec::new());
    }

    let rows = credit_dsl::metadata_person_credits
        .inner_join(link_dsl::item_metadata_links)
        .filter(credit_dsl::person_id.eq_any(person_ids))
        .order((credit_dsl::sort_order.asc(), link_dsl::updated_at.desc()))
        .select((
            MetadataPersonCredit::as_select(),
            ItemMetadataLink::as_select(),
        ))
        .load::<(MetadataPersonCredit, ItemMetadataLink)>(conn)?;

    Ok(rows
        .into_iter()
        .map(|(credit, link)| MetadataPersonCreditSummary {
            id: credit.id,
            metadata_link_id: credit.metadata_link_id,
            media_item_id: link.media_item_id,
            role: credit.role,
            department: credit.department,
            character_name: credit.character_name,
            sort_order: credit.sort_order,
        })
        .collect())
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

/// Search one metadata provider using the current provider configuration.
pub async fn search_provider(
    settings: &MetadataSettings,
    provider_id: MetadataProviderId,
    query: &str,
    media_type: Option<&str>,
) -> Result<Vec<MetadataSearchResult>, String> {
    let registry = MetadataRegistry::new();
    let provider = registry.provider(&provider_id).ok_or_else(|| {
        format!(
            "{} search is not implemented.",
            provider_display_name(&provider_id)
        )
    })?;
    provider.search(settings, query, media_type).await
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
    let cache_key = metadata_response_cache_key(
        &provider_id,
        "item",
        &[
            external_id,
            media_type,
            &locale_key,
        ],
    );
    if let Some(snapshot) = read_metadata_snapshot_cache(&cache_key) {
        return Ok(snapshot);
    }

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

        let registry = MetadataRegistry::new();
        let result = match registry.provider(&provider_id) {
            Some(provider) => {
                provider
                    .fetch_snapshot(&localized_settings, external_id, media_type)
                    .await
            }
            None => Err(format!(
                "{} metadata fetch is not implemented.",
                provider_display_name(&provider_id)
            )),
        };

        match result {
            Ok(mut snapshot) if snapshot_has_presentable_metadata(&snapshot) => {
                snapshot.locale_key = locale_key;
                snapshot.provider_locale_key = Some(provider_locale);
                write_metadata_snapshot_cache(&cache_key, &snapshot);
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
    let provider_language = configured_provider_language(settings, &provider_id);
    let season_external_key = season_external_id.unwrap_or_default();
    let season_number_key = season_number.to_string();
    let cache_key = metadata_response_cache_key(
        &provider_id,
        "season",
        &[
            show_external_id,
            &season_number_key,
            season_external_key,
            &provider_language,
        ],
    );
    if let Some(snapshot) = read_metadata_snapshot_cache(&cache_key) {
        return Ok(snapshot);
    }

    let registry = MetadataRegistry::new();
    let provider = registry.provider(&provider_id).ok_or_else(|| {
        format!(
            "{} season metadata fetch is not implemented.",
            provider_display_name(&provider_id)
        )
    })?;
    let snapshot = provider
        .fetch_season_snapshot(
            settings,
            show_external_id,
            season_number,
            season_external_id,
        )
        .await?;
    write_metadata_snapshot_cache(&cache_key, &snapshot);
    Ok(snapshot)
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
    let provider_language = configured_provider_language(settings, &provider_id);
    let episode_external_key = episode_external_id.unwrap_or_default();
    let season_number_key = season_number.to_string();
    let episode_number_key = episode_number.to_string();
    let cache_key = metadata_response_cache_key(
        &provider_id,
        "episode",
        &[
            show_external_id,
            &season_number_key,
            &episode_number_key,
            episode_external_key,
            &provider_language,
        ],
    );
    if let Some(snapshot) = read_metadata_snapshot_cache(&cache_key) {
        return Ok(snapshot);
    }

    let registry = MetadataRegistry::new();
    let provider = registry.provider(&provider_id).ok_or_else(|| {
        format!(
            "{} episode metadata fetch is not implemented.",
            provider_display_name(&provider_id)
        )
    })?;
    let snapshot = provider
        .fetch_episode_snapshot(
            settings,
            show_external_id,
            season_number,
            episode_number,
            episode_external_id,
        )
        .await?;
    write_metadata_snapshot_cache(&cache_key, &snapshot);
    Ok(snapshot)
}

/// Guess the best provider movie match for one library item.
pub async fn guess_provider_movie_match(
    settings: &MetadataSettings,
    provider_id: MetadataProviderId,
    relative_path: &str,
    display_title: &str,
) -> Result<Option<MetadataSearchResult>, String> {
    let registry = MetadataRegistry::new();
    let Some(provider) = registry.provider(&provider_id) else {
        return Ok(None);
    };
    provider
        .guess_movie_match(settings, relative_path, display_title)
        .await
}

/// Guess the best provider show match for one show item.
pub async fn guess_provider_show_match(
    settings: &MetadataSettings,
    provider_id: MetadataProviderId,
    relative_path: &str,
    display_title: &str,
) -> Result<Option<MetadataSearchResult>, String> {
    let registry = MetadataRegistry::new();
    let Some(provider) = registry.provider(&provider_id) else {
        return Ok(None);
    };
    provider
        .guess_show_match(settings, relative_path, display_title)
        .await
}

/// Load provider descendant metadata targets for one linked show.
pub async fn load_provider_show_descendant_targets(
    settings: &MetadataSettings,
    provider_id: MetadataProviderId,
    show_external_id: &str,
) -> Result<Vec<ProviderDescendantTarget>, String> {
    let registry = MetadataRegistry::new();
    let provider = registry.provider(&provider_id).ok_or_else(|| {
        format!(
            "{} show descendant lookup is not implemented.",
            provider_display_name(&provider_id)
        )
    })?;
    provider
        .load_show_descendant_targets(settings, show_external_id)
        .await
}

/// Resolve item-level metadata fields contributed by a secondary provider.
pub async fn fetch_provider_secondary_metadata(
    provider_id: MetadataProviderId,
    media_type: &str,
    database_id: &str,
    external_id: &str,
    locale_key: &str,
) -> Result<Option<ProviderMetadataDetails>, String> {
    let registry = MetadataRegistry::new();
    let provider = registry.provider(&provider_id).ok_or_else(|| {
        format!(
            "{} secondary metadata lookup is not implemented.",
            provider_display_name(&provider_id)
        )
    })?;
    provider
        .fetch_secondary_metadata(media_type, database_id, external_id, locale_key)
        .await
}

/// Resolve collection-level metadata fields contributed by a secondary provider.
pub async fn fetch_provider_secondary_collection_metadata(
    provider_id: MetadataProviderId,
    media_type: &str,
    database_id: &str,
    external_id: &str,
    locale_key: &str,
) -> Result<Option<ProviderMetadataCollection>, String> {
    let registry = MetadataRegistry::new();
    let provider = registry.provider(&provider_id).ok_or_else(|| {
        format!(
            "{} secondary collection metadata lookup is not implemented.",
            provider_display_name(&provider_id)
        )
    })?;
    provider
        .fetch_secondary_collection_metadata(media_type, database_id, external_id, locale_key)
        .await
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
    let details = provider_metadata_details(snapshot);
    upsert_item_metadata_link(
        conn,
        item_id,
        snapshot,
        &details,
        "primary",
        refresh_interval_seconds,
    )
}

/// Upsert a stored metadata link for one media item.
pub fn upsert_item_metadata_link(
    conn: &mut SqliteConnection,
    item_id: i32,
    snapshot: &StoredMetadataSnapshot,
    details: &ProviderMetadataDetails,
    relation_kind: &str,
    refresh_interval_seconds: Option<i64>,
) -> Result<ItemMetadataSummary, diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as metadata_links_dsl;
    let relation_kind = relation_kind.trim();
    let relation_kind =
        if relation_kind.is_empty() { "primary" } else { relation_kind }.to_string();
    configure_sqlite_connection(conn)?;
    retry_sqlite_write(|| {
        let existing = metadata_links_dsl::item_metadata_links
            .filter(metadata_links_dsl::media_item_id.eq(item_id))
            .filter(metadata_links_dsl::provider_id.eq(snapshot.provider_id.as_storage_value()))
            .filter(metadata_links_dsl::relation_kind.eq(&relation_kind))
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
        let logo_url = details.logo_url.clone();
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

        let payload = NewItemMetadataLink {
            media_item_id: item_id,
            provider_id: snapshot.provider_id.as_storage_value().to_string(),
            external_id: snapshot.external_id.clone(),
            title: snapshot.title.clone(),
            overview: snapshot.overview.clone(),
            tagline: details.tagline.clone(),
            artwork_url: snapshot.artwork_url.clone(),
            backdrop_url: snapshot.backdrop_url.clone(),
            release_year: snapshot.release_year,
            media_type: snapshot.media_type.clone(),
            relation_kind: relation_kind.clone(),
            match_state: "linked".into(),
            logo_url,
            cached_logo_path: if keep_cached_logo {
                existing
                    .as_ref()
                    .and_then(|row| row.cached_logo_path.clone())
            } else {
                None
            },
            genres_json: serde_json::to_string(&details.genres).ok(),
            rating: details.rating,
            content_rating: details.content_rating.clone(),
            trailer_title: details.trailer_title.clone(),
            trailer_url: normalized_youtube_url(details.trailer_url.as_deref()),
            theme_song_url: normalized_youtube_url(details.theme_song_url.as_deref()),
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
            .filter(metadata_links_dsl::relation_kind.eq(&relation_kind))
            .filter(metadata_links_dsl::locale_key.eq(&snapshot.locale_key))
            .select(ItemMetadataLink::as_select())
            .first(conn)?;

        if relation_kind == "primary" {
            sync_item_metadata_collections(conn, row.id, row.media_item_id, snapshot, details)?;
            sync_item_metadata_external_ids(conn, row.id, snapshot, details)?;
            sync_item_metadata_people(conn, row.id, snapshot, details)?;
        }

        to_item_metadata_summary_with_people(conn, row)
    })
}

fn normalized_youtube_url(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| youtube_watch_url(value).unwrap_or_else(|| value.to_string()))
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
            logo_url: existing.as_ref().and_then(|row| row.logo_url.clone()),
            cached_logo_path: existing
                .as_ref()
                .and_then(|row| row.cached_logo_path.clone()),
            genres_json: existing.as_ref().and_then(|row| row.genres_json.clone()),
            rating: existing.as_ref().and_then(|row| row.rating),
            content_rating: existing.as_ref().and_then(|row| row.content_rating.clone()),
            trailer_title: existing.as_ref().and_then(|row| row.trailer_title.clone()),
            trailer_url: existing.as_ref().and_then(|row| row.trailer_url.clone()),
            theme_song_url: existing.as_ref().and_then(|row| row.theme_song_url.clone()),
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

        to_item_metadata_summary_with_people(conn, row)
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

fn provider_metadata_details(snapshot: &StoredMetadataSnapshot) -> ProviderMetadataDetails {
    let registry = MetadataRegistry::new();
    registry
        .provider(&snapshot.provider_id)
        .map(|provider| provider.metadata_details(snapshot))
        .unwrap_or_default()
}

/// Return collection summaries derived from stored metadata for the requested library scope.
pub fn list_metadata_collection_summaries(
    conn: &mut SqliteConnection,
    library_id: Option<i32>,
) -> Result<Vec<MetadataCollectionSummary>, diesel::result::Error> {
    list_metadata_collection_summaries_with_preferred_languages(conn, library_id, &[], &[])
}

/// Return collection summaries merged by provider order and preferred metadata locale.
pub fn list_metadata_collection_summaries_with_preferred_languages(
    conn: &mut SqliteConnection,
    library_id: Option<i32>,
    preferred_languages: &[String],
    provider_order: &[MetadataProviderId],
) -> Result<Vec<MetadataCollectionSummary>, diesel::result::Error> {
    use crate::db::schema::media_items::dsl as media_items_dsl;
    use crate::db::schema::metadata_collection_items::dsl as collection_items_dsl;
    use crate::db::schema::metadata_collections::dsl as collections_dsl;

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
    let item_ids = items_by_id.keys().copied().collect::<Vec<_>>();
    let collection_item_rows = collection_items_dsl::metadata_collection_items
        .filter(collection_items_dsl::media_item_id.eq_any(&item_ids))
        .select(MetadataCollectionItem::as_select())
        .load::<MetadataCollectionItem>(conn)?;
    if collection_item_rows.is_empty() {
        return Ok(Vec::new());
    }

    let collection_ids = collection_item_rows
        .iter()
        .map(|item| item.collection_id)
        .collect::<Vec<_>>();
    let collection_rows = collections_dsl::metadata_collections
        .filter(collections_dsl::id.eq_any(collection_ids))
        .select(MetadataCollection::as_select())
        .load::<MetadataCollection>(conn)?;
    let collections_by_id = collection_rows
        .into_iter()
        .map(|collection| (collection.id, collection))
        .collect::<HashMap<_, _>>();

    let mut grouped = HashMap::<(String, String), (Vec<MetadataCollection>, HashSet<i32>)>::new();
    for collection_item in collection_item_rows {
        let Some(collection) = collections_by_id.get(&collection_item.collection_id) else {
            continue;
        };
        let Some(root_id) = root_media_item_id(collection_item.media_item_id, &items_by_id) else {
            continue;
        };

        grouped
            .entry((
                collection.source_provider_id.clone(),
                collection.source_external_id.clone(),
            ))
            .and_modify(|(collections, item_ids)| {
                if !collections
                    .iter()
                    .any(|existing| existing.id == collection.id)
                {
                    collections.push(collection.clone());
                }
                item_ids.insert(root_id);
            })
            .or_insert_with(|| {
                let mut item_ids = HashSet::new();
                item_ids.insert(root_id);
                (vec![collection.clone()], item_ids)
            });
    }

    let provider_rank = provider_order
        .iter()
        .enumerate()
        .map(|(index, provider)| (provider.as_storage_value().to_string(), index))
        .collect::<HashMap<_, _>>();
    let fallback_provider_rank = provider_rank.len();
    let language_rank = preferred_language_rank(preferred_languages);
    let fallback_language_rank = language_rank.len();

    let mut summaries = grouped
        .into_values()
        .filter_map(|(mut collections, item_ids)| {
            collections.sort_by(|left, right| {
                let left_provider_rank = provider_rank
                    .get(&left.provider_id)
                    .copied()
                    .unwrap_or(fallback_provider_rank);
                let right_provider_rank = provider_rank
                    .get(&right.provider_id)
                    .copied()
                    .unwrap_or(fallback_provider_rank);
                let left_relation_rank = if left.relation_kind == "primary" { 0 } else { 1 };
                let right_relation_rank = if right.relation_kind == "primary" { 0 } else { 1 };
                let left_language_rank = language_rank
                    .get(&normalize_locale_key(&left.locale_key))
                    .copied()
                    .unwrap_or(fallback_language_rank);
                let right_language_rank = language_rank
                    .get(&normalize_locale_key(&right.locale_key))
                    .copied()
                    .unwrap_or(fallback_language_rank);

                left_provider_rank
                    .cmp(&right_provider_rank)
                    .then_with(|| left_relation_rank.cmp(&right_relation_rank))
                    .then_with(|| left_language_rank.cmp(&right_language_rank))
                    .then_with(|| right.updated_at.cmp(&left.updated_at))
                    .then_with(|| right.id.cmp(&left.id))
            });
            let primary = collections.first()?.clone();
            let mut item_ids = item_ids.into_iter().collect::<Vec<_>>();
            item_ids.sort_unstable();
            Some(MetadataCollectionSummary {
                id: format!(
                    "collection:{}:{}",
                    primary.source_provider_id, primary.source_external_id
                ),
                provider_id: metadata_provider_id_from_db(&primary.provider_id),
                external_id: primary.external_id.clone(),
                name: first_collection_string(&collections, |collection| collection.name.as_ref())
                    .unwrap_or_else(|| primary.source_external_id.clone()),
                overview: first_collection_string(&collections, |collection| {
                    collection.overview.as_ref()
                }),
                artwork_url: first_collection_string(&collections, |collection| {
                    collection.artwork_url.as_ref()
                }),
                backdrop_url: first_collection_string(&collections, |collection| {
                    collection.backdrop_url.as_ref()
                }),
                theme_song_url: first_collection_string(&collections, |collection| {
                    collection.theme_song_url.as_ref()
                }),
                item_count: item_ids.len(),
                item_ids,
            })
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(summaries)
}

fn first_collection_string<F>(
    collections: &[MetadataCollection],
    value: F,
) -> Option<String>
where
    F: Fn(&MetadataCollection) -> Option<&String>,
{
    collections.iter().find_map(|collection| {
        value(collection)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

/// Upsert a secondary collection metadata row with a provider-supplied theme-song URL.
pub fn upsert_secondary_collection_theme_song_url(
    conn: &mut SqliteConnection,
    source_collection_id: i32,
    provider_id: MetadataProviderId,
    media_type: &str,
    database_id: &str,
    external_id: &str,
    theme_song_url: &str,
) -> Result<(), diesel::result::Error> {
    use crate::db::schema::metadata_collection_items::dsl as collection_items_dsl;
    use crate::db::schema::metadata_collections::dsl as collections_dsl;

    let theme_song_url = theme_song_url.trim();
    if theme_song_url.is_empty() {
        return Ok(());
    }

    let source_collection = collections_dsl::metadata_collections
        .filter(collections_dsl::id.eq(source_collection_id))
        .select(MetadataCollection::as_select())
        .first::<MetadataCollection>(conn)?;
    let now = current_timestamp();
    let secondary_collection = ProviderMetadataCollection {
        external_id: format!("{media_type}:{database_id}:{external_id}"),
        name: None,
        overview: None,
        artwork_url: None,
        backdrop_url: None,
        theme_song_url: Some(theme_song_url.to_string()),
    };
    let secondary_collection_id = upsert_metadata_collection(
        conn,
        provider_id.as_storage_value(),
        &source_collection.source_provider_id,
        &source_collection.source_external_id,
        "secondary",
        DEFAULT_METADATA_LOCALE,
        None,
        secondary_collection,
        now,
    )?;

    diesel::delete(
        collection_items_dsl::metadata_collection_items
            .filter(collection_items_dsl::collection_id.eq(secondary_collection_id)),
    )
    .execute(conn)?;

    let source_collection_ids = collections_dsl::metadata_collections
        .filter(collections_dsl::source_provider_id.eq(&source_collection.source_provider_id))
        .filter(collections_dsl::source_external_id.eq(&source_collection.source_external_id))
        .filter(collections_dsl::relation_kind.eq("primary"))
        .select(collections_dsl::id)
        .load::<i32>(conn)?;
    let source_items = collection_items_dsl::metadata_collection_items
        .filter(collection_items_dsl::collection_id.eq_any(source_collection_ids))
        .select(MetadataCollectionItem::as_select())
        .load::<MetadataCollectionItem>(conn)?;

    let mut seen_items = HashSet::new();
    for source_item in source_items {
        if !seen_items.insert(source_item.media_item_id) {
            continue;
        }
        let row = NewMetadataCollectionItem {
            collection_id: secondary_collection_id,
            media_item_id: source_item.media_item_id,
            metadata_link_id: source_item.metadata_link_id,
            updated_at: Some(now),
        };
        diesel::insert_into(collection_items_dsl::metadata_collection_items)
            .values(&row)
            .execute(conn)?;
    }

    Ok(())
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

fn preferred_language_rank(preferred_languages: &[String]) -> HashMap<String, usize> {
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

fn preferred_person_row(
    rows: Vec<MetadataPerson>,
    preferred_languages: &[String],
) -> Option<MetadataPerson> {
    let rank = preferred_language_rank(preferred_languages);
    let fallback_rank = rank.len();
    rows.into_iter().min_by(|left, right| {
        let left_rank = rank
            .get(&normalize_locale_key(&left.locale_key))
            .copied()
            .unwrap_or(fallback_rank);
        let right_rank = rank
            .get(&normalize_locale_key(&right.locale_key))
            .copied()
            .unwrap_or(fallback_rank);
        left_rank
            .cmp(&right_rank)
            .then_with(|| left.updated_at.cmp(&right.updated_at).reverse())
            .then_with(|| left.id.cmp(&right.id))
    })
}

/// Extract presentation-ready metadata from a stored link payload.
pub fn presentation_from_metadata_link(link: &ItemMetadataLink) -> LinkedMetadataPresentation {
    let tagline = link.tagline.clone();
    let overview = link.overview.clone();
    let genres = link
        .genres_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .filter(|genres| !genres.is_empty())
        .unwrap_or_default();
    let release_year = link.release_year;
    let logo_url = link.logo_url.clone();
    let rating = link.rating;
    let content_rating = link.content_rating.clone();

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
        trailer_title: link.trailer_title.clone(),
        trailer_url: link.trailer_url.clone(),
        theme_song_url: link.theme_song_url.clone(),
    }
}

/// Extract merged presentation metadata from ordered stored links.
pub fn presentation_from_metadata_links(links: &[ItemMetadataLink]) -> LinkedMetadataPresentation {
    let mut merged = LinkedMetadataPresentation::default();
    for link in links {
        let presentation = presentation_from_metadata_link(link);
        if merged.tagline.is_none() {
            merged.tagline = presentation.tagline;
        }
        if merged.overview.is_none() {
            merged.overview = presentation.overview;
        }
        if merged.genres.is_empty() && !presentation.genres.is_empty() {
            merged.genres = presentation.genres;
        }
        if merged.release_year.is_none() {
            merged.release_year = presentation.release_year;
        }
        if merged.media_type.is_none() {
            merged.media_type = presentation.media_type;
        }
        merged.poster_available |= presentation.poster_available;
        merged.backdrop_available |= presentation.backdrop_available;
        if merged.logo_url.is_none() {
            merged.logo_url = presentation.logo_url;
        }
        if merged.rating.is_none() {
            merged.rating = presentation.rating;
        }
        if merged.content_rating.is_none() {
            merged.content_rating = presentation.content_rating;
        }
        if merged.trailer_title.is_none() {
            merged.trailer_title = presentation.trailer_title;
        }
        if merged.trailer_url.is_none() {
            merged.trailer_url = presentation.trailer_url;
        }
        if merged.theme_song_url.is_none() {
            merged.theme_song_url = presentation.theme_song_url;
        }
    }

    merged
}

/// Persist stored metadata payload and cached artwork into the managed item asset structure.
pub async fn persist_item_metadata_assets(
    snapshot: &StoredMetadataSnapshot,
    _item_id: i32,
    data_dir: &str,
) -> Result<(Option<PathBuf>, Option<PathBuf>, Option<PathBuf>), String> {
    persist_item_metadata_assets_with_logo(snapshot, _item_id, data_dir, None).await
}

/// Persist stored metadata artwork with an optional logo URL already loaded from the database.
pub async fn persist_item_metadata_assets_with_logo(
    snapshot: &StoredMetadataSnapshot,
    _item_id: i32,
    data_dir: &str,
    logo_url_override: Option<&str>,
) -> Result<(Option<PathBuf>, Option<PathBuf>, Option<PathBuf>), String> {
    let item_dir = managed_metadata_asset_dir(
        data_dir,
        snapshot.provider_id.clone(),
        &snapshot.external_id,
        snapshot.media_type.as_deref(),
        &snapshot.locale_key,
    );
    fs::create_dir_all(&item_dir).map_err(|error| error.to_string())?;

    let logo_url = logo_url_override
        .map(str::to_string)
        .or_else(|| provider_metadata_details(snapshot).logo_url);

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

/// Cache person artwork referenced by a metadata payload and return a snapshot with cached paths embedded.
pub async fn persist_metadata_people_assets(
    snapshot: &StoredMetadataSnapshot,
    data_dir: &str,
) -> Result<StoredMetadataSnapshot, String> {
    let registry = MetadataRegistry::new();
    let Some(provider) = registry.provider(&snapshot.provider_id) else {
        return Ok(snapshot.clone());
    };
    provider.cache_person_assets(snapshot, data_dir).await
}

/// Return the deterministic provider-uuid based asset path for metadata payloads.
pub fn managed_metadata_asset_dir(
    data_dir: &str,
    provider_id: MetadataProviderId,
    external_id: &str,
    media_type: Option<&str>,
    locale_key: &str,
) -> PathBuf {
    let registry = MetadataRegistry::new();
    let item_kind = registry
        .provider(&provider_id)
        .map(|provider| provider.metadata_item_kind(media_type))
        .unwrap_or(MetadataItemKind::Item);
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

/// Convert an absolute managed asset path into a data-dir-relative database value.
pub fn metadata_asset_db_path(
    data_dir: &str,
    path: &Path,
) -> String {
    let data_dir = Path::new(data_dir);
    let relative_path = path.strip_prefix(data_dir).unwrap_or(path);
    relative_path.to_string_lossy().replace('\\', "/")
}

/// Resolve a stored metadata asset path against the current data directory.
pub fn resolve_metadata_asset_db_path(
    data_dir: &str,
    stored_path: &str,
) -> PathBuf {
    let path = PathBuf::from(stored_path);
    if path.is_absolute() { path } else { Path::new(data_dir).join(path) }
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

fn metadata_response_cache_dir(data_dir: &str) -> PathBuf {
    Path::new(data_dir)
        .join("metadata")
        .join("cache")
        .join("responses")
}

pub(crate) fn metadata_response_cache_key(
    provider_id: &MetadataProviderId,
    kind: &str,
    parts: &[&str],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(provider_id.as_storage_value().as_bytes());
    hasher.update([0]);
    hasher.update(kind.as_bytes());
    for part in parts {
        hasher.update([0]);
        hasher.update(part.trim().as_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn metadata_response_cache_path(cache_key: &str) -> PathBuf {
    let data_dir = crate::config::current_settings().general.data_dir;
    let (shard, file_stem) = cache_key.split_at(1);
    metadata_response_cache_dir(&data_dir)
        .join(shard)
        .join(format!("{file_stem}.json"))
}

fn read_metadata_snapshot_cache(cache_key: &str) -> Option<StoredMetadataSnapshot> {
    let contents = read_metadata_response_cache_text(cache_key)?;
    let entry = serde_json::from_str::<MetadataSnapshotCacheEntry>(&contents).ok()?;
    Some(entry.snapshot)
}

fn write_metadata_snapshot_cache(
    cache_key: &str,
    snapshot: &StoredMetadataSnapshot,
) {
    let entry = MetadataSnapshotCacheEntry {
        created_at: current_timestamp(),
        snapshot: snapshot.clone(),
    };
    if let Ok(contents) = serde_json::to_string(&entry) {
        write_metadata_response_cache_text(cache_key, &contents);
    }
}

pub(crate) fn read_metadata_response_cache_text(cache_key: &str) -> Option<String> {
    let path = metadata_response_cache_path(cache_key);
    let contents = fs::read_to_string(&path).ok()?;
    let created_at = fs::metadata(&path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.elapsed().ok())
        .and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
        .map(|age| current_timestamp().saturating_sub(age))
        .unwrap_or_else(current_timestamp);
    let age = current_timestamp().saturating_sub(created_at);
    if age > METADATA_RESPONSE_CACHE_TTL_SECONDS {
        let _ = fs::remove_file(path);
        return None;
    }
    Some(contents)
}

pub(crate) fn write_metadata_response_cache_text(
    cache_key: &str,
    contents: &str,
) {
    let path = metadata_response_cache_path(cache_key);
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let _ = fs::write(path, contents);
}

fn count_files_recursive(path: &Path) -> Result<usize, String> {
    if !path.exists() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            count += count_files_recursive(&entry_path)?;
        } else {
            count += 1;
        }
    }
    Ok(count)
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
    data_dir: &str,
) -> Result<(), diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as metadata_links_dsl;
    configure_sqlite_connection(conn)?;
    let stored_cache_path = metadata_asset_db_path(data_dir, cache_path);
    retry_sqlite_write(|| {
        match kind {
            ArtworkKind::Poster => {
                diesel::update(
                    metadata_links_dsl::item_metadata_links
                        .filter(metadata_links_dsl::id.eq(link_id)),
                )
                .set(metadata_links_dsl::cached_artwork_path.eq(stored_cache_path.clone()))
                .execute(conn)?;
            }
            ArtworkKind::Backdrop => {
                diesel::update(
                    metadata_links_dsl::item_metadata_links
                        .filter(metadata_links_dsl::id.eq(link_id)),
                )
                .set(metadata_links_dsl::cached_backdrop_path.eq(stored_cache_path.clone()))
                .execute(conn)?;
            }
            ArtworkKind::Logo => {
                diesel::update(
                    metadata_links_dsl::item_metadata_links
                        .filter(metadata_links_dsl::id.eq(link_id)),
                )
                .set(metadata_links_dsl::cached_logo_path.eq(stored_cache_path.clone()))
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

/// Provider-side season and episode identifiers resolved for one show descendant.
#[derive(Debug, Clone)]
pub struct ProviderDescendantTarget {
    /// Local season number for matching persisted items.
    pub season_number: i32,
    /// Local episode number for matching persisted items.
    pub episode_number: i32,
    /// Provider-side season identifier.
    pub season_external_id: String,
    /// Provider-side episode identifier.
    pub episode_external_id: String,
}

fn provider_settings(
    settings: &MetadataSettings,
    provider_id: MetadataProviderId,
) -> Result<MetadataProviderSettings, String> {
    let provider = settings
        .providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .cloned()
        .ok_or_else(|| "is not configured.".to_string())?;

    let requires_api_key = MetadataRegistry::new()
        .provider(&provider_id)
        .map(|provider| provider.descriptor().requires_api_key)
        .unwrap_or(true);
    let api_key_missing = provider
        .api_key
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .is_empty();
    if requires_api_key && api_key_missing {
        return Err("requires an API key but none is configured.".into());
    }

    Ok(provider)
}

fn configured_provider_language(
    settings: &MetadataSettings,
    provider_id: &MetadataProviderId,
) -> String {
    settings
        .providers
        .iter()
        .find(|provider| provider.id == *provider_id)
        .map(|provider| provider.language.clone())
        .unwrap_or_else(|| DEFAULT_METADATA_LOCALE.to_string())
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
        relation_kind: link.relation_kind,
        match_state: link.match_state,
        logo_url: link.logo_url,
        cached_logo_path: link.cached_logo_path,
        genres: link
            .genres_json
            .as_deref()
            .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
            .unwrap_or_default(),
        people: Vec::new(),
        rating: link.rating,
        content_rating: link.content_rating,
        trailer_title: link.trailer_title,
        trailer_url: link.trailer_url,
        theme_song_url: link.theme_song_url,
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

fn to_item_metadata_summary_with_people(
    conn: &mut SqliteConnection,
    link: ItemMetadataLink,
) -> Result<ItemMetadataSummary, diesel::result::Error> {
    use crate::db::schema::metadata_people::dsl as people_dsl;
    use crate::db::schema::metadata_person_credits::dsl as credit_dsl;

    let people = credit_dsl::metadata_person_credits
        .inner_join(people_dsl::metadata_people)
        .filter(credit_dsl::metadata_link_id.eq(link.id))
        .order(credit_dsl::sort_order.asc())
        .select((
            MetadataPersonCredit::as_select(),
            MetadataPerson::as_select(),
        ))
        .load::<(MetadataPersonCredit, MetadataPerson)>(conn)?
        .into_iter()
        .map(to_item_metadata_person_summary)
        .collect();
    let mut summary = to_item_metadata_summary(link);
    summary.people = people;
    Ok(summary)
}

fn to_item_metadata_person_summary(
    (credit, person): (MetadataPersonCredit, MetadataPerson)
) -> ItemMetadataPersonSummary {
    ItemMetadataPersonSummary {
        id: credit.id,
        person_id: person.id,
        external_id: person.external_id,
        locale_key: person.locale_key,
        name: person.name,
        role: credit.role,
        department: credit.department,
        character_name: credit.character_name,
        profile_url: person.profile_url,
        image_url: person.image_url,
        cached_image_path: person.cached_image_path,
        sort_order: credit.sort_order,
    }
}

fn to_metadata_person_summary(person: MetadataPerson) -> MetadataPersonSummary {
    MetadataPersonSummary {
        id: person.id,
        provider_id: metadata_provider_id_from_db(&person.provider_id),
        external_id: person.external_id,
        locale_key: person.locale_key,
        name: person.name,
        known_for: person
            .known_for_json
            .as_deref()
            .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
            .unwrap_or_default(),
        biography: person.biography,
        gender: person.gender,
        birthday: person.birthday,
        deathday: person.deathday,
        birth_place: person.birth_place,
        profile_url: person.profile_url,
        image_url: person.image_url,
        cached_image_path: person.cached_image_path,
        updated_at: person.updated_at,
    }
}

pub(crate) async fn try_cache_item_artwork(
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

fn provider_display_name(provider_id: &MetadataProviderId) -> String {
    let registry = MetadataRegistry::new();
    registry
        .provider(provider_id)
        .map(|provider| provider.descriptor().display_name)
        .unwrap_or_else(|| provider_id.as_storage_value().to_string())
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
    let provider_ids = provider_tag_values(&tag_values);
    let year = movie_year_from_name(preferred_source)
        .or_else(|| movie_year_from_name(file_stem))
        .or_else(|| movie_year_from_name(display_title));

    let cleaned = cleanup_movie_title(preferred_source);
    let fallback = cleanup_movie_title(display_title);
    ParsedMovieName {
        title: if cleaned.is_empty() { fallback } else { cleaned },
        year,
        provider_ids,
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

fn provider_tag_values(tags: &[String]) -> HashMap<String, String> {
    tags.iter()
        .flat_map(|tag| tag.split(':'))
        .filter_map(provider_tag_value)
        .collect()
}

fn provider_tag_value(part: &str) -> Option<(String, String)> {
    let part = part.trim();
    for separator in ["-", ":", "_"] {
        let Some((provider, external_id)) = part.split_once(separator) else {
            continue;
        };
        let provider = provider.trim().to_ascii_lowercase();
        let external_id = external_id.trim().to_string();
        if !provider.is_empty()
            && !external_id.is_empty()
            && !external_id.chars().any(char::is_whitespace)
        {
            return Some((provider, external_id));
        }
    }
    None
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
    media_item_id: i32,
    snapshot: &StoredMetadataSnapshot,
    details: &ProviderMetadataDetails,
) -> Result<(), diesel::result::Error> {
    use crate::db::schema::metadata_collection_items::dsl as collection_items_dsl;
    use crate::db::schema::metadata_collections::dsl as collections_dsl;

    let provider_id = snapshot.provider_id.as_storage_value().to_string();
    let existing_collection_ids = collections_dsl::metadata_collections
        .filter(collections_dsl::provider_id.eq(&provider_id))
        .filter(collections_dsl::relation_kind.eq("primary"))
        .filter(collections_dsl::locale_key.eq(&snapshot.locale_key))
        .select(collections_dsl::id)
        .load::<i32>(conn)?;
    if !existing_collection_ids.is_empty() {
        diesel::delete(
            collection_items_dsl::metadata_collection_items
                .filter(collection_items_dsl::media_item_id.eq(media_item_id))
                .filter(collection_items_dsl::collection_id.eq_any(existing_collection_ids)),
        )
        .execute(conn)?;
    }

    if details.collections.is_empty() {
        return Ok(());
    }

    let mut seen_collection_ids = HashSet::new();
    let now = current_timestamp();
    for collection in details.collections.iter().cloned() {
        let collection_external_id = collection.external_id.clone();
        let collection_id = upsert_metadata_collection(
            conn,
            &provider_id,
            &provider_id,
            &collection_external_id,
            "primary",
            &snapshot.locale_key,
            snapshot.provider_locale_key.clone(),
            collection,
            now,
        )?;
        if !seen_collection_ids.insert(collection_id) {
            continue;
        }
        let row = NewMetadataCollectionItem {
            collection_id,
            media_item_id,
            metadata_link_id,
            updated_at: Some(now),
        };
        diesel::insert_into(collection_items_dsl::metadata_collection_items)
            .values(&row)
            .execute(conn)?;
    }

    Ok(())
}

fn upsert_metadata_collection(
    conn: &mut SqliteConnection,
    provider_id: &str,
    source_provider_id: &str,
    source_external_id: &str,
    relation_kind: &str,
    locale_key: &str,
    provider_locale_key: Option<String>,
    collection: ProviderMetadataCollection,
    now: i64,
) -> Result<i32, diesel::result::Error> {
    use crate::db::schema::metadata_collections::dsl as collections_dsl;

    let existing = collections_dsl::metadata_collections
        .filter(collections_dsl::provider_id.eq(provider_id))
        .filter(collections_dsl::external_id.eq(&collection.external_id))
        .filter(collections_dsl::relation_kind.eq(relation_kind))
        .filter(collections_dsl::locale_key.eq(locale_key))
        .select(MetadataCollection::as_select())
        .first::<MetadataCollection>(conn)
        .optional()?;

    let collection_name = collection
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let should_retain_existing_name = relation_kind == "primary" && collection_name.is_none();
    let payload = NewMetadataCollection {
        provider_id: provider_id.to_string(),
        external_id: collection.external_id.clone(),
        source_provider_id: source_provider_id.to_string(),
        source_external_id: source_external_id.to_string(),
        relation_kind: relation_kind.to_string(),
        locale_key: locale_key.to_string(),
        provider_locale_key,
        name: if should_retain_existing_name {
            existing.as_ref().and_then(|row| row.name.clone())
        } else {
            collection_name
        },
        overview: collection
            .overview
            .or_else(|| existing.as_ref().and_then(|row| row.overview.clone())),
        artwork_url: collection
            .artwork_url
            .or_else(|| existing.as_ref().and_then(|row| row.artwork_url.clone())),
        backdrop_url: collection
            .backdrop_url
            .or_else(|| existing.as_ref().and_then(|row| row.backdrop_url.clone())),
        theme_song_url: collection
            .theme_song_url
            .or_else(|| existing.as_ref().and_then(|row| row.theme_song_url.clone())),
        updated_at: Some(now),
    };

    if let Some(existing) = existing {
        diesel::update(
            collections_dsl::metadata_collections.filter(collections_dsl::id.eq(existing.id)),
        )
        .set(&payload)
        .execute(conn)?;
        Ok(existing.id)
    } else {
        diesel::insert_into(collections_dsl::metadata_collections)
            .values(&payload)
            .execute(conn)?;
        collections_dsl::metadata_collections
            .filter(collections_dsl::provider_id.eq(provider_id))
            .filter(collections_dsl::external_id.eq(&collection.external_id))
            .filter(collections_dsl::relation_kind.eq(relation_kind))
            .filter(collections_dsl::locale_key.eq(locale_key))
            .select(collections_dsl::id)
            .first::<i32>(conn)
    }
}

fn sync_item_metadata_external_ids(
    conn: &mut SqliteConnection,
    metadata_link_id: i32,
    snapshot: &StoredMetadataSnapshot,
    details: &ProviderMetadataDetails,
) -> Result<(), diesel::result::Error> {
    use crate::db::schema::item_metadata_external_ids::dsl as external_ids_dsl;

    diesel::delete(
        external_ids_dsl::item_metadata_external_ids
            .filter(external_ids_dsl::metadata_link_id.eq(metadata_link_id)),
    )
    .execute(conn)?;

    let mut rows = Vec::new();
    let mut seen = HashSet::new();
    let now = current_timestamp();

    let primary_external_id = ProviderExternalId {
        source: snapshot.provider_id.as_storage_value().to_string(),
        external_id: snapshot.external_id.clone(),
    };
    for external_id in details
        .external_ids
        .iter()
        .chain(std::iter::once(&primary_external_id))
    {
        let source = external_id.source.trim().to_ascii_lowercase();
        let external_id = external_id.external_id.trim().to_string();
        if source.is_empty() || external_id.is_empty() || !seen.insert(source.clone()) {
            continue;
        }
        rows.push(NewItemMetadataExternalId {
            metadata_link_id,
            source,
            external_id,
            updated_at: Some(now),
        });
    }

    if !rows.is_empty() {
        diesel::insert_into(external_ids_dsl::item_metadata_external_ids)
            .values(&rows)
            .execute(conn)?;
    }

    Ok(())
}

fn sync_item_metadata_people(
    conn: &mut SqliteConnection,
    metadata_link_id: i32,
    snapshot: &StoredMetadataSnapshot,
    details: &ProviderMetadataDetails,
) -> Result<(), diesel::result::Error> {
    use crate::db::schema::item_metadata_people::dsl as people_dsl;
    use crate::db::schema::metadata_people::dsl as normalized_people_dsl;
    use crate::db::schema::metadata_person_credits::dsl as credit_dsl;

    diesel::delete(
        people_dsl::item_metadata_people.filter(people_dsl::metadata_link_id.eq(metadata_link_id)),
    )
    .execute(conn)?;
    diesel::delete(
        credit_dsl::metadata_person_credits
            .filter(credit_dsl::metadata_link_id.eq(metadata_link_id)),
    )
    .execute(conn)?;

    let people = details.people.clone();
    if people.is_empty() {
        return Ok(());
    }

    let rows = people
        .iter()
        .cloned()
        .map(|person| NewItemMetadataPerson {
            metadata_link_id,
            external_id: person.external_id,
            name: person.name,
            role: person.role,
            department: person.department,
            character_name: person.character_name,
            profile_url: person.profile_url,
            image_url: person.image_url,
            sort_order: person.sort_order,
        })
        .collect::<Vec<_>>();

    diesel::insert_into(people_dsl::item_metadata_people)
        .values(&rows)
        .execute(conn)?;

    for person in people {
        let identity_key = person_identity_key(&person);
        let provider_id = snapshot.provider_id.as_storage_value().to_string();
        let payload = NewMetadataPerson {
            provider_id: provider_id.clone(),
            external_id: person.external_id.clone(),
            identity_key: identity_key.clone(),
            locale_key: snapshot.locale_key.clone(),
            name: person.name.clone(),
            known_for_json: serde_json::to_string(&person.known_for).ok(),
            biography: person.biography.clone(),
            gender: person.gender.clone(),
            birthday: person.birthday.clone(),
            deathday: person.deathday.clone(),
            birth_place: person.birth_place.clone(),
            profile_url: person.profile_url.clone(),
            image_url: person.image_url.clone(),
            cached_image_path: person.cached_image_path.clone(),
            updated_at: Some(current_timestamp()),
        };
        let existing_person = normalized_people_dsl::metadata_people
            .filter(normalized_people_dsl::provider_id.eq(&provider_id))
            .filter(normalized_people_dsl::identity_key.eq(&identity_key))
            .filter(normalized_people_dsl::locale_key.eq(&snapshot.locale_key))
            .select(MetadataPerson::as_select())
            .first(conn)
            .optional()?;
        let normalized_person = if let Some(existing_person) = existing_person {
            diesel::update(
                normalized_people_dsl::metadata_people
                    .filter(normalized_people_dsl::id.eq(existing_person.id)),
            )
            .set(&payload)
            .execute(conn)?;
            normalized_people_dsl::metadata_people
                .filter(normalized_people_dsl::id.eq(existing_person.id))
                .select(MetadataPerson::as_select())
                .first(conn)?
        } else {
            diesel::insert_into(normalized_people_dsl::metadata_people)
                .values(&payload)
                .execute(conn)?;
            normalized_people_dsl::metadata_people
                .filter(normalized_people_dsl::provider_id.eq(&provider_id))
                .filter(normalized_people_dsl::identity_key.eq(&identity_key))
                .filter(normalized_people_dsl::locale_key.eq(&snapshot.locale_key))
                .select(MetadataPerson::as_select())
                .first(conn)?
        };

        diesel::insert_into(credit_dsl::metadata_person_credits)
            .values(&NewMetadataPersonCredit {
                metadata_link_id,
                person_id: normalized_person.id,
                role: person.role,
                department: person.department,
                character_name: person.character_name,
                sort_order: person.sort_order,
            })
            .execute(conn)?;
    }

    Ok(())
}

fn person_identity_key(person: &ProviderMetadataPerson) -> String {
    person
        .external_id
        .as_deref()
        .map(str::trim)
        .filter(|external_id| !external_id.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("name:{}", person.name.trim().to_ascii_lowercase()))
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
                provider_ids: HashMap::from([("tmdb".into(), "78".into())]),
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
                provider_ids: HashMap::from([
                    ("tmdb".into(), "332718".into()),
                    ("tvdb".into(), "12345".into()),
                ]),
            }
        );
        assert_eq!(
            parse_movie_name("2067 (2020) - 1080p.mkv", "2067 (2020) - 1080p"),
            ParsedMovieName {
                title: "2067".into(),
                year: Some(2020),
                provider_ids: HashMap::new(),
            }
        );
        assert_eq!(
            parse_movie_name("2067/2067 (2020) - 1080p.mkv", "2067"),
            ParsedMovieName {
                title: "2067".into(),
                year: Some(2020),
                provider_ids: HashMap::new(),
            }
        );
    }

    #[test]
    fn presentation_uses_only_stored_database_fields() {
        let link = ItemMetadataLink {
            id: 1,
            media_item_id: 1,
            provider_id: "unknown".into(),
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
            logo_url: None,
            cached_logo_path: None,
            genres_json: Some(serde_json::json!(["Drama", "Mystery"]).to_string()),
            rating: None,
            content_rating: None,
            trailer_title: None,
            trailer_url: None,
            theme_song_url: None,
            locale_key: "en-US".into(),
            provider_locale_key: None,
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
    fn youtube_helpers_accept_common_video_url_shapes() {
        assert_eq!(
            extract_youtube_video_id("https://www.youtube.com/watch?v=SLBACEP6LsI&t=4s").as_deref(),
            Some("SLBACEP6LsI")
        );
        assert_eq!(
            extract_youtube_video_id("https://youtu.be/SLBACEP6LsI").as_deref(),
            Some("SLBACEP6LsI")
        );
        assert_eq!(
            extract_youtube_video_id("www.youtube.com/watch?v=SLBACEP6LsI").as_deref(),
            Some("SLBACEP6LsI")
        );
        assert_eq!(
            extract_youtube_video_id("https://www.youtube.com/embed/SLBACEP6LsI?rel=0").as_deref(),
            Some("SLBACEP6LsI")
        );
        assert_eq!(
            youtube_watch_url("https://www.youtube.com/shorts/SLBACEP6LsI").as_deref(),
            Some("https://www.youtube.com/watch?v=SLBACEP6LsI")
        );
        assert_eq!(
            youtube_embed_url("SLBACEP6LsI", true).as_deref(),
            Some("https://www.youtube.com/embed/SLBACEP6LsI?autoplay=1&rel=0")
        );
        assert_eq!(
            extract_youtube_video_id("https://example.com/watch?v=SLBACEP6LsI"),
            None
        );
    }

    #[test]
    fn movie_match_score_prefers_matching_year() {
        let parsed = ParsedMovieName {
            title: "The Matrix".into(),
            year: Some(1999),
            provider_ids: HashMap::new(),
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
}
