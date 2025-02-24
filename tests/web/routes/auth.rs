use rocket::http::Status;
use rstest::rstest;
use serde_json::json;
use serial_test::serial;

use crate::fixtures;
use crate::web::test_post_json;
use crate::web::test_route;

#[rstest]
#[serial(db)]
#[tokio::test]
#[case::login_success("testuser", "testuser", "password123", "password123", Status::Ok)]
#[case::login_invalid(
    "testuser",
    "nonexistent",
    "password123",
    "wrong",
    Status::Unauthorized
)]
async fn test_login(
    #[future]
    #[from(fixtures::db_fixture)]
    db_future: fixtures::TestDb,
    #[case] username_0: &str,
    #[case] username_1: &str,
    #[case] password_0: &str,
    #[case] password_1: &str,
    #[case] expected_status: Status,
) {
    let db = db_future.await;
    let client = &db.client;

    // Create test user
    test_post_json(
        client,
        "/create_user",
        json!({
            "username": username_0,
            "password": password_0,
            "admin": false
        }),
        Status::Ok,
    )
    .await;

    // Test login
    let response = test_post_json(
        client,
        "/login",
        json!({
            "username": username_1,
            "password": password_1
        }),
        expected_status,
    )
    .await;

    if expected_status == Status::Ok {
        assert!(response.body.contains("token"));
    }
}

#[rocket::async_test]
async fn test_logout_route() {
    test_route("/logout", Status::Ok).await;
}
