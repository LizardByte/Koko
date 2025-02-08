#![doc(html_favicon_url = "../assets/icon.ico")]
#![doc(html_logo_url = "../assets/icon.png")]
#![doc = include_str!("../docs/README.md")]
#![deny(missing_docs)]

// modules
pub mod auth;
pub mod certs;
pub mod config;
pub mod db;
pub mod dependencies;
pub mod globals;
mod logging;
pub mod tray;
pub mod web;

// standard imports
use std::thread;

/// Main entry point for the application.
/// Initializes logging, the web server, and tray icon.
pub fn main() {
    logging::init().expect("Failed to initialize logging");

    let web_handle = thread::spawn(|| {
        web::launch();
    });

    tray::launch();

    web_handle.join().expect("Web server thread panicked");
}
