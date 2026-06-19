//! Configuration module for the application.

// standard imports
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

// lib imports
use config::{Config, ConfigError, Environment, File};
use diesel::prelude::*;
use dirs::config_local_dir;
use once_cell::sync::Lazy;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

// local imports
use crate::db::models::AppSetting;
use crate::db::schema::app_settings;
use crate::globals::GLOBAL_APP_NAME;

const METADATA_SETTINGS_KEY: &str = "metadata";
const MEDIA_SETTINGS_KEY: &str = "media";
const SERVER_SETTINGS_KEY: &str = "server";
const FFMPEG_SETTINGS_KEY: &str = "ffmpeg";
const SCHEDULED_TASKS_SETTINGS_KEY: &str = "scheduled_tasks";

/// General settings.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub struct GeneralSettings {
    /// The directory where application data is stored.
    #[serde(default)]
    pub data_dir: String,
}

/// Supported library categories for configured media roots.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MediaLibraryKind {
    /// Mixed content when the library is not limited to a single media type.
    #[default]
    Mixed,
    /// Feature films and similar long-form video content.
    Movies,
    /// Episodic television or serialized video content.
    Shows,
    /// Music, albums, and other audio-focused content.
    Music,
    /// Photos and other image collections.
    Photos,
    /// Books, comics, PDFs, and other reading material.
    Books,
    /// Home videos and other personal recordings.
    HomeVideos,
}

/// Scanner implementation used to inventory a media library.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MediaLibraryScanner {
    /// Choose the scanner that matches the library type.
    #[default]
    Auto,
    /// Generic directory scanner.
    Directory,
    /// Movie scanner.
    Movies,
    /// TV show scanner.
    Shows,
    /// Music scanner.
    Music,
    /// Photo scanner.
    Photos,
    /// Book scanner.
    Books,
}

impl MediaLibraryScanner {
    /// Return the concrete scanner used for the given library kind.
    pub fn effective_for_kind(
        &self,
        kind: &MediaLibraryKind,
    ) -> Self {
        match self {
            MediaLibraryScanner::Auto => Self::default_for_kind(kind),
            scanner => scanner.clone(),
        }
    }

    /// Return the default scanner for a library kind.
    pub fn default_for_kind(kind: &MediaLibraryKind) -> Self {
        match kind {
            MediaLibraryKind::Movies => MediaLibraryScanner::Movies,
            MediaLibraryKind::Shows => MediaLibraryScanner::Shows,
            MediaLibraryKind::Music => MediaLibraryScanner::Music,
            MediaLibraryKind::Photos => MediaLibraryScanner::Photos,
            MediaLibraryKind::Books => MediaLibraryScanner::Books,
            MediaLibraryKind::Mixed | MediaLibraryKind::HomeVideos => {
                MediaLibraryScanner::Directory
            }
        }
    }

    /// Return the stable storage representation.
    pub fn as_storage_value(&self) -> &'static str {
        match self {
            MediaLibraryScanner::Auto => "auto",
            MediaLibraryScanner::Directory => "directory",
            MediaLibraryScanner::Movies => "movies",
            MediaLibraryScanner::Shows => "shows",
            MediaLibraryScanner::Music => "music",
            MediaLibraryScanner::Photos => "photos",
            MediaLibraryScanner::Books => "books",
        }
    }

    /// Parse a scanner storage value.
    pub fn from_storage_value(value: &str) -> Self {
        match value.trim() {
            "directory" => MediaLibraryScanner::Directory,
            "movies" => MediaLibraryScanner::Movies,
            "shows" => MediaLibraryScanner::Shows,
            "music" => MediaLibraryScanner::Music,
            "photos" => MediaLibraryScanner::Photos,
            "books" => MediaLibraryScanner::Books,
            _ => MediaLibraryScanner::Auto,
        }
    }
}

fn default_recursive_scan() -> bool {
    true
}

fn default_ffmpeg_path() -> String {
    "ffmpeg".into()
}

fn default_ffprobe_path() -> String {
    "ffprobe".into()
}

fn default_metadata_language() -> String {
    "en-US".into()
}

fn default_metadata_languages() -> Vec<String> {
    vec![default_metadata_language()]
}

fn default_provider_enabled() -> bool {
    true
}

fn default_provider_rate_limit_per_second() -> u32 {
    4
}

fn default_provider_retry_attempts() -> u32 {
    3
}

fn default_provider_retry_backoff_ms() -> u32 {
    1_000
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn default_metadata_refresh_interval_days() -> Option<u32> {
    Some(30)
}

fn default_missing_item_auto_delete_days() -> Option<u32> {
    None
}

fn default_trash_cleanup_enabled() -> bool {
    false
}

fn default_scheduled_tasks_enabled() -> bool {
    true
}

fn default_scheduled_task_window_start() -> String {
    "02:00".into()
}

fn default_scheduled_task_window_stop() -> String {
    "06:00".into()
}

fn default_scheduled_task_weekdays() -> Vec<ScheduledTaskWeekday> {
    vec![
        ScheduledTaskWeekday::Monday,
        ScheduledTaskWeekday::Tuesday,
        ScheduledTaskWeekday::Wednesday,
        ScheduledTaskWeekday::Thursday,
        ScheduledTaskWeekday::Friday,
        ScheduledTaskWeekday::Saturday,
        ScheduledTaskWeekday::Sunday,
    ]
}

fn default_database_maintenance_interval_days() -> u32 {
    7
}

fn default_trash_cleanup_interval_days() -> u32 {
    1
}

fn default_metadata_provider_settings(id: MetadataProviderId) -> MetadataProviderSettings {
    match id {
        MetadataProviderId::Tmdb => MetadataProviderSettings::default(),
        MetadataProviderId::Tvdb => MetadataProviderSettings {
            id: MetadataProviderId::Tvdb,
            enabled: false,
            api_key: None,
            api_key_secret_ref: None,
            api_key_configured: false,
            clear_api_key: false,
            language: default_metadata_language(),
            rate_limit_per_second: default_provider_rate_limit_per_second(),
            retry_attempts: default_provider_retry_attempts(),
            retry_backoff_ms: default_provider_retry_backoff_ms(),
        },
        MetadataProviderId::MusicBrainz => MetadataProviderSettings {
            id: MetadataProviderId::MusicBrainz,
            enabled: false,
            api_key: None,
            api_key_secret_ref: None,
            api_key_configured: false,
            clear_api_key: false,
            language: default_metadata_language(),
            rate_limit_per_second: default_provider_rate_limit_per_second(),
            retry_attempts: default_provider_retry_attempts(),
            retry_backoff_ms: default_provider_retry_backoff_ms(),
        },
        MetadataProviderId::OpenLibrary => MetadataProviderSettings {
            id: MetadataProviderId::OpenLibrary,
            enabled: false,
            api_key: None,
            api_key_secret_ref: None,
            api_key_configured: false,
            clear_api_key: false,
            language: default_metadata_language(),
            rate_limit_per_second: default_provider_rate_limit_per_second(),
            retry_attempts: default_provider_retry_attempts(),
            retry_backoff_ms: default_provider_retry_backoff_ms(),
        },
        MetadataProviderId::LocalNfo => MetadataProviderSettings {
            id: MetadataProviderId::LocalNfo,
            enabled: true,
            api_key: None,
            api_key_secret_ref: None,
            api_key_configured: false,
            clear_api_key: false,
            language: default_metadata_language(),
            rate_limit_per_second: default_provider_rate_limit_per_second(),
            retry_attempts: default_provider_retry_attempts(),
            retry_backoff_ms: default_provider_retry_backoff_ms(),
        },
        MetadataProviderId::Themerr => MetadataProviderSettings {
            id: MetadataProviderId::Themerr,
            enabled: true,
            api_key: None,
            api_key_secret_ref: None,
            api_key_configured: false,
            clear_api_key: false,
            language: default_metadata_language(),
            rate_limit_per_second: default_provider_rate_limit_per_second(),
            retry_attempts: default_provider_retry_attempts(),
            retry_backoff_ms: default_provider_retry_backoff_ms(),
        },
        MetadataProviderId::TrailerDb => MetadataProviderSettings {
            id: MetadataProviderId::TrailerDb,
            enabled: true,
            api_key: None,
            api_key_secret_ref: None,
            api_key_configured: false,
            clear_api_key: false,
            language: default_metadata_language(),
            rate_limit_per_second: default_provider_rate_limit_per_second(),
            retry_attempts: default_provider_retry_attempts(),
            retry_backoff_ms: default_provider_retry_backoff_ms(),
        },
    }
}

fn default_library_metadata_providers() -> Vec<MetadataProviderId> {
    vec![MetadataProviderId::Tmdb]
}

fn default_library_metadata_language_mode() -> MediaLibraryMetadataLanguageMode {
    MediaLibraryMetadataLanguageMode::Auto
}

fn normalized_unique_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut normalized = Vec::new();

    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }

        let owned = trimmed.to_string();
        if seen.insert(owned.clone()) {
            normalized.push(owned);
        }
    }

    normalized
}

/// How metadata languages are chosen for a library.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MediaLibraryMetadataLanguageMode {
    /// Use every language preferred by users who can access the library.
    #[default]
    Auto,
    /// Use the explicit language list configured on the library.
    Manual,
}

/// A configured media-library root.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Default)]
pub struct MediaLibrarySettings {
    /// Human-friendly library name.
    #[serde(default)]
    pub name: String,
    /// Filesystem path to the media-library root.
    #[serde(default)]
    pub path: String,
    /// Filesystem paths for one logical library when multiple roots are configured.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Whether the scanner should recurse into subdirectories.
    #[serde(default = "default_recursive_scan")]
    pub recursive: bool,
    /// The intended media category for the library.
    #[serde(default)]
    pub kind: MediaLibraryKind,
    /// Scanner used to inventory files for the library.
    #[serde(default)]
    pub scanner: MediaLibraryScanner,
    /// Ordered metadata providers to use for this library.
    #[serde(default = "default_library_metadata_providers")]
    pub metadata_providers: Vec<MetadataProviderId>,
    /// Whether metadata languages are inferred from library users or set manually.
    #[serde(default = "default_library_metadata_language_mode")]
    pub metadata_language_mode: MediaLibraryMetadataLanguageMode,
    /// Ordered metadata languages to fetch and prefer for this library.
    #[serde(default = "default_metadata_languages")]
    pub metadata_languages: Vec<String>,
    /// User ids allowed to view this library. Empty means all users.
    #[serde(default)]
    pub allowed_user_ids: Vec<i32>,
}

impl MediaLibrarySettings {
    /// Return all configured filesystem roots for this logical library.
    pub fn configured_paths(&self) -> Vec<String> {
        normalized_unique_strings(
            std::iter::once(self.path.clone()).chain(self.paths.iter().cloned()),
        )
    }

    /// Return the first configured filesystem root for this library, when present.
    pub fn primary_path(&self) -> String {
        self.configured_paths()
            .into_iter()
            .next()
            .unwrap_or_default()
    }

    /// Normalize path and provider settings for persistence.
    pub fn normalize(&mut self) {
        let normalized_paths = self.configured_paths();
        self.path = normalized_paths.first().cloned().unwrap_or_default();
        self.paths = normalized_paths;
        self.metadata_providers = normalized_unique_strings(
            self.metadata_providers
                .iter()
                .map(|provider| provider.as_storage_value().to_string()),
        )
        .into_iter()
        .filter_map(|value| MetadataProviderId::from_storage_value(&value))
        .collect();
        if self.metadata_providers.is_empty() {
            self.metadata_providers = default_library_metadata_providers();
        }
        if self.metadata_providers.is_empty() {
            self.metadata_providers = default_library_metadata_providers();
        }
        self.metadata_languages = normalized_unique_strings(
            self.metadata_languages
                .iter()
                .map(|language| language.trim().to_string()),
        );
        if self.metadata_languages.is_empty() {
            self.metadata_languages = default_metadata_languages();
        }
        self.allowed_user_ids.sort_unstable();
        self.allowed_user_ids.dedup();
        self.allowed_user_ids.retain(|user_id| *user_id > 0);
    }
}

/// Media scanning settings.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Default)]
pub struct MediaSettings {
    /// Configured media-library roots.
    #[serde(default)]
    pub libraries: Vec<MediaLibrarySettings>,
    /// Automatically delete missing catalog items after this many days. `None` disables cleanup.
    #[serde(default = "default_missing_item_auto_delete_days")]
    pub missing_item_auto_delete_days: Option<u32>,
}

/// Supported external metadata providers.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum MetadataProviderId {
    /// TheMovieDB for movie and television metadata.
    #[default]
    Tmdb,
    /// TheTVDB for movie and television metadata.
    Tvdb,
    /// MusicBrainz for music-oriented metadata.
    #[serde(rename = "musicbrainz")]
    MusicBrainz,
    /// Open Library for book metadata.
    OpenLibrary,
    /// Local NFO files and sidecar metadata.
    LocalNfo,
    /// ThemerrDB theme-song metadata extension provider.
    Themerr,
    /// The Trailer Database trailer metadata extension provider.
    #[serde(rename = "trailerdb")]
    TrailerDb,
}

impl MetadataProviderId {
    /// Return the stable storage value for this provider identifier.
    pub fn as_storage_value(&self) -> &'static str {
        match self {
            MetadataProviderId::Tmdb => "tmdb",
            MetadataProviderId::Tvdb => "tvdb",
            MetadataProviderId::MusicBrainz => "musicbrainz",
            MetadataProviderId::OpenLibrary => "open_library",
            MetadataProviderId::LocalNfo => "local_nfo",
            MetadataProviderId::Themerr => "themerr",
            MetadataProviderId::TrailerDb => "trailerdb",
        }
    }

    /// Parse a provider identifier from a stored string value.
    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value.trim() {
            "tmdb" => Some(MetadataProviderId::Tmdb),
            "tvdb" => Some(MetadataProviderId::Tvdb),
            "musicbrainz" => Some(MetadataProviderId::MusicBrainz),
            "open_library" => Some(MetadataProviderId::OpenLibrary),
            "local_nfo" => Some(MetadataProviderId::LocalNfo),
            "themerr" => Some(MetadataProviderId::Themerr),
            "trailerdb" => Some(MetadataProviderId::TrailerDb),
            _ => None,
        }
    }
}

fn metadata_provider_api_key_secret_ref(provider_id: &MetadataProviderId) -> String {
    format!(
        "metadata-provider:{}:api-key",
        provider_id.as_storage_value()
    )
}

fn normalized_secret_value(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn metadata_provider_has_api_key_value(provider: &MetadataProviderSettings) -> bool {
    normalized_secret_value(provider.api_key.as_ref()).is_some()
        || normalized_secret_value(provider.api_key_secret_ref.as_ref()).is_some()
}

pub(crate) fn metadata_provider_api_key_configured(provider: &MetadataProviderSettings) -> bool {
    metadata_provider_has_api_key_value(provider) || provider.api_key_configured
}

/// Configuration for one metadata provider.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub struct MetadataProviderSettings {
    /// Provider identifier.
    #[serde(default)]
    pub id: MetadataProviderId,
    /// Whether this provider is enabled.
    #[serde(default = "default_provider_enabled")]
    pub enabled: bool,
    /// Provider-specific API key or token, when a new value is submitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Stable secret-store reference for this provider's API key or token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_secret_ref: Option<String>,
    /// Whether this provider has a saved API key or token.
    #[serde(default, skip_deserializing, skip_serializing_if = "is_false")]
    pub api_key_configured: bool,
    /// Whether the saved API key or token should be removed.
    #[serde(default, skip_serializing)]
    pub clear_api_key: bool,
    /// Preferred language for metadata results.
    #[serde(default = "default_metadata_language")]
    pub language: String,
    /// Maximum request rate the provider should use when making API calls.
    #[serde(default = "default_provider_rate_limit_per_second")]
    pub rate_limit_per_second: u32,
    /// Maximum number of retry attempts after transient provider failures.
    #[serde(default = "default_provider_retry_attempts")]
    pub retry_attempts: u32,
    /// Base retry backoff in milliseconds.
    #[serde(default = "default_provider_retry_backoff_ms")]
    pub retry_backoff_ms: u32,
}

/// Metadata acquisition settings.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub struct MetadataSettings {
    /// Ordered list of enabled and optional providers.
    #[serde(default)]
    pub providers: Vec<MetadataProviderSettings>,
    /// Automatic metadata refresh interval in days. `None` disables automatic refreshes.
    #[serde(default = "default_metadata_refresh_interval_days")]
    pub refresh_interval_days: Option<u32>,
}

/// Days when scheduled tasks can run.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScheduledTaskWeekday {
    /// Monday.
    #[default]
    Monday,
    /// Tuesday.
    Tuesday,
    /// Wednesday.
    Wednesday,
    /// Thursday.
    Thursday,
    /// Friday.
    Friday,
    /// Saturday.
    Saturday,
    /// Sunday.
    Sunday,
}

/// Shared time window for scheduled tasks.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ScheduledTaskWindowSettings {
    /// Local time when scheduled tasks may start, formatted as HH:MM.
    #[serde(default = "default_scheduled_task_window_start")]
    pub start_time: String,
    /// Local time when scheduled tasks must stop starting, formatted as HH:MM.
    #[serde(default = "default_scheduled_task_window_stop")]
    pub stop_time: String,
    /// Local weekdays when scheduled tasks may run. Empty means every day.
    #[serde(default = "default_scheduled_task_weekdays")]
    pub weekdays: Vec<ScheduledTaskWeekday>,
}

/// Scheduled metadata refresh task settings.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub struct MetadataRefreshTaskSettings {
    /// Whether automatic stale metadata refreshes run from the scheduled task runner.
    #[serde(default = "default_scheduled_tasks_enabled")]
    pub enabled: bool,
}

/// Scheduled trash cleanup task settings.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub struct TrashCleanupTaskSettings {
    /// Whether missing media item cleanup runs automatically.
    #[serde(default = "default_trash_cleanup_enabled")]
    pub enabled: bool,
    /// Number of days an item must be missing before automatic cleanup deletes it.
    #[serde(default = "default_missing_item_auto_delete_days")]
    pub missing_item_auto_delete_days: Option<u32>,
    /// Minimum number of days between trash cleanup runs.
    #[serde(default = "default_trash_cleanup_interval_days")]
    pub interval_days: u32,
}

/// Scheduled database maintenance task settings.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub struct DatabaseMaintenanceTaskSettings {
    /// Whether database checkpoint, cleanup, and vacuum maintenance runs automatically.
    #[serde(default = "default_scheduled_tasks_enabled")]
    pub enabled: bool,
    /// Minimum number of days between database maintenance runs.
    #[serde(default = "default_database_maintenance_interval_days")]
    pub interval_days: u32,
}

/// Scheduled task settings.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ScheduledTasksSettings {
    /// Whether the scheduled task runner is enabled.
    #[serde(default = "default_scheduled_tasks_enabled")]
    pub enabled: bool,
    /// Shared local run window for scheduled tasks.
    #[serde(default)]
    pub window: ScheduledTaskWindowSettings,
    /// Stale metadata refresh task.
    #[serde(default)]
    pub metadata_refresh: MetadataRefreshTaskSettings,
    /// Missing item trash cleanup task.
    #[serde(default)]
    pub trash_cleanup: TrashCleanupTaskSettings,
    /// Database cleanup and vacuum task.
    #[serde(default)]
    pub database_maintenance: DatabaseMaintenanceTaskSettings,
}

/// FFmpeg-related tooling settings.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub struct FfmpegSettings {
    /// Path or command name for the FFmpeg executable.
    #[serde(default = "default_ffmpeg_path")]
    pub ffmpeg_path: String,
    /// Path or command name for the ffprobe executable.
    #[serde(default = "default_ffprobe_path")]
    pub ffprobe_path: String,
}

/// Server settings.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ServerSettings {
    /// Whether to use HTTPS.
    #[serde(default)]
    pub use_https: bool,
    /// The address to bind to.
    #[serde(default)]
    pub address: String,
    /// The port to bind to.
    #[serde(default)]
    pub port: u16,
    /// Certificate path.
    #[serde(default)]
    pub cert_path: String,
    /// Key path.
    #[serde(default)]
    pub key_path: String,
    /// Use custom certs.
    #[serde(default)]
    pub use_custom_certs: bool,
}

/// Application settings.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Default)]
pub struct Settings {
    /// General settings.
    #[serde(default)]
    pub general: GeneralSettings,
    /// Media settings.
    #[serde(default)]
    pub media: MediaSettings,
    /// Metadata-provider settings.
    #[serde(default)]
    pub metadata: MetadataSettings,
    /// Server settings.
    #[serde(default)]
    pub server: ServerSettings,
    /// FFmpeg tooling settings.
    #[serde(default)]
    pub ffmpeg: FfmpegSettings,
    /// Scheduled task settings.
    #[serde(default)]
    pub scheduled_tasks: ScheduledTasksSettings,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        GeneralSettings {
            data_dir: config_local_dir()
                .unwrap()
                .join(GLOBAL_APP_NAME)
                .join("data")
                .to_str()
                .unwrap()
                .into(),
        }
    }
}

impl Default for ServerSettings {
    fn default() -> Self {
        ServerSettings {
            use_https: true,
            address: "127.0.0.1".into(),
            port: 9191,
            cert_path: "cert.pem".into(),
            key_path: "key.pem".into(),
            use_custom_certs: false,
        }
    }
}

impl Default for FfmpegSettings {
    fn default() -> Self {
        Self {
            ffmpeg_path: default_ffmpeg_path(),
            ffprobe_path: default_ffprobe_path(),
        }
    }
}

impl Default for MetadataProviderSettings {
    fn default() -> Self {
        Self {
            id: MetadataProviderId::Tmdb,
            enabled: default_provider_enabled(),
            api_key: None,
            api_key_secret_ref: None,
            api_key_configured: false,
            clear_api_key: false,
            language: default_metadata_language(),
            rate_limit_per_second: default_provider_rate_limit_per_second(),
            retry_attempts: default_provider_retry_attempts(),
            retry_backoff_ms: default_provider_retry_backoff_ms(),
        }
    }
}

impl Default for MetadataSettings {
    fn default() -> Self {
        Self {
            providers: vec![
                default_metadata_provider_settings(MetadataProviderId::Tmdb),
                default_metadata_provider_settings(MetadataProviderId::Tvdb),
                default_metadata_provider_settings(MetadataProviderId::MusicBrainz),
                default_metadata_provider_settings(MetadataProviderId::OpenLibrary),
                default_metadata_provider_settings(MetadataProviderId::LocalNfo),
                default_metadata_provider_settings(MetadataProviderId::Themerr),
                default_metadata_provider_settings(MetadataProviderId::TrailerDb),
            ],
            refresh_interval_days: default_metadata_refresh_interval_days(),
        }
    }
}

impl Default for ScheduledTaskWindowSettings {
    fn default() -> Self {
        Self {
            start_time: default_scheduled_task_window_start(),
            stop_time: default_scheduled_task_window_stop(),
            weekdays: default_scheduled_task_weekdays(),
        }
    }
}

impl Default for MetadataRefreshTaskSettings {
    fn default() -> Self {
        Self {
            enabled: default_scheduled_tasks_enabled(),
        }
    }
}

impl Default for TrashCleanupTaskSettings {
    fn default() -> Self {
        Self {
            enabled: default_trash_cleanup_enabled(),
            missing_item_auto_delete_days: default_missing_item_auto_delete_days(),
            interval_days: default_trash_cleanup_interval_days(),
        }
    }
}

impl Default for DatabaseMaintenanceTaskSettings {
    fn default() -> Self {
        Self {
            enabled: default_scheduled_tasks_enabled(),
            interval_days: default_database_maintenance_interval_days(),
        }
    }
}

impl Default for ScheduledTasksSettings {
    fn default() -> Self {
        Self {
            enabled: default_scheduled_tasks_enabled(),
            window: ScheduledTaskWindowSettings::default(),
            metadata_refresh: MetadataRefreshTaskSettings::default(),
            trash_cleanup: TrashCleanupTaskSettings::default(),
            database_maintenance: DatabaseMaintenanceTaskSettings::default(),
        }
    }
}

impl Settings {
    /// Create a new instance of `Settings`.
    pub fn new() -> Result<Self, ConfigError> {
        let config = Config::builder()
            .set_default("general.data_dir", GeneralSettings::default().data_dir)?
            .set_default("server.use_https", ServerSettings::default().use_https)?
            .set_default("server.address", ServerSettings::default().address)?
            .set_default("server.port", ServerSettings::default().port)?
            .set_default("server.cert_path", ServerSettings::default().cert_path)?
            .set_default("server.key_path", ServerSettings::default().key_path)?
            .set_default(
                "server.use_custom_certs",
                ServerSettings::default().use_custom_certs,
            )?
            .add_source(File::with_name(settings_base_path().to_str().unwrap()).required(false))
            .add_source(Environment::with_prefix(
                GLOBAL_APP_NAME.to_uppercase().as_str(),
            ))
            .build()?;

        config.try_deserialize()
    }

    /// Load settings from the configuration file.
    pub fn load() -> Self {
        Self::new().expect("Failed to load settings")
    }
}

/// Normalize settings values before persistence or runtime replacement.
pub fn normalize_settings(settings: &mut Settings) {
    if let Some(days) = settings.metadata.refresh_interval_days {
        settings.metadata.refresh_interval_days = match days {
            30 | 60 | 90 => Some(days),
            _ => default_metadata_refresh_interval_days(),
        };
    }

    for library in &mut settings.media.libraries {
        library.normalize();
    }
    if let Some(days) = settings.media.missing_item_auto_delete_days.take() {
        if days > 0
            && settings
                .scheduled_tasks
                .trash_cleanup
                .missing_item_auto_delete_days
                .is_none()
        {
            settings.scheduled_tasks.trash_cleanup.enabled = true;
            settings
                .scheduled_tasks
                .trash_cleanup
                .missing_item_auto_delete_days = Some(days.min(3650));
        }
    }
    normalize_scheduled_tasks_settings(&mut settings.scheduled_tasks);

    let mut seen_provider_ids = std::collections::HashSet::new();
    settings
        .metadata
        .providers
        .retain(|provider| seen_provider_ids.insert(provider.id.clone()));
    for provider in &mut settings.metadata.providers {
        provider.language = {
            let trimmed = provider.language.trim();
            if trimmed.is_empty() { default_metadata_language() } else { trimmed.to_string() }
        };
        provider.rate_limit_per_second = provider.rate_limit_per_second.max(1);
        provider.retry_backoff_ms = provider.retry_backoff_ms.max(1);
        provider.api_key = normalized_secret_value(provider.api_key.as_ref());
        provider.api_key_secret_ref = normalized_secret_value(provider.api_key_secret_ref.as_ref());
        provider.api_key_configured = metadata_provider_has_api_key_value(provider);
        if provider.clear_api_key {
            provider.api_key_configured = false;
        }
        if provider.id == MetadataProviderId::Tvdb && provider.api_key_configured {
            // Older settings snapshots can retain TVDB as disabled even after an API key
            // is saved, which leaves TVDB-only libraries unusable.
            provider.enabled = true;
        }
    }

    for provider_id in [
        MetadataProviderId::Tmdb,
        MetadataProviderId::Tvdb,
        MetadataProviderId::MusicBrainz,
        MetadataProviderId::OpenLibrary,
        MetadataProviderId::LocalNfo,
        MetadataProviderId::Themerr,
        MetadataProviderId::TrailerDb,
    ] {
        if !settings
            .metadata
            .providers
            .iter()
            .any(|provider| provider.id == provider_id)
        {
            settings
                .metadata
                .providers
                .push(default_metadata_provider_settings(provider_id));
        }
    }
}

pub(crate) fn merge_metadata_provider_secret_state(
    settings: &mut Settings,
    existing: &Settings,
) {
    let existing_providers = existing
        .metadata
        .providers
        .iter()
        .map(|provider| (provider.id.clone(), provider.clone()))
        .collect::<HashMap<_, _>>();

    for provider in &mut settings.metadata.providers {
        let existing_provider = existing_providers.get(&provider.id);
        let submitted_api_key = normalized_secret_value(provider.api_key.as_ref());
        provider.api_key = submitted_api_key;

        if provider.clear_api_key {
            if provider.api_key_secret_ref.is_none() {
                provider.api_key_secret_ref =
                    existing_provider.and_then(|provider| provider.api_key_secret_ref.clone());
            }
            continue;
        }

        if provider.api_key.is_none() {
            if let Some(existing_provider) = existing_provider {
                provider.api_key_secret_ref = existing_provider.api_key_secret_ref.clone();
                if provider.api_key_secret_ref.is_none() {
                    provider.api_key = normalized_secret_value(existing_provider.api_key.as_ref());
                }
                provider.api_key_configured =
                    metadata_provider_api_key_configured(existing_provider);
            }
        }
    }
}

fn persist_metadata_provider_secret(provider: &mut MetadataProviderSettings) -> Result<(), String> {
    if provider.clear_api_key {
        if let Some(secret_ref) = provider.api_key_secret_ref.take() {
            crate::secrets::delete_secret(&secret_ref)?;
        }
        provider.api_key = None;
        provider.api_key_configured = false;
        provider.clear_api_key = false;
        return Ok(());
    }

    if let Some(api_key) = provider.api_key.take() {
        let secret_ref = provider
            .api_key_secret_ref
            .clone()
            .unwrap_or_else(|| metadata_provider_api_key_secret_ref(&provider.id));
        crate::secrets::store_secret(&secret_ref, &api_key)?;
        provider.api_key_secret_ref = Some(secret_ref);
        provider.api_key_configured = true;
    } else {
        provider.api_key_configured = provider.api_key_secret_ref.is_some();
    }
    provider.clear_api_key = false;

    Ok(())
}

pub(crate) fn settings_with_persisted_secrets(settings: &Settings) -> Result<Settings, String> {
    let mut normalized = settings.clone();
    normalize_settings(&mut normalized);

    for provider in &mut normalized.metadata.providers {
        persist_metadata_provider_secret(provider)?;
    }
    normalize_settings(&mut normalized);
    Ok(normalized)
}

pub(crate) fn settings_for_api_response(settings: &Settings) -> Settings {
    let mut redacted = settings.clone();
    normalize_settings(&mut redacted);
    for provider in &mut redacted.metadata.providers {
        provider.api_key = None;
        provider.api_key_configured = metadata_provider_api_key_configured(provider);
        provider.api_key_secret_ref = None;
        provider.clear_api_key = false;
    }
    redacted
}

pub(crate) fn resolve_metadata_provider_api_key(
    provider: &mut MetadataProviderSettings
) -> Result<(), String> {
    if normalized_secret_value(provider.api_key.as_ref()).is_some() {
        provider.api_key = normalized_secret_value(provider.api_key.as_ref());
        return Ok(());
    }

    let Some(secret_ref) = normalized_secret_value(provider.api_key_secret_ref.as_ref()) else {
        provider.api_key = None;
        provider.api_key_configured = false;
        return Ok(());
    };

    provider.api_key = crate::secrets::load_secret(&secret_ref)?;
    provider.api_key_configured = provider.api_key.is_some();
    Ok(())
}

fn normalize_scheduled_time(
    value: &mut String,
    default_value: String,
) {
    let trimmed = value.trim();
    let parts = trimmed.split(':').collect::<Vec<_>>();
    let parsed = if parts.len() == 2 {
        parts[0]
            .parse::<u32>()
            .ok()
            .zip(parts[1].parse::<u32>().ok())
            .filter(|(hour, minute)| *hour < 24 && *minute < 60)
    } else {
        None
    };

    *value = if let Some((hour, minute)) = parsed {
        format!("{hour:02}:{minute:02}")
    } else {
        default_value
    };
}

fn normalize_scheduled_tasks_settings(settings: &mut ScheduledTasksSettings) {
    normalize_scheduled_time(
        &mut settings.window.start_time,
        default_scheduled_task_window_start(),
    );
    normalize_scheduled_time(
        &mut settings.window.stop_time,
        default_scheduled_task_window_stop(),
    );
    settings.window.weekdays.sort_by_key(|day| match day {
        ScheduledTaskWeekday::Monday => 1,
        ScheduledTaskWeekday::Tuesday => 2,
        ScheduledTaskWeekday::Wednesday => 3,
        ScheduledTaskWeekday::Thursday => 4,
        ScheduledTaskWeekday::Friday => 5,
        ScheduledTaskWeekday::Saturday => 6,
        ScheduledTaskWeekday::Sunday => 7,
    });
    settings.window.weekdays.dedup();
    if settings.window.weekdays.is_empty() {
        settings.window.weekdays = default_scheduled_task_weekdays();
    }
    if let Some(days) = settings.trash_cleanup.missing_item_auto_delete_days {
        settings.trash_cleanup.missing_item_auto_delete_days = (days > 0).then_some(days.min(3650));
    }
    if settings.trash_cleanup.enabled
        && settings
            .trash_cleanup
            .missing_item_auto_delete_days
            .is_none()
    {
        settings.trash_cleanup.missing_item_auto_delete_days = Some(30);
    }
    settings.trash_cleanup.interval_days = settings.trash_cleanup.interval_days.clamp(1, 365);
    settings.database_maintenance.interval_days =
        settings.database_maintenance.interval_days.clamp(1, 365);
}

/// Return a settings snapshot suitable for YAML persistence.
pub fn settings_for_persistence(settings: &Settings) -> Settings {
    let mut normalized = settings.clone();
    normalize_settings(&mut normalized);
    normalized.media.libraries.clear();
    for provider in &mut normalized.metadata.providers {
        provider.api_key = None;
        provider.api_key_configured = metadata_provider_api_key_configured(provider);
        provider.clear_api_key = false;
    }
    normalized
}

/// Serialize settings to YAML for disk persistence, omitting DB-owned library settings.
pub fn settings_yaml_for_persistence(settings: &Settings) -> Result<String, String> {
    let normalized = settings_for_persistence(settings);
    let mut value = serde_yaml::to_value(&normalized).map_err(|error| error.to_string())?;

    if let serde_yaml::Value::Mapping(root) = &mut value {
        let general_key = serde_yaml::Value::String("general".into());
        if let Some(general) = root.get(&general_key).cloned() {
            root.clear();
            root.insert(general_key, general);
        }
    }

    serde_yaml::to_string(&value).map_err(|error| error.to_string())
}

fn runtime_setting_value<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}

fn parse_runtime_setting<T: for<'de> Deserialize<'de>>(
    value: &str,
    key: &str,
) -> Result<T, String> {
    serde_json::from_str(value)
        .map_err(|error| format!("Failed to parse persisted {key} settings: {error}"))
}

fn media_settings_for_database(settings: &Settings) -> MediaSettings {
    let mut media = settings.media.clone();
    media.libraries.clear();
    media
}

fn upsert_runtime_setting(
    conn: &mut diesel::SqliteConnection,
    key: &str,
    value: String,
) -> Result<(), String> {
    use crate::db::schema::app_settings::dsl as app_settings_dsl;

    diesel::insert_into(app_settings_dsl::app_settings)
        .values(AppSetting {
            key: key.to_string(),
            value,
            updated_at: Some(chrono::Utc::now().timestamp()),
        })
        .on_conflict(app_settings_dsl::key)
        .do_update()
        .set((
            app_settings_dsl::value.eq(diesel::upsert::excluded(app_settings_dsl::value)),
            app_settings_dsl::updated_at.eq(diesel::upsert::excluded(app_settings_dsl::updated_at)),
        ))
        .execute(conn)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn insert_runtime_setting_if_missing(
    conn: &mut diesel::SqliteConnection,
    key: &str,
    value: String,
) -> Result<(), String> {
    use crate::db::schema::app_settings::dsl as app_settings_dsl;

    diesel::insert_into(app_settings_dsl::app_settings)
        .values(AppSetting {
            key: key.to_string(),
            value,
            updated_at: Some(chrono::Utc::now().timestamp()),
        })
        .on_conflict_do_nothing()
        .execute(conn)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Persist DB-owned runtime settings to SQLite.
pub fn save_database_settings(
    conn: &mut diesel::SqliteConnection,
    settings: &Settings,
) -> Result<(), String> {
    let normalized = settings_with_persisted_secrets(settings)?;

    upsert_runtime_setting(
        conn,
        METADATA_SETTINGS_KEY,
        runtime_setting_value(&normalized.metadata)?,
    )?;
    upsert_runtime_setting(
        conn,
        MEDIA_SETTINGS_KEY,
        runtime_setting_value(&media_settings_for_database(&normalized))?,
    )?;
    upsert_runtime_setting(
        conn,
        SERVER_SETTINGS_KEY,
        runtime_setting_value(&normalized.server)?,
    )?;
    upsert_runtime_setting(
        conn,
        FFMPEG_SETTINGS_KEY,
        runtime_setting_value(&normalized.ffmpeg)?,
    )?;
    upsert_runtime_setting(
        conn,
        SCHEDULED_TASKS_SETTINGS_KEY,
        runtime_setting_value(&normalized.scheduled_tasks)?,
    )?;
    Ok(())
}

/// Seed DB-owned runtime settings from bootstrap settings when no DB value exists yet.
pub fn seed_database_settings(
    conn: &mut diesel::SqliteConnection,
    settings: &Settings,
) -> Result<(), String> {
    let normalized = settings_with_persisted_secrets(settings)?;

    insert_runtime_setting_if_missing(
        conn,
        METADATA_SETTINGS_KEY,
        runtime_setting_value(&normalized.metadata)?,
    )?;
    insert_runtime_setting_if_missing(
        conn,
        MEDIA_SETTINGS_KEY,
        runtime_setting_value(&media_settings_for_database(&normalized))?,
    )?;
    insert_runtime_setting_if_missing(
        conn,
        SERVER_SETTINGS_KEY,
        runtime_setting_value(&normalized.server)?,
    )?;
    insert_runtime_setting_if_missing(
        conn,
        FFMPEG_SETTINGS_KEY,
        runtime_setting_value(&normalized.ffmpeg)?,
    )?;
    insert_runtime_setting_if_missing(
        conn,
        SCHEDULED_TASKS_SETTINGS_KEY,
        runtime_setting_value(&normalized.scheduled_tasks)?,
    )?;
    Ok(())
}

/// Load DB-owned runtime settings and merge them over the bootstrap settings.
pub fn load_database_settings(
    conn: &mut diesel::SqliteConnection,
    bootstrap: &Settings,
) -> Result<Settings, String> {
    let rows = app_settings::table
        .filter(app_settings::key.eq_any([
            METADATA_SETTINGS_KEY,
            MEDIA_SETTINGS_KEY,
            SERVER_SETTINGS_KEY,
            FFMPEG_SETTINGS_KEY,
            SCHEDULED_TASKS_SETTINGS_KEY,
        ]))
        .select(AppSetting::as_select())
        .load::<AppSetting>(conn)
        .map_err(|error| error.to_string())?;

    let mut settings = bootstrap.clone();
    for row in rows {
        match row.key.as_str() {
            METADATA_SETTINGS_KEY => {
                settings.metadata = parse_runtime_setting(&row.value, METADATA_SETTINGS_KEY)?;
            }
            MEDIA_SETTINGS_KEY => {
                let libraries = settings.media.libraries.clone();
                settings.media = parse_runtime_setting(&row.value, MEDIA_SETTINGS_KEY)?;
                settings.media.libraries = libraries;
            }
            SERVER_SETTINGS_KEY => {
                settings.server = parse_runtime_setting(&row.value, SERVER_SETTINGS_KEY)?;
            }
            FFMPEG_SETTINGS_KEY => {
                settings.ffmpeg = parse_runtime_setting(&row.value, FFMPEG_SETTINGS_KEY)?;
            }
            SCHEDULED_TASKS_SETTINGS_KEY => {
                settings.scheduled_tasks =
                    parse_runtime_setting(&row.value, SCHEDULED_TASKS_SETTINGS_KEY)?;
            }
            _ => {}
        }
    }
    let has_plaintext_provider_secrets = settings
        .metadata
        .providers
        .iter()
        .any(|provider| normalized_secret_value(provider.api_key.as_ref()).is_some());
    normalize_settings(&mut settings);
    if has_plaintext_provider_secrets {
        settings = settings_with_persisted_secrets(&settings)?;
        save_database_settings(conn, &settings)?;
    }
    Ok(settings)
}

fn settings_base_path() -> PathBuf {
    settings_directory_path().join("settings")
}

/// Return the settings directory path.
pub fn settings_directory_path() -> PathBuf {
    if let Ok(path) = std::env::var("KOKO_SETTINGS_DIR") {
        let path = path.trim();
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }

    config_local_dir().unwrap().join(GLOBAL_APP_NAME)
}

/// Return the YAML settings file path.
pub fn settings_file_path() -> PathBuf {
    if let Ok(path) = std::env::var("KOKO_SETTINGS_PATH") {
        let path = path.trim();
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }

    settings_directory_path().join("settings.yml")
}

/// Save settings to disk.
pub fn save_settings(settings: &Settings) -> Result<(), String> {
    let settings_path = settings_file_path();
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let yaml = settings_yaml_for_persistence(settings)?;
    fs::write(settings_path, yaml).map_err(|error| error.to_string())
}

/// Global mutable settings state for the application.
pub static CURRENT_SETTINGS: Lazy<RwLock<Settings>> = Lazy::new(|| RwLock::new(Settings::load()));

/// Return a clone of the current in-memory settings.
pub fn current_settings() -> Settings {
    let mut settings = CURRENT_SETTINGS.read().unwrap().clone();
    normalize_settings(&mut settings);
    settings
}

/// Replace the in-memory settings state.
pub fn replace_current_settings(settings: Settings) {
    let mut normalized = settings;
    normalize_settings(&mut normalized);
    *CURRENT_SETTINGS.write().unwrap() = normalized;
}
