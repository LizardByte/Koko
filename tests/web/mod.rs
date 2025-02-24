mod routes;

use koko::web;
use rocket::http::{ContentType, Header, Status};
use rocket::local::asynchronous::{Client, LocalResponse};
use serde_json::Value;

pub struct TestResponse {
    pub status: Status,
    pub body: String,
    pub headers: Vec<Header<'static>>,
}

pub async fn test_route(
    path: &'static str,
    expected_status: Status,
) -> TestResponse {
    let rocket = web::rocket();
    let client = Client::tracked(rocket)
        .await
        .expect("Failed to launch web server");

    let response = client.get(path).dispatch().await;
    create_test_response(response, expected_status).await
}

pub async fn test_post_json(
    client: &Client,
    path: &'static str,
    json: Value,
    expected_status: Status,
) -> TestResponse {
    let response = client
        .post(path)
        .header(ContentType::JSON)
        .body(json.to_string())
        .dispatch()
        .await;

    create_test_response(response, expected_status).await
}

async fn create_test_response(
    response: LocalResponse<'_>,
    expected_status: Status,
) -> TestResponse {
    assert_eq!(response.status(), expected_status);

    let status = response.status();
    let headers: Vec<Header<'static>> = response
        .headers()
        .iter()
        .map(|h| Header::new(h.name().to_string(), h.value().to_string()))
        .collect();
    let body = response.into_string().await.unwrap_or_default();

    TestResponse {
        status,
        body,
        headers,
    }
}

#[rocket::async_test]
async fn test_swagger_ui_route() {
    test_route("/swagger-ui/", Status::SeeOther).await;
}

#[rocket::async_test]
async fn test_rapidoc_route() {
    test_route("/rapidoc/", Status::SeeOther).await;
}

#[rocket::async_test]
async fn test_non_existent_route() {
    test_route("/non-existent", Status::NotFound).await;
}
