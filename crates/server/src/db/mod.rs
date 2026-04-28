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

/// Reconcile SQLite schemas that were created by older development migrations.
pub fn reconcile_legacy_sqlite_schema(
    conn: &mut diesel::SqliteConnection
) -> diesel::result::QueryResult<()> {
    reconcile_legacy_migration_records(conn)
}

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
            WHERE EXISTS (SELECT 1 FROM pragma_table_info('item_metadata_links') WHERE name = 'logo_url');\
        INSERT OR IGNORE INTO __diesel_schema_migrations(version, run_on)
            SELECT '0000021', CURRENT_TIMESTAMP
            WHERE EXISTS (SELECT 1 FROM pragma_table_info('media_libraries') WHERE name = 'metadata_language_mode');\
        INSERT OR IGNORE INTO __diesel_schema_migrations(version, run_on)
            SELECT '0000022', CURRENT_TIMESTAMP
            WHERE EXISTS (SELECT 1 FROM pragma_table_info('item_metadata_links') WHERE name = 'theme_song_url')
               OR EXISTS (SELECT 1 FROM pragma_table_info('item_metadata_links') WHERE name = 'theme_song_youtube_url');\
        INSERT OR IGNORE INTO __diesel_schema_migrations(version, run_on)
            SELECT '0000023', CURRENT_TIMESTAMP
            WHERE EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'metadata_collections');",
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
    )?;

    if sqlite_migration_record_exists(conn, "0000021")? {
        ensure_sqlite_column(
            conn,
            "media_libraries",
            Some("metadata_providers_json"),
            "metadata_language_mode",
            "ALTER TABLE media_libraries ADD COLUMN metadata_language_mode TEXT NOT NULL DEFAULT 'auto'",
        )?;
        ensure_sqlite_column(
            conn,
            "media_libraries",
            Some("metadata_providers_json"),
            "metadata_languages_json",
            "ALTER TABLE media_libraries ADD COLUMN metadata_languages_json TEXT NOT NULL DEFAULT '[\"en-US\"]'",
        )?;
        ensure_sqlite_column(
            conn,
            "media_libraries",
            Some("metadata_providers_json"),
            "allowed_user_ids_json",
            "ALTER TABLE media_libraries ADD COLUMN allowed_user_ids_json TEXT NOT NULL DEFAULT '[]'",
        )?;
    }
    if sqlite_migration_record_exists(conn, "0000022")? {
        ensure_sqlite_column(
            conn,
            "item_metadata_links",
            Some("trailer_url"),
            "theme_song_url",
            "ALTER TABLE item_metadata_links ADD COLUMN theme_song_url TEXT DEFAULT NULL",
        )?;
        if sqlite_column_exists(conn, "item_metadata_links", "theme_song_url")?
            && sqlite_column_exists(conn, "item_metadata_links", "theme_song_youtube_url")?
        {
            conn.batch_execute(
                "UPDATE item_metadata_links
                 SET theme_song_url = theme_song_youtube_url
                 WHERE theme_song_url IS NULL
                   AND theme_song_youtube_url IS NOT NULL",
            )?;
        }
    }
    if sqlite_migration_record_exists(conn, "0000023")? {
        repair_metadata_collection_schema(conn)?;
    }

    Ok(())
}

fn repair_metadata_collection_schema(
    conn: &mut diesel::SqliteConnection
) -> diesel::result::QueryResult<()> {
    if !sqlite_table_exists(conn, "metadata_collections")? {
        return Ok(());
    }

    let collection_has_source_provider =
        sqlite_column_exists(conn, "metadata_collections", "source_provider_id")?;
    let collection_has_source_external =
        sqlite_column_exists(conn, "metadata_collections", "source_external_id")?;
    let collection_has_relation =
        sqlite_column_exists(conn, "metadata_collections", "relation_kind")?;
    let collection_has_locale = sqlite_column_exists(conn, "metadata_collections", "locale_key")?;
    let collection_has_provider_locale =
        sqlite_column_exists(conn, "metadata_collections", "provider_locale_key")?;
    let collection_has_theme =
        sqlite_column_exists(conn, "metadata_collections", "theme_song_url")?;
    let collection_has_overview = sqlite_column_exists(conn, "metadata_collections", "overview")?;
    let collection_has_artwork = sqlite_column_exists(conn, "metadata_collections", "artwork_url")?;
    let collection_has_backdrop =
        sqlite_column_exists(conn, "metadata_collections", "backdrop_url")?;
    let collection_has_updated = sqlite_column_exists(conn, "metadata_collections", "updated_at")?;
    let collection_has_name = sqlite_column_exists(conn, "metadata_collections", "name")?;
    let collection_name_is_not_null = sqlite_column_not_null(conn, "metadata_collections", "name")?;

    let item_table_exists = sqlite_table_exists(conn, "metadata_collection_items")?;
    let item_has_collection_id = item_table_exists
        && sqlite_column_exists(conn, "metadata_collection_items", "collection_id")?;
    let item_has_media_item_id = item_table_exists
        && sqlite_column_exists(conn, "metadata_collection_items", "media_item_id")?;
    let item_has_metadata_link_id = item_table_exists
        && sqlite_column_exists(conn, "metadata_collection_items", "metadata_link_id")?;
    let item_has_updated =
        item_table_exists && sqlite_column_exists(conn, "metadata_collection_items", "updated_at")?;
    let metadata_links_table_exists = sqlite_table_exists(conn, "item_metadata_links")?;

    if collection_has_source_provider
        && collection_has_source_external
        && collection_has_relation
        && collection_has_locale
        && collection_has_provider_locale
        && collection_has_theme
        && collection_has_name
        && item_table_exists
        && item_has_collection_id
        && item_has_media_item_id
        && item_has_metadata_link_id
        && !collection_name_is_not_null
    {
        return Ok(());
    }

    conn.batch_execute(
        "DROP TABLE IF EXISTS metadata_collection_items_next;\
         DROP TABLE IF EXISTS metadata_collections_next;\
         CREATE TABLE metadata_collections_next (\
            id INTEGER PRIMARY KEY AUTOINCREMENT,\
            provider_id TEXT NOT NULL,\
            external_id TEXT NOT NULL,\
            source_provider_id TEXT NOT NULL,\
            source_external_id TEXT NOT NULL,\
            relation_kind TEXT NOT NULL,\
            locale_key TEXT NOT NULL,\
            provider_locale_key TEXT DEFAULT NULL,\
            name TEXT DEFAULT NULL,\
            overview TEXT DEFAULT NULL,\
            artwork_url TEXT DEFAULT NULL,\
            backdrop_url TEXT DEFAULT NULL,\
            theme_song_url TEXT DEFAULT NULL,\
            updated_at BIGINT DEFAULT NULL,\
            UNIQUE (provider_id, external_id, relation_kind, locale_key)\
         );\
         CREATE TABLE metadata_collection_items_next (\
            id INTEGER PRIMARY KEY AUTOINCREMENT,\
            collection_id INTEGER NOT NULL,\
            media_item_id INTEGER NOT NULL,\
            metadata_link_id INTEGER NOT NULL,\
            updated_at BIGINT DEFAULT NULL,\
            FOREIGN KEY (collection_id) REFERENCES metadata_collections_next(id) ON DELETE CASCADE,\
            FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE CASCADE,\
            FOREIGN KEY (metadata_link_id) REFERENCES item_metadata_links(id) ON DELETE CASCADE,\
            UNIQUE (collection_id, media_item_id)\
         );",
    )?;

    let source_provider_expr = if collection_has_source_provider {
        "COALESCE(c.source_provider_id, c.provider_id)"
    } else {
        "c.provider_id"
    };
    let source_external_expr = if collection_has_source_external {
        "COALESCE(c.source_external_id, c.external_id)"
    } else {
        "c.external_id"
    };
    let relation_expr =
        if collection_has_relation { "COALESCE(c.relation_kind, 'primary')" } else { "'primary'" };
    let can_join_metadata_links = metadata_links_table_exists
        && item_has_collection_id
        && (item_has_metadata_link_id || item_has_media_item_id);
    let metadata_link_join = if can_join_metadata_links && item_has_metadata_link_id {
        Some("ml.id = ci.metadata_link_id".to_string())
    } else if can_join_metadata_links && item_has_media_item_id {
        Some(format!(
            "ml.media_item_id = ci.media_item_id AND ml.provider_id = {source_provider_expr}"
        ))
    } else {
        None
    };
    let collection_link_join_sql = metadata_link_join
        .as_ref()
        .map(|join_condition| {
            format!(
                "LEFT JOIN metadata_collection_items ci ON ci.collection_id = c.id\n\
                 LEFT JOIN item_metadata_links ml ON {join_condition}"
            )
        })
        .unwrap_or_default();
    let locale_expr = if collection_has_locale && can_join_metadata_links {
        "COALESCE(c.locale_key, ml.locale_key, 'en-US')"
    } else if collection_has_locale {
        "COALESCE(c.locale_key, 'en-US')"
    } else if can_join_metadata_links {
        "COALESCE(ml.locale_key, 'en-US')"
    } else {
        "'en-US'"
    };
    let provider_locale_expr = if collection_has_provider_locale && can_join_metadata_links {
        "COALESCE(c.provider_locale_key, ml.provider_locale_key)"
    } else if collection_has_provider_locale {
        "c.provider_locale_key"
    } else if can_join_metadata_links {
        "ml.provider_locale_key"
    } else {
        "NULL"
    };
    let name_expr = if collection_has_name {
        if collection_has_relation {
            "CASE WHEN c.provider_id = 'themerr' AND COALESCE(c.relation_kind, 'primary') = 'secondary' THEN NULL ELSE NULLIF(TRIM(c.name), '') END"
        } else {
            "NULLIF(TRIM(c.name), '')"
        }
    } else {
        "c.external_id"
    };
    let overview_expr = if collection_has_overview { "c.overview" } else { "NULL" };
    let artwork_expr = if collection_has_artwork { "c.artwork_url" } else { "NULL" };
    let backdrop_expr = if collection_has_backdrop { "c.backdrop_url" } else { "NULL" };
    let theme_expr = if collection_has_theme { "c.theme_song_url" } else { "NULL" };
    let updated_expr = if collection_has_updated { "c.updated_at" } else { "NULL" };

    let collection_repair_sql = format!(
        r#"
INSERT OR IGNORE INTO metadata_collections_next (
    provider_id, external_id, source_provider_id, source_external_id,
    relation_kind, locale_key, provider_locale_key, name, overview,
    artwork_url, backdrop_url, theme_song_url, updated_at
)
SELECT DISTINCT
    c.provider_id,
    c.external_id,
    {source_provider_expr},
    {source_external_expr},
    {relation_expr},
    {locale_expr},
    {provider_locale_expr},
    {name_expr},
    {overview_expr},
    {artwork_expr},
    {backdrop_expr},
    {theme_expr},
    {updated_expr}
FROM metadata_collections c
{collection_link_join_sql}
ORDER BY {updated_expr} DESC, c.id DESC;
"#
    );
    conn.batch_execute(&collection_repair_sql)?;

    if let Some(join_condition) = metadata_link_join {
        let item_updated_expr = match (item_has_updated, collection_has_updated) {
            (true, true) => "COALESCE(ci.updated_at, c.updated_at)",
            (true, false) => "ci.updated_at",
            (false, true) => "c.updated_at",
            (false, false) => "NULL",
        };
        let membership_locale_expr = if collection_has_locale {
            "COALESCE(c.locale_key, ml.locale_key, 'en-US')"
        } else {
            "COALESCE(ml.locale_key, 'en-US')"
        };
        conn.batch_execute(&format!(
            r#"
INSERT OR IGNORE INTO metadata_collection_items_next (
    collection_id, media_item_id, metadata_link_id, updated_at
)
SELECT
    nc.id,
    ml.media_item_id,
    ml.id,
    {item_updated_expr}
FROM metadata_collection_items ci
JOIN metadata_collections c ON c.id = ci.collection_id
JOIN item_metadata_links ml ON {join_condition}
JOIN metadata_collections_next nc
    ON nc.provider_id = c.provider_id
   AND nc.external_id = c.external_id
   AND nc.source_provider_id = {source_provider_expr}
   AND nc.source_external_id = {source_external_expr}
   AND nc.relation_kind = {relation_expr}
   AND nc.locale_key = {membership_locale_expr}
ORDER BY {item_updated_expr} DESC, ci.id DESC;
"#
        ))?;
    }

    conn.batch_execute(
        "PRAGMA foreign_keys = OFF;\
         DROP INDEX IF EXISTS idx_metadata_collection_items_metadata_link_id;\
         DROP INDEX IF EXISTS idx_metadata_collection_items_media_item_id;\
         DROP INDEX IF EXISTS idx_metadata_collection_items_collection_id;\
         DROP TABLE IF EXISTS metadata_collection_items;\
         DROP TABLE metadata_collections;\
         ALTER TABLE metadata_collections_next RENAME TO metadata_collections;\
         ALTER TABLE metadata_collection_items_next RENAME TO metadata_collection_items;\
         CREATE INDEX idx_metadata_collection_items_collection_id \
            ON metadata_collection_items (collection_id);\
         CREATE INDEX idx_metadata_collection_items_media_item_id \
            ON metadata_collection_items (media_item_id);\
         CREATE INDEX idx_metadata_collection_items_metadata_link_id \
            ON metadata_collection_items (metadata_link_id);\
         PRAGMA foreign_keys = ON;",
    )
}

fn sqlite_table_exists(
    conn: &mut diesel::SqliteConnection,
    table_name: &str,
) -> diesel::result::QueryResult<bool> {
    use diesel::prelude::*;

    #[derive(diesel::QueryableByName)]
    struct CountRow {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        count: i64,
    }

    let escaped_table = table_name.replace('\'', "''");
    let row = diesel::sql_query(format!(
        "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = '{escaped_table}'"
    ))
    .get_result::<CountRow>(conn)?;

    Ok(row.count > 0)
}

fn sqlite_column_exists(
    conn: &mut diesel::SqliteConnection,
    table_name: &str,
    column_name: &str,
) -> diesel::result::QueryResult<bool> {
    use diesel::prelude::*;

    #[derive(diesel::QueryableByName)]
    struct CountRow {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        count: i64,
    }

    let escaped_table = table_name.replace('\'', "''");
    let escaped_column = column_name.replace('\'', "''");
    let row = diesel::sql_query(format!(
        "SELECT COUNT(*) AS count FROM pragma_table_info('{escaped_table}') WHERE name = '{escaped_column}'"
    ))
    .get_result::<CountRow>(conn)?;

    Ok(row.count > 0)
}

fn sqlite_column_not_null(
    conn: &mut diesel::SqliteConnection,
    table_name: &str,
    column_name: &str,
) -> diesel::result::QueryResult<bool> {
    use diesel::prelude::*;

    #[derive(diesel::QueryableByName)]
    struct CountRow {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        count: i64,
    }

    let escaped_table = table_name.replace('\'', "''");
    let escaped_column = column_name.replace('\'', "''");
    let row = diesel::sql_query(format!(
        "SELECT CAST(COALESCE(MAX(\"notnull\"), 0) AS BIGINT) AS count \
         FROM pragma_table_info('{escaped_table}') \
         WHERE name = '{escaped_column}'"
    ))
    .get_result::<CountRow>(conn)?;

    Ok(row.count != 0)
}

fn sqlite_migration_record_exists(
    conn: &mut diesel::SqliteConnection,
    version: &str,
) -> diesel::result::QueryResult<bool> {
    use diesel::prelude::*;

    #[derive(diesel::QueryableByName)]
    struct CountRow {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        count: i64,
    }

    let escaped_version = version.replace('\'', "''");
    let row = diesel::sql_query(format!(
        "SELECT COUNT(*) AS count FROM __diesel_schema_migrations WHERE version = '{escaped_version}'"
    ))
    .get_result::<CountRow>(conn)?;

    Ok(row.count > 0)
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
