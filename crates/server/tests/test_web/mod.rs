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

pub async fn test_request(
    method: &str,
    path: &'static str,
    json: Option<Value>,
    expected_status: Status,
    client: Option<&Client>,
) -> TestResponse {
    let client = match client {
        Some(c) => c.to_owned(),
        None => {
            let rocket = web::rocket();
            &Client::tracked(rocket)
                .await
                .expect("Failed to launch web server")
        }
    };

    let request = match method.to_lowercase().as_str() {
        "get" => client.get(path),
        "post" => client.post(path),
        "put" => client.put(path),
        "delete" => client.delete(path),
        "patch" => client.patch(path),
        _ => panic!("Unsupported HTTP method: {}", method),
    };

    let request = if let Some(json_value) = json {
        request
            .header(ContentType::JSON)
            .body(json_value.to_string())
    } else {
        request
    };

    let response = request.dispatch().await;
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
    test_request("get", "/swagger-ui/", None, Status::SeeOther, None).await;
}

#[rocket::async_test]
async fn test_rapidoc_route() {
    test_request("get", "/rapidoc/", None, Status::SeeOther, None).await;
}

#[rocket::async_test]
async fn test_non_existent_route() {
    test_request("get", "/non-existent", None, Status::NotFound, None).await;
}
