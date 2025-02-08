#![doc = "Miscellaneous utilities for the application."]

// local imports
use crate::config::GLOBAL_SETTINGS;

// global constants and variables
pub(crate) static GLOBAL_APP_NAME: &str = "Koko";
pub(crate) static GLOBAL_ICON_ICO_PATH: &str = "assets/icon.ico";

/// Get the server URL based on the global settings.
pub fn get_server_url() -> String {
    let schema = if GLOBAL_SETTINGS.server.use_https { "https" } else { "http" };
    format!(
        "{}://{}:{}",
        schema, GLOBAL_SETTINGS.server.address, GLOBAL_SETTINGS.server.port
    )
}
