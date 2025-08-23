//! Web server utilities for the application.

// modules
mod routes;

// lib imports
use rocket::config::Config;
use rocket::config::TlsConfig;
use rocket::figment::Figment;
use rocket_okapi::settings::UrlObject;
use rocket_okapi::{rapidoc::*, swagger_ui::*};

// local imports
use crate::certs;
use crate::config::GLOBAL_SETTINGS;
use crate::db::{DbConn, Migrate};
use crate::globals;
use crate::signal_handler::ShutdownSignal;

/// Build the web server.
pub fn rocket() -> rocket::Rocket<rocket::Build> {
    rocket_with_db_path(None)
}

/// Build the web server with a custom database path (primarily for testing).
pub fn rocket_with_db_path(custom_db_path: Option<String>) -> rocket::Rocket<rocket::Build> {
    // the cert path changes depending on if the user wants to use custom certs
    let (cert_path, key_path);
    if !GLOBAL_SETTINGS.server.use_custom_certs {
        cert_path = format!("{}/cert.pem", GLOBAL_SETTINGS.general.data_dir);
        key_path = format!("{}/key.pem", GLOBAL_SETTINGS.general.data_dir);
    } else {
        cert_path = GLOBAL_SETTINGS.server.cert_path.clone();
        key_path = GLOBAL_SETTINGS.server.key_path.clone();
    }

    if GLOBAL_SETTINGS.server.use_https {
        certs::ensure_certificates_exist(cert_path.clone(), key_path.clone());
    }

    // Use custom database path for tests, or default for production
    let db_path = custom_db_path.unwrap_or_else(|| globals::APP_PATHS.db_path.clone());

    let figment = Figment::from(Config::default())
        .merge((
            "databases",
            rocket::figment::map! {
                "sqlite_db" => rocket::figment::map! {
                    "url" => format!("sqlite://{}", db_path),
                }
            },
        ))
        .merge(("address", GLOBAL_SETTINGS.server.address.clone()))
        .merge(("port", GLOBAL_SETTINGS.server.port))
        .merge((
            "tls",
            if GLOBAL_SETTINGS.server.use_https {
                Some(TlsConfig::from_paths(cert_path, key_path))
            } else {
                None
            },
        ));

    rocket::custom(figment)
        .attach(DbConn::fairing())
        .attach(Migrate)
        .mount("/", routes::all_routes())
        .mount(
            "/swagger-ui/",
            make_swagger_ui(&SwaggerUIConfig {
                url: "../openapi.json".to_owned(),
                ..Default::default()
            }),
        )
        .mount(
            "/rapidoc/",
            make_rapidoc(&RapiDocConfig {
                general: GeneralConfig {
                    spec_urls: vec![UrlObject::new(
                        "General",
                        "../openapi.json",
                    )],
                    ..Default::default()
                },
                hide_show: HideShowConfig {
                    allow_spec_url_load: false,
                    allow_spec_file_load: false,
                    ..Default::default()
                },
                ..Default::default()
            }),
        )
}

/// Launch the web server with graceful shutdown support.
pub async fn launch_with_shutdown(shutdown_signal: ShutdownSignal) {
    let rocket = rocket().ignite().await.expect("Failed to ignite rocket");

    // Start the rocket server
    let rocket_handle = rocket.launch();

    // Clone the shutdown signal for the future
    let shutdown_signal_clone = shutdown_signal.clone();

    // Create a future that completes when shutdown is signaled
    let shutdown_future = async move {
        while !shutdown_signal_clone.is_shutdown() {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        log::info!("Web server received shutdown signal");
    };

    // Race between the server and shutdown signal
    tokio::select! {
        result = rocket_handle => {
            log::info!("Rocket server has shut down");
            // Rocket shut down (likely due to SIGINT), signal other components to shut down
            shutdown_signal.shutdown();
            if let Err(e) = result {
                log::error!("Web server error: {}", e);
            }
        }
        _ = shutdown_future => {
            log::info!("Web server shutting down gracefully");
        }
    }
}

/// Launch the web server.
#[rocket::main]
pub async fn launch() {
    rocket()
        .launch()
        .await
        .expect("Failed to launch web server");
}
