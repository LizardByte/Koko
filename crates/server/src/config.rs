#![doc = "Configuration module for the application."]

// lib imports
use config::{Config, ConfigError, Environment, File};
use dirs::config_local_dir;
use once_cell::sync::Lazy;
use serde::Deserialize;

// local imports
use crate::globals::GLOBAL_APP_NAME;

/// General settings.
#[derive(Debug, Deserialize)]
pub struct GeneralSettings {
    /// The directory where application data is stored.
    #[serde(default)]
    pub data_dir: String,
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
#[derive(Debug, Deserialize, Default)]
pub struct Settings {
    /// General settings.
    #[serde(default)]
    pub general: GeneralSettings,
    /// Server settings.
    #[serde(default)]
    pub server: ServerSettings,
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

impl Settings {
    /// Create a new instance of `Settings`.
    pub fn new() -> Result<Self, ConfigError> {
        // Start with defaults provided via set_default and then merge in any provided config file or environment variables.
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
            // Add other configuration sources; values here will override the defaults.
            .add_source(
                File::with_name(
                    config_local_dir()
                        .unwrap()
                        .join(GLOBAL_APP_NAME)
                        .join("settings")
                        .to_str()
                        .unwrap(),
                )
                .required(false),
            )
            .add_source(Environment::with_prefix(
                GLOBAL_APP_NAME.to_uppercase().as_str(),
            ))
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
