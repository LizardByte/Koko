// lib imports
use rocket::http::Status;
use rstest::rstest;
use serde_json::json;
use serial_test::serial;

// test imports
use crate::fixtures;
use crate::test_web::test_request;

#[rstest]
#[serial(db)]
#[tokio::test]
async fn test_create_first_user(
    #[future]
    #[from(fixtures::db_fixture)]
    #[with(true)]
    db_future: fixtures::TestDb
) {
    db_future.await;

    // nothing to do, the fixture handles creating the first user
}

#[rstest]
#[serial(db)]
#[tokio::test]
#[case::create_user_requires_auth("testuser", "password123", false, Status::Unauthorized)]
async fn test_create_subsequent_user_requires_auth(
    #[future]
    #[from(fixtures::db_fixture)]
    #[with(true)]
    db_future: fixtures::TestDb,
    #[case] username: &str,
    #[case] password: &str,
    #[case] admin: bool,
    #[case] expected_status: Status,
) {
    let db = db_future.await;

    // Try to create second user without auth
    test_request(
        "post",
        "/create_user",
        Some(json!({
            "username": username,
            "password": password,
            "admin": admin
        })),
        expected_status,
        Some(&db.client),
    )
    .await;
}
