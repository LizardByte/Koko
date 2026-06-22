//! Database utilities for the application.

pub(crate) mod models;
pub(crate) mod schema;

// standard imports
use std::collections::{
    HashMap,
    HashSet,
};
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

// lib imports
use diesel::Connection;
use diesel::connection::SimpleConnection;
use diesel::migration::{
    Migration,
    MigrationSource,
    MigrationVersion,
    Result as MigrationResult,
};
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
const MIGRATIONS: EmbeddedMigrations = embed_migrations!("sql/migrations");

/// Ordered SQLite migration revisions.
///
/// Diesel stores migration versions as text and normally sorts pending migrations
/// by that text. Keep the opaque revision IDs here in the exact order they must
/// be applied.
const SQLITE_MIGRATION_ORDER: &[&str] = &["a54d52c8da5e"];

#[derive(Debug)]
struct MigrationOrderError(String);

impl fmt::Display for MigrationOrderError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for MigrationOrderError {}

/// Apply SQLite pragmas that improve concurrency and reduce lock contention.
pub fn configure_sqlite_connection(
    conn: &mut diesel::SqliteConnection
) -> diesel::result::QueryResult<()> {
    conn.batch_execute(
        "PRAGMA foreign_keys = ON;PRAGMA journal_mode = WAL;PRAGMA synchronous = NORMAL;PRAGMA \
         busy_timeout = 5000;",
    )
}

/// Run pending SQLite migrations in the order declared by `SQLITE_MIGRATION_ORDER`.
pub fn run_pending_sqlite_migrations(
    conn: &mut diesel::SqliteConnection
) -> MigrationResult<Vec<MigrationVersion<'static>>> {
    let applied_versions = applied_sqlite_migration_versions(conn)?;
    let mut migrations_by_version = sqlite_migrations_by_version()?;
    validate_sqlite_migration_order(&migrations_by_version)?;

    let mut applied = Vec::new();
    for version in SQLITE_MIGRATION_ORDER {
        if applied_versions.contains(*version) {
            continue;
        }

        let Some(migration) = migrations_by_version.remove(*version) else {
            return migration_order_error(format!(
                "SQLite migration revision {version} is listed but not embedded"
            ));
        };
        applied.push(conn.run_migration(&*migration)?);
    }

    Ok(applied)
}

/// Revert applied SQLite migrations in reverse `SQLITE_MIGRATION_ORDER`.
pub fn revert_all_sqlite_migrations(
    conn: &mut diesel::SqliteConnection
) -> MigrationResult<Vec<MigrationVersion<'static>>> {
    let applied_versions = applied_sqlite_migration_versions(conn)?;
    let mut migrations_by_version = sqlite_migrations_by_version()?;
    validate_sqlite_migration_order(&migrations_by_version)?;

    let mut reverted = Vec::new();
    for version in SQLITE_MIGRATION_ORDER.iter().rev() {
        if !applied_versions.contains(*version) {
            continue;
        }

        let Some(migration) = migrations_by_version.remove(*version) else {
            return migration_order_error(format!(
                "SQLite migration revision {version} is listed but not embedded"
            ));
        };
        reverted.push(conn.revert_migration(&*migration)?);
    }

    Ok(reverted)
}

fn sqlite_migrations_by_version()
-> MigrationResult<HashMap<String, Box<dyn Migration<diesel::sqlite::Sqlite>>>> {
    let migrations =
        <EmbeddedMigrations as MigrationSource<diesel::sqlite::Sqlite>>::migrations(&MIGRATIONS)?;

    Ok(migrations
        .into_iter()
        .map(|migration| (migration.name().version().to_string(), migration))
        .collect())
}

fn applied_sqlite_migration_versions(
    conn: &mut diesel::SqliteConnection
) -> MigrationResult<HashSet<String>> {
    let mut versions = HashSet::new();
    for version in conn.applied_migrations()? {
        let version = version.to_string();
        if SQLITE_MIGRATION_ORDER.contains(&version.as_str()) {
            versions.insert(version);
        }
    }

    Ok(versions)
}

fn validate_sqlite_migration_order(
    migrations_by_version: &HashMap<String, Box<dyn Migration<diesel::sqlite::Sqlite>>>
) -> MigrationResult<()> {
    let mut listed_versions = HashSet::new();
    let mut duplicate_versions = Vec::new();
    for version in SQLITE_MIGRATION_ORDER {
        if !listed_versions.insert(*version) {
            duplicate_versions.push((*version).to_owned());
        }
    }
    duplicate_versions.sort();
    if !duplicate_versions.is_empty() {
        return migration_order_error(format!(
            "Duplicate SQLite migration revisions in SQLITE_MIGRATION_ORDER: {}",
            duplicate_versions.join(", ")
        ));
    }

    let mut missing_versions = SQLITE_MIGRATION_ORDER
        .iter()
        .copied()
        .filter(|version| !migrations_by_version.contains_key(*version))
        .collect::<Vec<_>>();
    missing_versions.sort_unstable();
    if !missing_versions.is_empty() {
        return migration_order_error(format!(
            "SQLite migration revisions are listed but not embedded: {}",
            missing_versions.join(", ")
        ));
    }

    let mut unlisted_versions = migrations_by_version
        .keys()
        .filter(|version| !listed_versions.contains(version.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    unlisted_versions.sort();
    if !unlisted_versions.is_empty() {
        return migration_order_error(format!(
            "SQLite migration revisions are embedded but missing from SQLITE_MIGRATION_ORDER: {}",
            unlisted_versions.join(", ")
        ));
    }

    Ok(())
}

fn migration_order_error<T>(message: String) -> MigrationResult<T> {
    Err(Box::new(MigrationOrderError(message)))
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
    run_pending_sqlite_migrations(&mut conn)
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
                    run_pending_sqlite_migrations(c).expect("Failed to run migrations");
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
            conn.run(release_sqlite_database_lock).await;
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
