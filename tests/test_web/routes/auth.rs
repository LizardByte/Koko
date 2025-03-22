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
#[case::login_success("admin", "password123", Status::Ok)]
#[case::login_wrong_password("admin", "wrong", Status::Unauthorized)]
#[case::login_non_existent_user("nonexistent", "wrong", Status::Unauthorized)]
async fn test_login(
    #[future]
    #[from(fixtures::db_fixture)]
    #[with(true)]
    db_future: fixtures::TestDb,
    #[case] username: &str,
    #[case] password: &str,
    #[case] expected_status: Status,
) {
    let db = db_future.await;

    // Test login
    let response = test_request(
        "post",
        "/login",
        Some(json!({
            "username": username,
            "password": password,
        })),
        expected_status,
        Some(&db.client),
    )
    .await;

    if expected_status == Status::Ok {
        assert!(response.body.contains("token"));
    }
}

#[rocket::async_test]
async fn test_logout_route() {
    test_request("get", "/logout", None, Status::Ok, None).await;
}
