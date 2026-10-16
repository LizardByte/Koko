// lib imports
use rocket::http::{
    ContentType,
    Status,
};

use serde_json::Value;

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

#[rocket::async_test]
async fn test_favicon_serves_application_icon_without_authentication() {
    let client = create_test_client(Some("favicon")).await;
    let response = client.get("/favicon.ico").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::Icon));
    let bytes = response.into_bytes().await.expect("Expected icon bytes");
    assert_eq!(bytes, include_bytes!("../../../../../assets/icon.ico"));

    let response = client.head("/favicon.ico").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::Icon));
    assert!(response.into_bytes().await.unwrap_or_default().is_empty());
}

#[rocket::async_test]
async fn test_all_provider_descriptors_serve_local_logo_assets() {
    let client = create_test_client(Some("provider_logo_assets")).await;
    let providers_response = client.get("/api/v1/metadata/providers").dispatch().await;
    assert_eq!(providers_response.status(), Status::Ok);
    let providers: Vec<Value> = providers_response
        .into_json()
        .await
        .expect("Expected provider descriptors");
    assert_eq!(providers.len(), 7);

    for provider in providers {
        for field in [
            "logo_light_url",
            "logo_dark_url",
        ] {
            let url = provider[field]
                .as_str()
                .expect("Every provider must supply both logo variants");
            assert!(
                url.starts_with("/assets/providers/"),
                "Expected a local logo: {url}"
            );
            let response = client.get(url).dispatch().await;
            assert_eq!(response.status(), Status::Ok, "Missing logo: {url}");
            let content_type = response
                .content_type()
                .expect("Expected an image content type");
            let bytes = response.into_bytes().await.expect("Expected logo bytes");
            if url.ends_with(".svg") {
                assert_eq!(content_type, ContentType::SVG);
                let svg = std::str::from_utf8(&bytes).expect("Expected a UTF-8 SVG");
                assert!(svg.contains("<svg") && svg.contains("</svg>"));
                assert!(!svg.contains("<script") && !svg.contains("href=\"http"));
            } else {
                assert_eq!(content_type, ContentType::PNG);
                let image = image::load_from_memory(&bytes).expect("Expected a valid PNG");
                assert!(image.width() > 0 && image.height() > 0);
            }
        }
    }
}

#[rocket::async_test]
async fn test_provider_asset_route_rejects_unknown_files() {
    let client = create_test_client(Some("provider_logo_unknown")).await;
    for path in [
        "/assets/providers/unknown.svg",
        "/assets/providers/unknown",
        "/assets/providers/README.md",
    ] {
        let response = client.get(path).dispatch().await;
        assert_eq!(
            response.status(),
            Status::NotFound,
            "Unexpected asset: {path}"
        );
    }
}
