//! Configuration module for the application.

// standard imports
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

// lib imports
use config::{Config, ConfigError, Environment, File};
use dirs::config_local_dir;
use once_cell::sync::Lazy;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

// local imports
use crate::globals::GLOBAL_APP_NAME;

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

fn default_metadata_refresh_interval_days() -> Option<u32> {
    Some(30)
}

fn default_metadata_provider_settings(id: MetadataProviderId) -> MetadataProviderSettings {
    match id {
        MetadataProviderId::Tmdb => MetadataProviderSettings::default(),
        MetadataProviderId::Tvdb => MetadataProviderSettings {
            id: MetadataProviderId::Tvdb,
            enabled: false,
            api_key: None,
            language: default_metadata_language(),
            rate_limit_per_second: default_provider_rate_limit_per_second(),
            retry_attempts: default_provider_retry_attempts(),
            retry_backoff_ms: default_provider_retry_backoff_ms(),
        },
        MetadataProviderId::MusicBrainz => MetadataProviderSettings {
            id: MetadataProviderId::MusicBrainz,
            enabled: false,
            api_key: None,
            language: default_metadata_language(),
            rate_limit_per_second: default_provider_rate_limit_per_second(),
            retry_attempts: default_provider_retry_attempts(),
            retry_backoff_ms: default_provider_retry_backoff_ms(),
        },
        MetadataProviderId::OpenLibrary => MetadataProviderSettings {
            id: MetadataProviderId::OpenLibrary,
            enabled: false,
            api_key: None,
            language: default_metadata_language(),
            rate_limit_per_second: default_provider_rate_limit_per_second(),
            retry_attempts: default_provider_retry_attempts(),
            retry_backoff_ms: default_provider_retry_backoff_ms(),
        },
        MetadataProviderId::LocalNfo => MetadataProviderSettings {
            id: MetadataProviderId::LocalNfo,
            enabled: true,
            api_key: None,
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
    /// Ordered metadata providers to use for this library.
    #[serde(default = "default_library_metadata_providers")]
    pub metadata_providers: Vec<MetadataProviderId>,
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
    }
}

/// Media scanning settings.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Default)]
pub struct MediaSettings {
    /// Configured media-library roots.
    #[serde(default)]
    pub libraries: Vec<MediaLibrarySettings>,
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
            _ => None,
        }
    }
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
    /// Provider-specific API key or token, when required.
    #[serde(default)]
    pub api_key: Option<String>,
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
            ],
            refresh_interval_days: default_metadata_refresh_interval_days(),
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
        provider.api_key = provider
            .api_key
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if provider.id == MetadataProviderId::Tvdb && provider.api_key.is_some() {
            // Older settings snapshots can retain TVDB as disabled even after an API key
            // is saved, which leaves TVDB-only libraries unusable.
            provider.enabled = true;
        }
    }

    if !settings
        .metadata
        .providers
        .iter()
        .any(|provider| provider.id == MetadataProviderId::Tmdb)
    {
        settings
            .metadata
            .providers
            .push(default_metadata_provider_settings(MetadataProviderId::Tmdb));
    }
    if !settings
        .metadata
        .providers
        .iter()
        .any(|provider| provider.id == MetadataProviderId::Tvdb)
    {
        settings
            .metadata
            .providers
            .push(default_metadata_provider_settings(MetadataProviderId::Tvdb));
    }
}

/// Return a settings snapshot suitable for YAML persistence.
pub fn settings_for_persistence(settings: &Settings) -> Settings {
    let mut normalized = settings.clone();
    normalize_settings(&mut normalized);
    normalized.media.libraries.clear();
    normalized
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
    let normalized = settings_for_persistence(settings);
    let settings_path = settings_file_path();
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let yaml = serde_yaml::to_string(&normalized).map_err(|error| error.to_string())?;
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
