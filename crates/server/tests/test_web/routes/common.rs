// lib imports
use rocket::http::Status;

// test imports
use crate::test_utils::{
    create_test_client,
    make_request,
};

#[rocket::async_test]
async fn test_root_route() {
    let client = create_test_client(Some("common_routes")).await;

    let response = make_request(
        Some(&client),
        "get",
        "/",
        None,
        None,
        Some(Status::Ok),
        Some(true),
    )
    .await;

    let content_type = response
        .headers
        .iter()
        .find(|header| header.name().as_str().eq_ignore_ascii_case("content-type"))
        .map(|header| header.value().to_string())
        .unwrap_or_default();

    assert!(
        content_type.contains("text/html"),
        "Expected HTML content type for the root route, got: {}",
        content_type
    );
    assert!(
        response.body.contains("<html") && response.body.contains("Koko"),
        "Expected the root route to serve the web client or fallback HTML page"
    );
}

#[rocket::async_test]
async fn test_root_route_serves_built_asset_when_available() {
    let client = create_test_client(Some("common_routes_assets")).await;

    let response = make_request(
        Some(&client),
        "get",
        "/",
        None,
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;

    let asset_path = response
        .body
        .split('"')
        .find(|segment| segment.starts_with("/assets/"));

    if let Some(asset_path) = asset_path {
        make_request(
            Some(&client),
            "get",
            asset_path,
            None,
            None,
            Some(Status::Ok),
            Some(false),
        )
        .await;
    }
}
