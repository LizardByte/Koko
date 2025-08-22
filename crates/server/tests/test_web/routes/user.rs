// lib imports
use rocket::http::Status;
use rstest::rstest;

// test imports
use crate::test_utils::{create_test_client, create_test_user};

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
