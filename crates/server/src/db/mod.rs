//! Database utilities for the application.

pub(crate) mod models;
pub(crate) mod schema;

// standard imports
use std::fs;
use std::path::Path;

// lib imports
use diesel::Connection;
use diesel::connection::SimpleConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use rocket::{
    Build, Rocket,
    fairing::{Fairing, Info, Kind},
};
use rocket_sync_db_pools::{database, diesel};

/// Embedded migrations for the SQLite database.
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("sql/migrations");

/// Apply SQLite pragmas that improve concurrency and reduce lock contention.
pub fn configure_sqlite_connection(
    conn: &mut diesel::SqliteConnection
) -> diesel::result::QueryResult<()> {
    conn.batch_execute(
        "PRAGMA foreign_keys = ON;\
         PRAGMA journal_mode = WAL;\
         PRAGMA synchronous = NORMAL;\
         PRAGMA busy_timeout = 5000;",
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
    if let Err(error) = conn.batch_execute(
        "PRAGMA wal_checkpoint(TRUNCATE);\
         PRAGMA optimize;",
    ) {
        log::warn!(
            "Failed to checkpoint SQLite database during shutdown: {}",
            error
        );
    }
}

fn reconcile_legacy_migration_records(
    conn: &mut diesel::SqliteConnection
) -> diesel::result::QueryResult<()> {
    use diesel::connection::SimpleConnection;
    conn.batch_execute(
        "CREATE TABLE IF NOT EXISTS __diesel_schema_migrations (\
            version VARCHAR(50) PRIMARY KEY NOT NULL,\
            run_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP\
        );\
        INSERT OR IGNORE INTO __diesel_schema_migrations(version, run_on)
            SELECT '0000001', CURRENT_TIMESTAMP
            WHERE EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'users');\
        INSERT OR IGNORE INTO __diesel_schema_migrations(version, run_on)
            SELECT '0000002', CURRENT_TIMESTAMP
            WHERE EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'media_files');\
        INSERT OR IGNORE INTO __diesel_schema_migrations(version, run_on)
            SELECT '0000003', CURRENT_TIMESTAMP
            WHERE EXISTS (SELECT 1 FROM pragma_table_info('scan_state') WHERE name = 'scan_revision');\
        INSERT OR IGNORE INTO __diesel_schema_migrations(version, run_on)
            SELECT '0000004', CURRENT_TIMESTAMP
            WHERE EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'item_metadata_links');\
        INSERT OR IGNORE INTO __diesel_schema_migrations(version, run_on)
            SELECT '0000005', CURRENT_TIMESTAMP
            WHERE EXISTS (SELECT 1 FROM pragma_table_info('item_metadata_links') WHERE name = 'media_type');\
        INSERT OR IGNORE INTO __diesel_schema_migrations(version, run_on)
            SELECT '0000006', CURRENT_TIMESTAMP
            WHERE EXISTS (SELECT 1 FROM pragma_table_info('media_files') WHERE name = 'source_root_path');\
        INSERT OR IGNORE INTO __diesel_schema_migrations(version, run_on)
            SELECT '0000007', CURRENT_TIMESTAMP
            WHERE EXISTS (SELECT 1 FROM pragma_table_info('media_libraries') WHERE name = 'paths_json');\
        INSERT OR IGNORE INTO __diesel_schema_migrations(version, run_on)
            SELECT '0000008', CURRENT_TIMESTAMP
            WHERE EXISTS (SELECT 1 FROM pragma_table_info('item_metadata_links') WHERE name = 'media_type');\
        INSERT OR IGNORE INTO __diesel_schema_migrations(version, run_on)
            SELECT '0000009', CURRENT_TIMESTAMP
            WHERE EXISTS (SELECT 1 FROM pragma_table_info('media_files') WHERE name = 'metadata_match_attempted_at');\
        INSERT OR IGNORE INTO __diesel_schema_migrations(version, run_on)
            SELECT '0000010', CURRENT_TIMESTAMP
            WHERE EXISTS (SELECT 1 FROM pragma_table_info('playback_progress') WHERE name = 'user_id');\
        INSERT OR IGNORE INTO __diesel_schema_migrations(version, run_on)
            SELECT '0000011', CURRENT_TIMESTAMP
            WHERE EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'media_items');\
        INSERT OR IGNORE INTO __diesel_schema_migrations(version, run_on)
            SELECT '0000012', CURRENT_TIMESTAMP
            WHERE EXISTS (SELECT 1 FROM pragma_table_info('users') WHERE name = 'birthday');\
        INSERT OR IGNORE INTO __diesel_schema_migrations(version, run_on)
            SELECT '0000013', CURRENT_TIMESTAMP
            WHERE EXISTS (SELECT 1 FROM pragma_table_info('media_libraries') WHERE name = 'metadata_providers_json');\
        INSERT OR IGNORE INTO __diesel_schema_migrations(version, run_on)
            SELECT '0000014', CURRENT_TIMESTAMP
            WHERE EXISTS (SELECT 1 FROM pragma_table_info('item_metadata_links') WHERE name = 'locale_key');\
        INSERT OR IGNORE INTO __diesel_schema_migrations(version, run_on)
            SELECT '0000015', CURRENT_TIMESTAMP
            WHERE EXISTS (SELECT 1 FROM pragma_table_info('item_metadata_links') WHERE name = 'locale_key');\
        INSERT OR IGNORE INTO __diesel_schema_migrations(version, run_on)
            SELECT '0000016', CURRENT_TIMESTAMP
            WHERE EXISTS (SELECT 1 FROM pragma_table_info('item_metadata_links') WHERE name = 'logo_url');",
    )?;

    ensure_sqlite_column(
        conn,
        "item_metadata_links",
        Some("logo_url"),
        "cached_logo_path",
        "ALTER TABLE item_metadata_links ADD COLUMN cached_logo_path TEXT DEFAULT NULL",
    )?;
    ensure_sqlite_column(
        conn,
        "item_metadata_links",
        Some("logo_url"),
        "genres_json",
        "ALTER TABLE item_metadata_links ADD COLUMN genres_json TEXT DEFAULT NULL",
    )
}

fn ensure_sqlite_column(
    conn: &mut diesel::SqliteConnection,
    table_name: &str,
    required_column_name: Option<&str>,
    column_name: &str,
    add_column_sql: &str,
) -> diesel::result::QueryResult<()> {
    use diesel::prelude::*;

    #[derive(diesel::QueryableByName)]
    struct CountRow {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        count: i64,
    }

    let escaped_table = table_name.replace('\'', "''");
    let escaped_column = column_name.replace('\'', "''");
    let table_row = diesel::sql_query(format!(
        "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = '{escaped_table}'"
    ))
    .get_result::<CountRow>(conn)?;
    if table_row.count == 0 {
        return Ok(());
    }
    if let Some(required_column_name) = required_column_name {
        let escaped_required_column = required_column_name.replace('\'', "''");
        let prerequisite_row = diesel::sql_query(format!(
            "SELECT COUNT(*) AS count FROM pragma_table_info('{escaped_table}') WHERE name = '{escaped_required_column}'"
        ))
        .get_result::<CountRow>(conn)?;
        if prerequisite_row.count == 0 {
            return Ok(());
        }
    }
    let row = diesel::sql_query(format!(
        "SELECT COUNT(*) AS count FROM pragma_table_info('{escaped_table}') WHERE name = '{escaped_column}'"
    ))
    .get_result::<CountRow>(conn)?;
    if row.count == 0 {
        conn.batch_execute(add_column_sql)?;
    }
    Ok(())
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
                    reconcile_legacy_migration_records(c)
                        .expect("Failed to reconcile legacy SQLite migration records");
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
