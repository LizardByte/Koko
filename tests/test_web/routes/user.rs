use rocket::http::Status;
use rstest::rstest;
use serde_json::json;
use serial_test::serial;

use crate::fixtures;
use crate::test_web::test_request;

#[rstest]
#[serial(db)]
#[tokio::test]
#[case::create_user_success("testuser", "password123", "1234", true, Status::Ok)]
async fn test_create_first_user(
    #[future]
    #[from(fixtures::db_fixture)]
    db_future: fixtures::TestDb,
    #[case] username: &str,
    #[case] password: &str,
    #[case] pin: &str,
    #[case] admin: bool,
    #[case] expected_status: Status,
) {
    let db = db_future.await;
    let client = &db.client;

    let response = test_request(
        "post",
        "/create_user",
        Some(json!({
            "username": username,
            "password": password,
            "pin": pin,
            "admin": admin
        })),
        expected_status,
        Some(client),
    )
    .await;

    assert_eq!(response.body, "User created");
}

#[rstest]
#[serial(db)]
#[tokio::test]
#[case::create_user_requires_auth("testuser", "password123", false, Status::Unauthorized)]
async fn test_create_subsequent_user_requires_auth(
    #[future]
    #[from(fixtures::db_fixture)]
    db_future: fixtures::TestDb,
    #[case] username: &str,
    #[case] password: &str,
    #[case] admin: bool,
    #[case] expected_status: Status,
) {
    let db = db_future.await;
    let client = &db.client;

    // Create first user
    test_request(
        "post",
        "/create_user",
        Some(json!({
            "username": "user1",
            "password": "password123",
            "admin": true
        })),
        Status::Ok,
        Some(client),
    )
    .await;

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
        Some(client),
    )
    .await;
}
