// lib imports
use rocket::http::Status;
use rocket::serde::json::serde_json;
use rstest::rstest;

// test imports
use crate::test_utils::{create_test_client, create_test_user, login_user, make_request};

#[rstest]
#[case("admin", "password123", true, Some("1234"))]
#[case("user", "userpass456", false, None)]
#[case("power-user", "complex!Pass@789", false, Some("9876"))]
async fn test_create_first_user_scenarios(
    #[case] username: &str,
    #[case] password: &str,
    #[case] admin: bool,
    #[case] pin: Option<&str>,
) {
    let client = create_test_client(Some(&format!("user_routes_first_{}", username))).await;

    // Create the first user (should not require authentication)
    let (status, body) =
        create_test_user(&client, username, password, admin, pin, Some(Status::Ok)).await;

    assert_eq!(status, Status::Ok);
    assert_eq!(body, "User created");
}

#[rocket::async_test]
async fn test_create_user_requires_auth() {
    let client = create_test_client(Some("user_routes_requires_auth")).await;

    // First, create a user to populate the database
    let (status, _) = create_test_user(
        &client,
        "admin",
        "password123",
        true,
        Some("1234"),
        Some(Status::Ok),
    )
    .await;
    assert_eq!(status, Status::Ok);

    // Try to create a second user without auth (should fail)
    let (status, _) = create_test_user(
        &client,
        "test_user",
        "password123",
        false,
        None,
        Some(Status::Unauthorized),
    )
    .await;
    assert_eq!(status, Status::Unauthorized);
}

#[rocket::async_test]
async fn test_bootstrap_reports_no_users_before_setup() {
    let client = create_test_client(Some("user_routes_bootstrap_empty")).await;

    let response = make_request(
        Some(&client),
        "get",
        "/api/v1/bootstrap",
        None,
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;

    let json: serde_json::Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(json["has_users"], false);
    assert!(json["current_user"].is_null());
}

#[rocket::async_test]
async fn test_first_created_user_is_forced_to_admin() {
    let client = create_test_client(Some("user_routes_first_forced_admin")).await;

    let (status, _) = create_test_user(
        &client,
        "firstuser",
        "password123",
        false,
        None,
        Some(Status::Ok),
    )
    .await;
    assert_eq!(status, Status::Ok);

    let token = login_user(&client, "firstuser", "password123", Some(Status::Ok))
        .await
        .expect("Expected first user to be able to log in");
    let response = make_request(
        Some(&client),
        "get",
        "/api/v1/bootstrap",
        None,
        Some(format!("Bearer {}", token)),
        Some(Status::Ok),
        Some(false),
    )
    .await;

    let json: serde_json::Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(json["has_users"], true);
    assert_eq!(json["current_user"]["username"], "firstuser");
    assert_eq!(json["current_user"]["admin"], true);
}

