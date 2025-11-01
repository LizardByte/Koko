#![doc(html_favicon_url = "../assets/icon.ico")]
#![doc(html_logo_url = "../assets/icon.png")]
#![doc = include_str!("../../../docs/README.md")]
#![deny(missing_docs)]

// modules
pub mod auth;
pub mod certs;
pub mod config;
pub mod db;
pub mod dependencies;
pub mod globals;
mod logging;
pub mod signal_handler;
pub mod tray;
pub mod web;

// Remote desktop modules (Linux only)
#[cfg(target_os = "linux")]
pub mod capture;
#[cfg(target_os = "linux")]
pub mod clipboard;
#[cfg(target_os = "linux")]
pub mod input;
#[cfg(target_os = "linux")]
pub mod streaming;

/// Main entry point for the application.
/// Initializes logging, the web server, and tray icon.
#[cfg(not(tarpaulin_include))]
pub fn main() {
    logging::init().expect("Failed to initialize logging");

    // Create a shutdown coordinator to manage all threads
    let mut coordinator = signal_handler::ShutdownCoordinator::new();

    // Register the web server thread
    coordinator.register_async_thread("web-server", |shutdown_signal| async move {
        web::launch_with_shutdown(shutdown_signal).await;
        log::info!("Web server thread completed");
    });

    // Start the monitoring system
    coordinator.start_monitor();

    // Run tray on main thread - this will block until tray exits
    // The tray gets the main shutdown signal to coordinate with other threads
    tray::launch_with_shutdown(coordinator.signal());

    log::info!("Tray has exited, initiating coordinated shutdown");

    // Trigger shutdown of all threads
    coordinator.shutdown();

    // Wait for all threads to complete
    coordinator.wait_for_completion();

    log::info!("Application shutdown complete");
}
