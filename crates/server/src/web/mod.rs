//! Web server utilities for the application.

// modules
pub(crate) mod routes;

// lib imports
use diesel::Connection;
use rocket::config::Config;
use rocket::config::TlsConfig;
use rocket::fairing::AdHoc;
use rocket::figment::Figment;
use rocket_okapi::settings::UrlObject;
use rocket_okapi::{
    rapidoc::*,
    swagger_ui::*,
};

// local imports
use crate::certs;
use crate::config::{
    current_settings,
    load_database_settings,
    replace_current_settings,
    seed_database_settings,
};
use crate::db::{
    DbConn,
    Migrate,
    ReleaseDatabase,
    initialize_sqlite_database,
};
use crate::globals;
use crate::signal_handler::ShutdownSignal;

/// Build the web server.
pub fn rocket() -> rocket::Rocket<rocket::Build> {
    rocket_with_db_path(None)
}

/// Build the web server with a custom database path (primarily for testing).
pub fn rocket_with_db_path(custom_db_path: Option<String>) -> rocket::Rocket<rocket::Build> {
    let bootstrap_settings = current_settings();

    // Use custom database path for tests, or default for production.
    let db_path = custom_db_path.unwrap_or_else(|| globals::APP_PATHS.db_path.clone());
    let settings = match initialize_sqlite_database(&db_path) {
        Ok(()) => match diesel::SqliteConnection::establish(&db_path) {
            Ok(mut conn) => {
                if let Err(error) = seed_database_settings(&mut conn, &bootstrap_settings) {
                    log::warn!("Failed to seed database-backed settings: {}", error);
                }
                match load_database_settings(&mut conn, &bootstrap_settings) {
                    Ok(settings) => {
                        replace_current_settings(settings.clone());
                        settings
                    }
                    Err(error) => {
                        log::warn!("Failed to load database-backed settings: {}", error);
                        bootstrap_settings
                    }
                }
            }
            Err(error) => {
                log::warn!("Failed to reopen SQLite database for settings: {}", error);
                bootstrap_settings
            }
        },
        Err(error) => {
            log::warn!("{}", error);
            bootstrap_settings
        }
    };

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
        .attach(AdHoc::on_liftoff("Start background workers", |rocket| {
            Box::pin(async move {
                let scheduled_tasks_db = DbConn::get_one(rocket).await;
                match scheduled_tasks_db {
                    Some(scheduled_tasks_db) => {
                        crate::scheduled_tasks::start_scheduled_task_runner(scheduled_tasks_db);
                    }
                    None => {
                        log::error!(
                            "Failed to acquire database connection for background worker startup"
                        );
                    }
                }

                let metadata_recovery_db = DbConn::get_one(rocket).await;
                match metadata_recovery_db {
                    Some(metadata_recovery_db) => {
                        rocket::tokio::spawn(async move {
                            let settings = crate::config::current_settings();
                            crate::web::routes::media::recover_pending_metadata_refreshes(
                                &metadata_recovery_db,
                                &settings,
                            )
                            .await;
                        });
                    }
                    None => {
                        log::error!(
                            "Failed to acquire database connection for metadata refresh recovery"
                        );
                    }
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
    launch_rocket_with_shutdown(rocket(), shutdown_signal).await;
}

/// Launch a configured Rocket instance with graceful shutdown support.
pub async fn launch_rocket_with_shutdown(
    rocket: rocket::Rocket<rocket::Build>,
    shutdown_signal: ShutdownSignal,
) {
    let rocket = rocket.ignite().await.expect("Failed to ignite rocket");
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
