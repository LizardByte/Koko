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
use crate::config::current_settings;
use crate::db::{DbConn, Migrate};
use crate::globals;
use crate::signal_handler::ShutdownSignal;

/// Build the web server.
pub fn rocket() -> rocket::Rocket<rocket::Build> {
    rocket_with_db_path(None)
}

/// Build the web server with a custom database path (primarily for testing).
pub fn rocket_with_db_path(custom_db_path: Option<String>) -> rocket::Rocket<rocket::Build> {
    let settings = current_settings();

    // the cert path changes depending on if the user wants to use custom certs
    let (cert_path, key_path);
    if !settings.server.use_custom_certs {
        cert_path = format!("{}/cert.pem", settings.general.data_dir);
        key_path = format!("{}/key.pem", settings.general.data_dir);
    } else {
        cert_path = settings.server.cert_path.clone();
        key_path = settings.server.key_path.clone();
    }

    if settings.server.use_https {
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
        .merge(("address", settings.server.address.clone()))
        .merge(("port", settings.server.port))
        .merge((
            "tls",
            if settings.server.use_https {
                Some(TlsConfig::from_paths(cert_path, key_path))
            } else {
                None
            },
        ));

    rocket::custom(figment)
        .attach(DbConn::fairing())
        .attach(Migrate)
        .mount("/", routes::api_routes())
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
        .mount("/", routes::spa_routes())
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
