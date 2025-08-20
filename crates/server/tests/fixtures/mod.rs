// standard imports
use std::fs;
use std::path::PathBuf;

// lib imports
use diesel::Connection;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::MigrationHarness;
use rocket::http::Status;
use rocket::local::asynchronous::Client;
use rstest::fixture;
use serde_json::json;

// local imports
use koko::db::MIGRATIONS;
use koko::globals::CURRENT_ENV;
use koko::web::rocket;

// test imports
use crate::test_utils::{TestResponse, make_request};

pub struct TestDb {
    pub client: Client,
    db_path: PathBuf,
}

impl Drop for TestDb {
    fn drop(&mut self) {
        if self.db_path.exists() {
            if let Ok(mut conn) = SqliteConnection::establish(self.db_path.to_str().unwrap()) {
                let _ = conn.revert_all_migrations(MIGRATIONS);
            }

            // Sleep to allow processes to release the database file
            std::thread::sleep(std::time::Duration::from_secs(1));

            // Delete the database file
            match fs::remove_file(&self.db_path) {
                Ok(_) => (),
                Err(e) => eprintln!("Warning: Failed to delete test database: {}", e),
            }
        }
    }
}

#[fixture]
pub async fn db_fixture(#[default(false)] base_user: bool) -> TestDb {
    CURRENT_ENV.store(1, std::sync::atomic::Ordering::SeqCst);

    // Create a unique database file for this test
    let test_id = std::thread::current().id();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let db_path = PathBuf::from(format!(
        "./test_data/test_{}_{}.db",
        timestamp,
        format!("{:?}", test_id)
            .replace("ThreadId(", "")
            .replace(")", "")
    ));

    // Ensure test_data directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create test_data directory");
    }

    // Set the database URL for this test
    std::env::set_var("DATABASE_URL", format!("sqlite:{}", db_path.display()));

    let rocket_instance = rocket();
    let client = Client::tracked(rocket_instance)
        .await
        .expect("Failed to launch rocket for test");

    if base_user {
        let response: TestResponse = make_request(
            Some(&client),
            "post",
            "/create_user",
            Some(json!({
                "username": "admin",
                "password": "password123",
                "pin": "1234",
                "admin": true,
            })),
            None,
            Some(Status::Ok),
            Some(false),
        )
        .await;

        assert_eq!(response.body, "User created");
    }

    TestDb { client, db_path }
}
