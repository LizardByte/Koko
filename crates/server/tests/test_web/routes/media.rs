// lib imports
use rocket::http::Status;
use rocket::serde::json::{
    Value,
    serde_json,
};

// test imports
use crate::test_utils::{
    create_test_client,
    make_request,
};

#[rocket::async_test]
async fn test_get_server_capabilities_route() {
    let client = create_test_client(Some("media_capabilities_route")).await;

    let response = make_request(
        Some(&client),
        "get",
        "/api/v1/system/capabilities",
        None,
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;

    let json: Value = serde_json::from_str(&response.body).unwrap();
    let object = json.as_object().unwrap();

    assert!(object.contains_key("app_name"));
    assert!(object.contains_key("version"));
    assert!(object.contains_key("server_url"));
    assert!(object.contains_key("https_enabled"));
    assert!(object.contains_key("libraries_configured"));
    assert!(!object.contains_key("ffmpeg_strategy"));
    assert!(object.contains_key("api_versions"));
    assert!(object.contains_key("transcoding"));
}

#[rocket::async_test]
async fn test_get_metadata_providers_route() {
    let client = create_test_client(Some("metadata_providers_route")).await;

    let response = make_request(
        Some(&client),
        "get",
        "/api/v1/metadata/providers",
        None,
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;

    let json: Value = serde_json::from_str(&response.body).unwrap();
    assert!(
        json.is_array(),
        "Expected /api/v1/metadata/providers to return a JSON array"
    );
}

#[rocket::async_test]
async fn test_get_libraries_route() {
    let client = create_test_client(Some("media_libraries_route")).await;

    let response = make_request(
        Some(&client),
        "get",
        "/api/v1/libraries",
        None,
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;

    let json: Value = serde_json::from_str(&response.body).unwrap();
    assert!(
        json.is_array(),
        "Expected /api/v1/libraries to return a JSON array"
    );
}

#[rocket::async_test]
async fn test_get_library_inventory_missing_route() {
    let client = create_test_client(Some("media_library_inventory_missing_route")).await;

    make_request(
        Some(&client),
        "get",
        "/api/v1/libraries/999/files",
        None,
        None,
        Some(Status::NotFound),
        Some(false),
    )
    .await;
}

#[rocket::async_test]
async fn test_refresh_library_metadata_missing_route() {
    let client = create_test_client(Some("media_refresh_library_metadata_missing_route")).await;

    make_request(
        Some(&client),
        "post",
        "/api/v1/libraries/999/metadata/refresh",
        None,
        None,
        Some(Status::NotFound),
        Some(false),
    )
    .await;
}

#[rocket::async_test]
async fn test_get_items_route() {
    let client = create_test_client(Some("media_items_route")).await;

    let response = make_request(
        Some(&client),
        "get",
        "/api/v1/items",
        None,
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;

    let json: Value = serde_json::from_str(&response.body).unwrap();
    assert!(
        json.is_array(),
        "Expected /api/v1/items to return a JSON array"
    );
}

#[rocket::async_test]
async fn test_get_item_missing_route() {
    let client = create_test_client(Some("media_item_missing_route")).await;

    make_request(
        Some(&client),
        "get",
        "/api/v1/items/999",
        None,
        None,
        Some(Status::NotFound),
        Some(false),
    )
    .await;
}

#[rocket::async_test]
async fn test_get_item_metadata_missing_route() {
    let client = create_test_client(Some("media_item_metadata_missing_route")).await;

    make_request(
        Some(&client),
        "get",
        "/api/v1/items/999/metadata",
        None,
        None,
        Some(Status::NotFound),
        Some(false),
    )
    .await;
}

#[rocket::async_test]
async fn test_search_items_route() {
    let client = create_test_client(Some("media_search_route")).await;

    let response = make_request(
        Some(&client),
        "get",
        "/api/v1/search?query=test",
        None,
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;

    let json: Value = serde_json::from_str(&response.body).unwrap();
    assert!(
        json.is_array(),
        "Expected /api/v1/search to return a JSON array"
    );
}
