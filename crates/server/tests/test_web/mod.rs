// mod imports
mod routes;
mod test_auth_routes;

// lib imports
use rocket::http::Status;

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
