// mod imports
mod routes;
mod test_auth_routes;

// lib imports
use rocket::http::Status;

// local imports
use koko::web;

// test imports
use crate::test_utils::{
    create_test_client,
    make_request,
};

#[rocket::async_test]
async fn test_openapi_schema_document() {
    let client = create_test_client(Some("openapi_schema")).await;
    let response = client.get("/openapi.json").dispatch().await;
    assert_eq!(response.status(), Status::Ok);

    let document: serde_json::Value = response.into_json().await.unwrap();
    assert_eq!(document["openapi"], "3.0.0");
    assert_eq!(
        document["paths"]["/login"]["post"]["requestBody"]["content"]["application/json"]["schema"]
            ["$ref"],
        "#/components/schemas/LoginForm"
    );

    let schemas = document["components"]["schemas"].as_object().unwrap();
    for name in [
        "LoginForm",
        "TokenResponse",
        "BootstrapResponse",
        "PackageResponse",
        "SettingsResponse",
        "Settings",
        "MediaLibrarySettings",
        "ServerCapabilitiesResponse",
    ] {
        assert!(schemas.contains_key(name), "Missing OpenAPI schema: {name}");
    }
    assert_eq!(
        schemas["LoginForm"]["properties"]["password"]["type"],
        "string"
    );
    assert!(
        schemas["LoginForm"]["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("password"))
    );

    fn assert_schema_references(
        document: &serde_json::Value,
        value: &serde_json::Value,
    ) {
        match value {
            serde_json::Value::Object(object) => {
                if let Some(reference) = object.get("$ref").and_then(serde_json::Value::as_str) {
                    let pointer = reference
                        .strip_prefix('#')
                        .expect("Expected a local schema");
                    assert!(
                        document.pointer(pointer).is_some(),
                        "Unresolved schema: {reference}"
                    );
                }
                for value in object.values() {
                    assert_schema_references(document, value);
                }
            }
            serde_json::Value::Array(array) => {
                for value in array {
                    assert_schema_references(document, value);
                }
            }
            _ => {}
        }
    }
    assert_schema_references(&document, &document);
}

#[rocket::async_test]
async fn test_swagger_ui_route() {
    make_request(
        None,
        "get",
        "/swagger-ui/",
        None,
        None,
        Some(Status::SeeOther),
        Some(false),
    )
    .await;
}

#[rocket::async_test]
async fn test_rapidoc_route() {
    make_request(
        None,
        "get",
        "/rapidoc/",
        None,
        None,
        Some(Status::SeeOther),
        Some(false),
    )
    .await;
}

#[rocket::async_test]
async fn test_non_existent_route() {
    make_request(
        None,
        "get",
        "/non-existent",
        None,
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;
}

#[tokio::test]
async fn test_web_server_rocket_build() {
    // Test that we can build a rocket instance without errors
    let rocket = web::rocket_with_db_path(Some(":memory:".to_string()));
    assert!(
        rocket.ignite().await.is_ok(),
        "Rocket should ignite successfully"
    );
}

#[tokio::test]
async fn test_web_server_with_custom_db_path() {
    // Test web server with custom database path
    let custom_db_path = Some(":memory:".to_string());
    let rocket = web::rocket_with_db_path(custom_db_path);
    assert!(
        rocket.ignite().await.is_ok(),
        "Rocket with custom DB path should ignite successfully"
    );
}
