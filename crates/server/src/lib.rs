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
pub mod ffmpeg_resolve;
pub mod globals;
mod logging;
pub mod media;
pub mod metadata;
pub mod scanner;
pub mod scheduled_tasks;
mod secrets;
pub mod signal_handler;
pub mod transcode;
#[cfg(feature = "tray")]
pub mod tray;
pub mod utils;
pub mod web;

/// Main entry point for the application.
/// Initializes logging, the web server, and tray icon.
#[cfg(all(not(tarpaulin_include), feature = "tray"))]
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

/// Main entry point for the application without tray support.
/// Initializes logging and runs the web server on the main thread.
#[cfg(all(not(tarpaulin_include), not(feature = "tray")))]
pub fn main() {
    logging::init().expect("Failed to initialize logging");
    log::info!("Starting without tray support");

    let runtime =
        tokio::runtime::Runtime::new().expect("Failed to create tokio runtime for web server");
    runtime.block_on(web::launch_with_shutdown(
        signal_handler::ShutdownSignal::new(),
    ));
}
