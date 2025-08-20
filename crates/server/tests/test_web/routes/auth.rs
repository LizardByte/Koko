// lib imports
use rocket::http::Status;
use serde_json::json;

// test imports
use crate::test_utils::{
    create_and_login_user,
    create_test_client,
    create_test_user,
    make_request,
};

#[rocket::async_test]
async fn test_login_success() {
    let client = create_test_client(Some("auth_routes_login_success")).await;

    // Create and login user using the helper function
    let token = create_and_login_user(&client, "admin", "password123", true, Some("1234"))
        .await
        .expect("Should create and login user successfully");

    // Verify we got a valid token
    assert!(!token.is_empty());
}

#[rocket::async_test]
async fn test_login_wrong_password() {
    let client = create_test_client(Some("auth_routes_wrong_password")).await;

    // Create a user
    let (_status, _) = create_test_user(
        &client,
        "admin",
        "password123",
        true,
        Some("1234"),
        Some(Status::Ok),
    )
    .await;

    // Test login with the wrong password
    let login_data = json!({
        "username": "admin",
        "password": "wrong"
    });

    let _response = make_request(
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

#[rocket::async_test]
async fn test_login_non_existent_user() {
    let client = create_test_client(Some("auth_routes_non_existent")).await;

    // Test login with a non-existent user (no users created)
    let login_data = json!({
        "username": "nonexistent",
        "password": "wrong"
    });

    let _response = make_request(
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

#[rocket::async_test]
async fn test_logout_route() {
    let client = create_test_client(Some("auth_routes_logout")).await;

    let response = make_request(
        Some(&client),
        "get",
        "/logout",
        None,
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;
    assert_eq!(response.body, "Logout Page");
}
