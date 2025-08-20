// lib imports
use rocket::http::Status;

// test imports
use crate::test_utils::{create_test_client, make_request};

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
        Some(false),
    )
    .await;
    assert_eq!(response.body, "Welcome to Koko!");
}
