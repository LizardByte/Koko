//! Database utilities for the application.

pub(crate) mod models;
pub(crate) mod schema;

// lib imports
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
pub fn configure_sqlite_connection(conn: &mut diesel::SqliteConnection) -> diesel::result::QueryResult<()> {
    conn.batch_execute(
        "PRAGMA foreign_keys = ON;\
         PRAGMA journal_mode = WAL;\
         PRAGMA synchronous = NORMAL;\
         PRAGMA busy_timeout = 5000;",
    )
}

/// Database connection fairing.
#[database("sqlite_db")]
pub struct DbConn(diesel::SqliteConnection);

/// Fairing to run migrations when the application starts.
pub struct Migrate;

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

impl rocket_okapi::request::OpenApiFromRequest<'_> for DbConn {
    fn from_request_input(
        _gen: &mut rocket_okapi::gen::OpenApiGenerator,
        _name: String,
        _required: bool,
    ) -> rocket_okapi::Result<rocket_okapi::request::RequestHeaderInput> {
        Ok(rocket_okapi::request::RequestHeaderInput::None)
    }
}
