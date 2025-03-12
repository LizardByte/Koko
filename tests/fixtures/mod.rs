use diesel::sqlite::SqliteConnection;
use diesel::Connection;
use diesel_migrations::MigrationHarness;
use koko::globals::CURRENT_ENV;
use koko::web::rocket;
use rocket::local::asynchronous::Client;
use rstest::fixture;
use std::fs;
use std::path::Path;

use koko::db::MIGRATIONS;

pub struct TestDb {
    pub client: Client,
}

impl Drop for TestDb {
    fn drop(&mut self) {
        let db_path = Path::new("./test_data/koko.db");
        if db_path.exists() {
            if let Ok(mut conn) = SqliteConnection::establish(db_path.to_str().unwrap()) {
                let _ = conn.revert_all_migrations(MIGRATIONS);
            }
        }
    }
}

#[fixture]
pub async fn db_fixture() -> TestDb {
    CURRENT_ENV.store(1, std::sync::atomic::Ordering::SeqCst);

    let db_path = Path::new("./test_data/koko.db");
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).expect("Failed to create test_data directory");
    }

    // Initialize database with migrations
    if let Ok(mut conn) = SqliteConnection::establish(db_path.to_str().unwrap()) {
        conn.run_pending_migrations(MIGRATIONS)
            .expect("Failed to run migrations");
    }

    let rocket = rocket();
    let client = Client::tracked(rocket)
        .await
        .expect("Failed to launch web server");

    TestDb { client }
}
