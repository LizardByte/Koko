use diesel::sqlite::SqliteConnection;
use diesel::Connection;
use diesel_migrations::MigrationHarness;
use koko::globals::CURRENT_ENV;
use koko::web::rocket;
use once_cell::sync::Lazy;
use rocket::local::asynchronous::Client;
use rstest::fixture;
use std::fs;
use std::path::Path;

use koko::db::MIGRATIONS;

// constants
static DB_PATH: Lazy<&'static Path> = Lazy::new(|| Path::new("./test_data/koko.db"));

pub struct TestDb {
    pub client: Client,
}

impl Drop for TestDb {
    fn drop(&mut self) {
        if DB_PATH.exists() {
            if let Ok(mut conn) = SqliteConnection::establish(DB_PATH.to_str().unwrap()) {
                let _ = conn.revert_all_migrations(MIGRATIONS);
            }
        }
    }
}

#[fixture]
pub async fn db_fixture() -> TestDb {
    CURRENT_ENV.store(1, std::sync::atomic::Ordering::SeqCst);

    if let Some(parent) = DB_PATH.parent() {
        fs::create_dir_all(parent).expect("Failed to create test_data directory");
    }

    // Initialize database with migrations
    if let Ok(mut conn) = SqliteConnection::establish(DB_PATH.to_str().unwrap()) {
        conn.run_pending_migrations(MIGRATIONS)
            .expect("Failed to run migrations");
    }

    let rocket = rocket();
    let client = Client::tracked(rocket)
        .await
        .expect("Failed to launch web server");

    TestDb { client }
}
