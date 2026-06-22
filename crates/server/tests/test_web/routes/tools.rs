// lib imports
use rocket::http::Status;

// test imports
use crate::test_utils::{create_test_client, create_test_user, login_user, make_request};

/// The discover endpoint must reject unauthenticated callers. It is
/// admin-gated, so without a valid admin token it must never return 200.
#[rocket::async_test]
async fn discover_requires_admin() {
    let client = create_test_client(Some("tools_discover_requires_admin")).await;

    // Unauthenticated -> forbidden/unauthorized, never 200.
    let response = make_request(
        Some(&client),
        "post",
        "/api/v1/system/tools/discover",
        None,
        None,
        None,
        Some(false),
    )
    .await;
    assert!(
        response.status == Status::Unauthorized || response.status == Status::Forbidden,
        "expected auth failure, got {}",
        response.status
    );
}

/// An admin caller can reach the discover endpoint and gets a well-formed
/// response with the expected top-level fields.
#[rocket::async_test]
async fn discover_returns_candidates_for_admin() {
    let client = create_test_client(Some("tools_discover_admin")).await;

    // Set up an admin user and log in.
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
        .expect("admin login should succeed");

    let response = make_request(
        Some(&client),
        "post",
        "/api/v1/system/tools/discover",
        None,
        Some(format!("Bearer {}", token)),
        Some(Status::Ok),
        Some(false),
    )
    .await;

    let json: serde_json::Value =
        serde_json::from_str(&response.body).expect("discover response should be valid JSON");
    // Top-level fields must be present (regardless of whether ffmpeg exists).
    assert!(
        json.get("configured_ffmpeg").is_some(),
        "missing configured_ffmpeg"
    );
    assert!(
        json.get("configured_ffprobe").is_some(),
        "missing configured_ffprobe"
    );
    assert!(json.get("candidates").is_some(), "missing candidates");
    assert!(
        json["candidates"].is_array(),
        "candidates should be an array"
    );
}

/// The session-status endpoint returns 404 for an unknown session with no
/// recorded error.
#[rocket::async_test]
async fn session_status_unknown_session_is_404() {
    let client = create_test_client(Some("tools_status_unknown")).await;
    let response = make_request(
        Some(&client),
        "get",
        "/api/v1/sessions/does-not-exist/status",
        None,
        None,
        Some(Status::NotFound),
        Some(false),
    )
    .await;
    assert_eq!(response.status, Status::NotFound);
}
