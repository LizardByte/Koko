// standard imports
use std::fs;
use std::path::Path;

// lib imports
use diesel::Connection;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::MigrationHarness;
use once_cell::sync::Lazy;
use rocket::http::Status;
use rocket::local::asynchronous::Client;
use rstest::fixture;
use serde_json::json;

// local imports
use koko::db::MIGRATIONS;
use koko::globals::CURRENT_ENV;
use koko::web::rocket;

// test imports
use crate::test_web::test_request;

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

            // Sleep to try to all the processes to release the database file
            std::thread::sleep(std::time::Duration::from_secs(1));

            // Delete the database file
            match fs::remove_file(DB_PATH.clone()) {
                Ok(_) => (),
                Err(e) => eprintln!("Warning: Failed to delete test database: {}", e),
            }
        }
    }
}

#[fixture]
pub async fn db_fixture(#[default(false)] base_user: bool) -> TestDb {
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

    if base_user {
        let response = test_request(
            "post",
            "/create_user",
            Some(json!({
                "username": "admin",
                "password": "password123",
                "pin": "1234",
                "admin": true,
            })),
            Status::Ok,
            Some(&client),
        )
        .await;

        assert_eq!(response.body, "User created");
    }

    TestDb { client }
}
