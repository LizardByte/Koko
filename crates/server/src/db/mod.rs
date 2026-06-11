//! Database utilities for the application.

pub(crate) mod models;
pub(crate) mod schema;

// standard imports
use std::fs;
use std::path::Path;

// lib imports
use diesel::Connection;
use diesel::connection::SimpleConnection;
use diesel_migrations::{
    EmbeddedMigrations,
    MigrationHarness,
    embed_migrations,
};
use rocket::{
    Build,
    Rocket,
    fairing::{
        Fairing,
        Info,
        Kind,
    },
};
use rocket_sync_db_pools::{
    database,
    diesel,
};

/// Embedded migrations for the SQLite database.
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("sql/migrations");

/// Apply SQLite pragmas that improve concurrency and reduce lock contention.
pub fn configure_sqlite_connection(
    conn: &mut diesel::SqliteConnection
) -> diesel::result::QueryResult<()> {
    conn.batch_execute(
        "PRAGMA foreign_keys = ON;PRAGMA journal_mode = WAL;PRAGMA synchronous = NORMAL;PRAGMA \
         busy_timeout = 5000;",
    )
}

/// Prepare the SQLite database path before Rocket initializes the pool.
pub fn prepare_sqlite_database_path(db_path: &str) {
    if let Some(parent) = Path::new(db_path).parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            log::warn!(
                "Failed to create database directory {:?}: {}",
                parent,
                error
            );
        }
    }

    clear_stale_sqlite_lock_files(db_path);
}

/// Prepare a SQLite database and run embedded migrations outside the Rocket pool.
pub fn initialize_sqlite_database(db_path: &str) -> Result<(), String> {
    prepare_sqlite_database_path(db_path);
    let mut conn = diesel::SqliteConnection::establish(db_path)
        .map_err(|error| format!("Failed to open SQLite database {db_path}: {error}"))?;
    configure_sqlite_connection(&mut conn)
        .map_err(|error| format!("Failed to configure SQLite database {db_path}: {error}"))?;
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|error| format!("Failed to run SQLite migrations {db_path}: {error}"))?;
    Ok(())
}

fn clear_stale_sqlite_lock_files(db_path: &str) {
    let Ok(mut conn) = diesel::SqliteConnection::establish(db_path) else {
        return;
    };
    if configure_sqlite_connection(&mut conn).is_err() {
        return;
    }
    if conn.batch_execute("BEGIN IMMEDIATE; ROLLBACK;").is_err() {
        log::warn!("SQLite database appears to be locked by another active process");
        return;
    }

    let _ = conn.batch_execute("PRAGMA wal_checkpoint(TRUNCATE);");
    let journal_path = format!("{db_path}-journal");
    if Path::new(&journal_path).exists() {
        if let Err(error) = fs::remove_file(&journal_path) {
            log::debug!(
                "SQLite rollback journal {} was not removed: {}",
                journal_path,
                error
            );
        }
    }
}

fn release_sqlite_database_lock(conn: &mut diesel::SqliteConnection) {
    if let Err(error) = conn.batch_execute("PRAGMA wal_checkpoint(TRUNCATE);PRAGMA optimize;") {
        log::warn!(
            "Failed to checkpoint SQLite database during shutdown: {}",
            error
        );
    }
}

/// Database connection fairing.
#[database("sqlite_db")]
pub struct DbConn(diesel::SqliteConnection);

/// Fairing to run migrations when the application starts.
pub struct Migrate;

/// Fairing to checkpoint SQLite and release database locks during shutdown.
pub struct ReleaseDatabase;

#[rocket::async_trait]
impl Fairing for Migrate {
    fn info(&self) -> Info {
        Info {
            name: "Database Migrations",
            kind: Kind::Ignite,
        }
    }

    async fn on_ignite(
        &self,
        rocket: Rocket<Build>,
    ) -> Result<Rocket<Build>, Rocket<Build>> {
        if let Some(conn) = DbConn::get_one(&rocket).await {
            let _ = conn
                .run(|c| {
                    configure_sqlite_connection(c).expect("Failed to configure SQLite connection");
                    c.run_pending_migrations(MIGRATIONS)
                        .expect("Failed to run migrations");
                })
                .await;
        }
        Ok(rocket)
    }
}

#[rocket::async_trait]
impl Fairing for ReleaseDatabase {
    fn info(&self) -> Info {
        Info {
            name: "Release Database",
            kind: Kind::Shutdown,
        }
    }

    async fn on_shutdown(
        &self,
        rocket: &Rocket<rocket::Orbit>,
    ) {
        if let Some(conn) = DbConn::get_one(rocket).await {
            conn.run(|c| release_sqlite_database_lock(c)).await;
        }
    }
}

impl rocket_okapi::request::OpenApiFromRequest<'_> for DbConn {
    fn from_request_input(
        _gen: &mut rocket_okapi::gen::OpenApiGenerator,
        _name: String,
        _required: bool,
    ) -> rocket_okapi::Result<rocket_okapi::request::RequestHeaderInput> {
        Ok(rocket_okapi::request::RequestHeaderInput::None)
    }
}
