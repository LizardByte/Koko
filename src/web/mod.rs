#![doc = "Web server utilities for the application."]

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
use crate::db::DbConn;

/// Build the web server.
pub fn rocket() -> rocket::Rocket<rocket::Build> {
    if GLOBAL_SETTINGS.server.use_https {
        certs::ensure_certificates_exist(GLOBAL_SETTINGS.server.cert_path.clone(), GLOBAL_SETTINGS.server.key_path.clone());
    }

    let figment = Figment::from(Config::default())
        .merge((
            "databases",
            rocket::figment::map! {
                "sqlite_db" => rocket::figment::map! {
                    "url" => format!("sqlite://{}", GLOBAL_SETTINGS.database.path.clone())
                }
            },
        ))
        .merge(("address", GLOBAL_SETTINGS.server.address.clone()))
        .merge(("port", GLOBAL_SETTINGS.server.port))
        .merge((
            "tls",
            if GLOBAL_SETTINGS.server.use_https {
                Some(TlsConfig::from_paths(GLOBAL_SETTINGS.server.cert_path.clone(), GLOBAL_SETTINGS.server.key_path.clone()))
            } else {
                None
            },
        ));

    rocket::custom(figment)
        .attach(DbConn::fairing())
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

/// Launch the web server.
#[rocket::main]
pub async fn launch() {
    rocket()
        .launch()
        .await
        .expect("Failed to launch web server");
}
