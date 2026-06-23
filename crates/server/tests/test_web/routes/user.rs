// lib imports
use rocket::http::Status;
use rocket::serde::json::{
    json,
    serde_json,
};
use rstest::rstest;

// test imports
use crate::test_utils::{
    create_test_client,
    create_test_user,
    login_user,
    make_request,
};

const TINY_PNG_BASE64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=";

#[rstest]
#[case("admin", "password123", true, Some("1234"))]
#[case("user", "userpass456", false, None)]
#[case("power-user", "complex!Pass@789", false, Some("9876"))]
#[test_attr(rocket::async_test)]
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

#[rocket::async_test]
async fn test_user_profile_fields_are_returned() {
    let client = create_test_client(Some("user_routes_profile_fields")).await;

    let response = make_request(
        Some(&client),
        "post",
        "/create_user",
        Some(json!({
            "username": "profileuser",
            "password": "password123",
            "admin": true,
            "birthday": "1984-10-26",
            "profile_image_upload": {
                "mime_type": "image/png",
                "data_base64": TINY_PNG_BASE64
            }
        })),
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;
    assert_eq!(response.body, "User created");

    let token = login_user(&client, "profileuser", "password123", Some(Status::Ok))
        .await
        .expect("Expected profile user to be able to log in");
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
    assert_eq!(json["current_user"]["birthday"], "1984-10-26");
    let image_url = json["current_user"]["profile_image_url"]
        .as_str()
        .expect("Expected uploaded profile image URL");
    assert!(image_url.starts_with("/api/v1/user-profile-images/profile-"));

    let image_response = client.get(image_url).dispatch().await;
    assert_eq!(image_response.status(), Status::Ok);
    let image_bytes = image_response
        .into_bytes()
        .await
        .expect("Expected uploaded profile image bytes");
    assert!(!image_bytes.is_empty());
}

#[rocket::async_test]
async fn test_admin_can_update_existing_user_profile_fields() {
    let client = create_test_client(Some("user_routes_update_profile")).await;

    let (status, _) = create_test_user(
        &client,
        "admin",
        "password123",
        true,
        None,
        Some(Status::Ok),
    )
    .await;
    assert_eq!(status, Status::Ok);

    let token = login_user(&client, "admin", "password123", Some(Status::Ok))
        .await
        .expect("Expected admin to be able to log in");
    let auth_header = Some(format!("Bearer {}", token));

    make_request(
        Some(&client),
        "post",
        "/create_user",
        Some(json!({
            "username": "viewer",
            "password": "password123",
            "admin": false
        })),
        auth_header.clone(),
        Some(Status::Ok),
        Some(false),
    )
    .await;

    let response = make_request(
        Some(&client),
        "put",
        "/api/v1/users/2",
        Some(json!({
            "username": "viewer-updated",
            "admin": false,
            "birthday": "2001-02-03",
            "profile_image_upload": {
                "mime_type": "image/png",
                "data_base64": TINY_PNG_BASE64
            }
        })),
        auth_header,
        Some(Status::Ok),
        Some(false),
    )
    .await;

    let json: serde_json::Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(json["username"], "viewer-updated");
    assert_eq!(json["admin"], false);
    assert_eq!(json["birthday"], "2001-02-03");
    assert!(
        json["profile_image_url"]
            .as_str()
            .unwrap()
            .starts_with("/api/v1/user-profile-images/profile-")
    );
}

#[rocket::async_test]
async fn test_cannot_demote_last_admin() {
    let client = create_test_client(Some("user_routes_last_admin")).await;

    let (status, _) = create_test_user(
        &client,
        "admin",
        "password123",
        true,
        None,
        Some(Status::Ok),
    )
    .await;
    assert_eq!(status, Status::Ok);

    let token = login_user(&client, "admin", "password123", Some(Status::Ok))
        .await
        .expect("Expected admin to be able to log in");
    let response = make_request(
        Some(&client),
        "put",
        "/api/v1/users/1",
        Some(json!({
            "username": "admin",
            "admin": false,
            "birthday": null,
            "remove_profile_image": true
        })),
        Some(format!("Bearer {}", token)),
        Some(Status::BadRequest),
        Some(false),
    )
    .await;
    assert_eq!(response.status, Status::BadRequest);
}
