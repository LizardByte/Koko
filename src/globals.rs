#![doc = "Miscellaneous utilities for the application."]

// lib imports
use once_cell::sync::Lazy;

// local imports
use crate::config::GLOBAL_SETTINGS;

// global constants and variables
pub(crate) static GLOBAL_APP_NAME: &str = "Koko";
pub(crate) static GLOBAL_ICON_ICO_PATH: &str = "assets/icon.ico";

/// Paths used by the application.
#[derive(Default)]
pub struct AppPaths {
    /// Path to the SQLite database.
    pub db_path: String,
    /// Path to the log file.
    pub log_path: String,
}

impl AppPaths {
    /// Create a new AppPaths instance.
    pub fn new() -> Self {
        let base_dir = GLOBAL_SETTINGS.general.data_dir.clone();

        // create the base directory if it doesn't exist
        std::fs::create_dir_all(&base_dir).unwrap();

        AppPaths {
            db_path: format!("{}/{}.db", base_dir, GLOBAL_APP_NAME.to_lowercase()),
            log_path: format!("{}/{}.log", base_dir, GLOBAL_APP_NAME.to_lowercase()),
        }
    }
}

/// Get the server URL based on the global settings.
pub fn get_server_url() -> String {
    let schema = if GLOBAL_SETTINGS.server.use_https { "https" } else { "http" };
    format!(
        "{}://{}:{}",
        schema, GLOBAL_SETTINGS.server.address, GLOBAL_SETTINGS.server.port
    )
}

/// Global AppPaths instance.
pub static APP_PATHS: Lazy<AppPaths> = Lazy::new(AppPaths::new);
