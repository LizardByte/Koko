//! Web server utilities for the application.

// modules
mod routes;

// lib imports
use rocket::config::Config;
use rocket::config::TlsConfig;
use rocket::fairing::AdHoc;
use rocket::figment::Figment;
use rocket_okapi::settings::UrlObject;
use rocket_okapi::{rapidoc::*, swagger_ui::*};

// local imports
use crate::certs;
use crate::config::current_settings;
use crate::db::{DbConn, Migrate, ReleaseDatabase, prepare_sqlite_database_path};
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
    prepare_sqlite_database_path(&db_path);
    let database_url = sqlite_database_url(&db_path);

    let figment = Figment::from(Config::default())
        .merge((
            "databases",
            rocket::figment::map! {
                "sqlite_db" => rocket::figment::map! {
                    "url" => database_url,
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
        .attach(ReleaseDatabase)
        .attach(AdHoc::on_liftoff("Start library monitor", |rocket| {
            Box::pin(async move {
                if let Some(db) = DbConn::get_one(rocket).await {
                    routes::media::start_library_monitor(db);
                } else {
                    log::error!(
                        "Failed to acquire database connection for library monitor startup"
                    );
                }
            })
        }))
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

fn sqlite_database_url(db_path: &str) -> String {
    if db_path == ":memory:" || db_path.starts_with("file:") {
        return format!("sqlite://{db_path}");
    }

    let normalized = db_path.replace('\\', "/");
    let has_windows_drive = normalized
        .as_bytes()
        .get(1)
        .is_some_and(|character| *character == b':');
    if has_windows_drive {
        format!("sqlite:///{normalized}")
    } else {
        format!("sqlite://{normalized}")
    }
}

/// Launch the web server with graceful shutdown support.
pub async fn launch_with_shutdown(shutdown_signal: ShutdownSignal) {
    let rocket = rocket().ignite().await.expect("Failed to ignite rocket");
    let rocket_shutdown = rocket.shutdown();

    // Start the rocket server
    let rocket_handle = rocket.launch();
    tokio::pin!(rocket_handle);

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
        result = &mut rocket_handle => {
            log::info!("Rocket server has shut down");
            // Rocket shut down (likely due to SIGINT), signal other components to shut down
            shutdown_signal.shutdown();
            if let Err(e) = result {
                log::error!("Web server error: {}", e);
            }
        }
        _ = shutdown_future => {
            log::info!("Web server shutting down gracefully");
            rocket_shutdown.notify();
            if let Err(e) = rocket_handle.await {
                log::error!("Web server error during graceful shutdown: {}", e);
            }
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
