#![doc = "Configuration module for the application."]

// standard imports
use std::path::PathBuf;

// lib imports
use config::{Config, ConfigError, Environment, File};
use once_cell::sync::Lazy;
use serde::Deserialize;

/// General settings.
#[derive(Debug, Deserialize)]
pub struct GeneralSettings {
    /// The log path.
    #[serde(default)]
    pub log_path: String,
}

/// Database settings.
#[derive(Debug, Deserialize)]
pub struct DatabaseSettings {
    /// The URL for the database.
    #[serde(default)]
    pub path: String,
}

/// API settings.
#[derive(Debug, Deserialize)]
pub struct ApiSettings {
    /// The secret for the JWT.
    pub jwt_secret: String,
}

/// Server settings.
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
pub struct Settings {
    /// General settings.
    #[serde(default)]
    pub general: GeneralSettings,
    /// Database settings.
    #[serde(default)]
    pub database: DatabaseSettings,
    /// API settings.
    pub api: ApiSettings,
    /// Server settings.
    #[serde(default)]
    pub server: ServerSettings,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        GeneralSettings {
            log_path: "config/koko.log".into(),
        }
    }
}

impl Default for DatabaseSettings {
    fn default() -> Self {
        DatabaseSettings {
            path: "config/koko.db".into(),
        }
    }
}

impl Default for ServerSettings {
    fn default() -> Self {
        ServerSettings {
            use_https: true,
            address: "localhost".into(),
            port: 9191,
            cert_path: PathBuf::from("config").join("cert.pem").to_str().unwrap().into(),
            key_path: PathBuf::from("config").join("key.pem").to_str().unwrap().into(),
            use_custom_certs: false,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            general: GeneralSettings::default(),
            database: DatabaseSettings::default(),
            api: ApiSettings {
                jwt_secret: "".into(),
            },
            server: ServerSettings::default(),
        }
    }
}

impl Settings {
    /// Create a new instance of `Settings`.
    pub fn new() -> Result<Self, ConfigError> {
        // Start with defaults provided via set_default and then merge in any provided config file or environment variables.
        let config = Config::builder()
            .set_default("general.log_path", GeneralSettings::default().log_path)?
            .set_default("database.path", DatabaseSettings::default().path)?
            .set_default("server.use_https", ServerSettings::default().use_https)?
            .set_default("server.address", ServerSettings::default().address)?
            .set_default("server.port", ServerSettings::default().port)?
            .set_default("server.cert_path", ServerSettings::default().cert_path)?
            .set_default("server.key_path", ServerSettings::default().key_path)?
            .set_default("server.use_custom_certs", ServerSettings::default().use_custom_certs)?
            // You can now add other configuration sources; values here will override the defaults.
            .add_source(File::with_name("config/settings").required(false))
            .add_source(Environment::with_prefix("KOKO"))
            .build()?;

        // Deserialize the configuration into our Settings struct.
        config.try_deserialize()
    }

    /// Load settings from the configuration file.
    pub fn load() -> Self {
        Self::new().expect("Failed to load settings")
    }
}

/// Global settings for the application.
pub static GLOBAL_SETTINGS: Lazy<Settings> = Lazy::new(Settings::load);
