// mod imports
mod routes;
mod test_auth_routes;

// lib imports
use rocket::http::Status;

// local imports
use koko::web;

// test imports
use crate::test_utils::make_request;

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
        Some(Status::NotFound),
        Some(false),
    )
    .await;
}

#[tokio::test]
async fn test_web_server_rocket_build() {
    // Test that we can build a rocket instance without errors
    let rocket = web::rocket();
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
