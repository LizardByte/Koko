//! Integration tests for authentication routes.

// lib imports
use rocket::http::Status;
use rstest::rstest;
use serde_json::json;

// test imports
use crate::test_utils::{
    create_and_login_user,
    create_test_client,
    create_test_user,
    make_request,
};

#[rocket::async_test]
async fn test_create_first_user_no_auth_required() {
    let client = create_test_client(Some("auth_first_user")).await;

    let user_data = json!({
        "username": "admin",
        "password": "admin123",
        "admin": true
    });

    let response = make_request(
        Some(&client),
        "post",
        "/create_user",
        Some(user_data),
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;
    assert_eq!(response.body, "User created");
}

#[rstest]
#[case("testuser", "testpass123", false, None, "regular user")]
#[case("admin", "adminpass456", true, None, "admin user")]
#[case("userpin", "pass789", false, Some("1234"), "user with PIN")]
#[case("admin_pin", "adminpass", true, Some("5678"), "admin user with PIN")]
async fn test_login_with_valid_credentials(
    #[case] username: &str,
    #[case] password: &str,
    #[case] admin: bool,
    #[case] pin: Option<&str>,
    #[case] _description: &str,
) {
    let client = create_test_client(Some(&format!("auth_login_valid_{}", username))).await;

    // Create and login user using the helper function
    let token = create_and_login_user(&client, username, password, admin, pin)
        .await
        .expect("Should create and login user successfully");

    // Verify we got a valid token
    assert!(!token.is_empty());
}

#[rstest]
#[case("nonexistent", "wrongpass", "non-existent user")]
#[case("", "", "empty credentials")]
#[case("user", "", "empty password")]
#[case("", "password", "empty username")]
async fn test_login_with_invalid_credentials(
    #[case] username: &str,
    #[case] password: &str,
    #[case] _description: &str,
) {
    let client = create_test_client(Some(&format!(
        "auth_login_invalid_{}",
        username.replace("", "empty")
    )))
    .await;

    // Try to login with invalid credentials
    let login_data = json!({
        "username": username,
        "password": password
    });

    make_request(
        Some(&client),
        "post",
        "/login",
        Some(login_data),
        None,
        Some(Status::Unauthorized),
        Some(false),
    )
    .await;
}

#[rstest]
#[case("testuser2", "correctpass", "wrongpass")]
#[case("admin", "admin123", "notadmin")]
#[case("user", "mypassword", "yourpassword")]
async fn test_login_with_wrong_password(
    #[case] username: &str,
    #[case] correct_password: &str,
    #[case] wrong_password: &str,
) {
    let client = create_test_client(Some(&format!("auth_wrong_password_{}", username))).await;

    // Create a user
    let (status, _) = create_test_user(
        &client,
        username,
        correct_password,
        false,
        None,
        Some(Status::Ok),
    )
    .await;
    assert_eq!(status, Status::Ok);

    // Try to login with the wrong password
    let login_data = json!({
        "username": username,
        "password": wrong_password
    });

    let response = make_request(
        Some(&client),
        "post",
        "/login",
        Some(login_data),
        None,
        Some(Status::Unauthorized),
        Some(false),
    )
    .await;
    assert_eq!(response.status, Status::Unauthorized);
}

#[rstest]
#[case("/jwt_test", "GET")]
#[case("/admin_test", "GET")]
async fn test_jwt_protected_routes_without_token(
    #[case] route: &str,
    #[case] method: &str,
) {
    let client = create_test_client(Some("auth_no_token")).await;

    let response = make_request(
        Some(&client),
        &method.to_lowercase(),
        route,
        None,
        None,
        Some(Status::Unauthorized),
        Some(false),
    )
    .await;
    assert_eq!(response.status, Status::Unauthorized);
}

#[rstest]
#[case("Bearer invalid_token")]
#[case("Bearer ")]
#[case("invalid_token")]
#[case("Bearer malformed.jwt.token")]
async fn test_jwt_protected_routes_with_invalid_token(#[case] auth_header: &str) {
    let client = create_test_client(Some("auth_invalid_token")).await;

    let _response = make_request(
        Some(&client),
        "get",
        "/jwt_test",
        None,
        Some(auth_header.to_string()),
        Some(Status::Unauthorized),
        Some(false),
    )
    .await;
    // Status assertion is now handled by expected_status parameter
}

#[rocket::async_test]
async fn test_jwt_protected_route_with_valid_token() {
    let client = create_test_client(Some("auth_valid_token")).await;

    // Create and login user to get a token
    let token = create_and_login_user(&client, "jwtuser", "jwtpass123", false, None)
        .await
        .expect("Should create and login user successfully");

    // Use the token to access a protected route
    let auth_header = format!("Bearer {}", token);
    let response = make_request(
        Some(&client),
        "get",
        "/jwt_test",
        None,
        Some(auth_header),
        Some(Status::Ok),
        Some(false),
    )
    .await;
    assert_eq!(response.body, "Protected Page");
}

#[rocket::async_test]
async fn test_admin_route_with_non_admin_user() {
    let client = create_test_client(Some("auth_non_admin")).await;

    // Create and login non-admin user
    let token = create_and_login_user(&client, "regularuser", "regularpass", false, None)
        .await
        .expect("Should create and login user successfully");

    // Try to access the admin route
    let auth_header = format!("Bearer {}", token);
    let _response = make_request(
        Some(&client),
        "get",
        "/admin_test",
        None,
        Some(auth_header),
        Some(Status::Forbidden),
        Some(false),
    )
    .await;
    // Status assertion is now handled by expected_status parameter
}

#[rocket::async_test]
async fn test_admin_route_with_admin_user() {
    let client = create_test_client(Some("auth_admin")).await;

    // Create and login admin user
    let token = create_and_login_user(&client, "adminuser", "adminpass", true, None)
        .await
        .expect("Should create and login admin user successfully");

    // Access admin route
    let auth_header = format!("Bearer {}", token);
    let response = make_request(
        Some(&client),
        "get",
        "/admin_test",
        None,
        Some(auth_header),
        Some(Status::Ok),
        Some(false),
    )
    .await;
    assert_eq!(response.body, "Admin only content");
}

#[rocket::async_test]
async fn test_create_user_requires_admin_after_first_user() {
    let client = create_test_client(Some("auth_admin_required")).await;

    // Create first user (admin)
    let (status, _) = create_test_user(
        &client,
        "firstadmin",
        "adminpass",
        true,
        None,
        Some(Status::Ok),
    )
    .await;
    assert_eq!(status, Status::Ok);

    // Try to create a second user without authentication (should fail)
    let (status, _) = create_test_user(
        &client,
        "seconduser",
        "userpass",
        false,
        None,
        Some(Status::Unauthorized),
    )
    .await;
    assert_eq!(status, Status::Unauthorized);
}

#[rocket::async_test]
async fn test_create_user_with_pin() {
    let client = create_test_client(Some("auth_with_pin")).await;

    // Create a user with PIN
    let (status, body) = create_test_user(
        &client,
        "pinuser",
        "userpass",
        false,
        Some("1234"),
        Some(Status::Ok),
    )
    .await;
    assert_eq!(status, Status::Ok);
    assert_eq!(body, "User created");
}

#[rocket::async_test]
async fn test_create_user_with_invalid_pin() {
    let client = create_test_client(Some("auth_invalid_pin")).await;

    // Try to create user with invalid PIN (too short)
    let invalid_pin_data = json!({
        "username": "badpinuser",
        "password": "userpass",
        "pin": "123",
        "admin": false
    });

    let _response = make_request(
        Some(&client),
        "post",
        "/create_user",
        Some(invalid_pin_data),
        None,
        Some(Status::BadRequest),
        Some(false),
    )
    .await;

    // Try to create user with invalid PIN (too long)
    let invalid_pin_data = json!({
        "username": "badpinuser2",
        "password": "userpass",
        "pin": "1234567",
        "admin": false
    });

    let _response = make_request(
        Some(&client),
        "post",
        "/create_user",
        Some(invalid_pin_data),
        None,
        Some(Status::BadRequest),
        Some(false),
    )
    .await;

    // Try to create user with non-numeric PIN
    let invalid_pin_data = json!({
        "username": "badpinuser3",
        "password": "userpass",
        "pin": "abcd",
        "admin": false
    });

    let _response = make_request(
        Some(&client),
        "post",
        "/create_user",
        Some(invalid_pin_data),
        None,
        Some(Status::BadRequest),
        Some(false),
    )
    .await;
}
