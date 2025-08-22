// lib imports
use rocket::http::Status;
use rocket::serde::json::{Value, serde_json};

// test imports
use crate::test_utils::{TestResponse, make_request};

#[rocket::async_test]
async fn test_get_dependencies_route() {
    let response: TestResponse = make_request(
        None,
        "get",
        "/dependencies",
        None,
        None,
        Some(Status::Ok),
        Some(true),
    )
    .await;

    // ensure the response is a JSON list of dictionaries, and each dictionary has the key name,
    // version, and license
    let body = response.body;
    let json: Value = serde_json::from_str(&body).unwrap();

    assert!(json.is_array(), "Response is not a JSON array");

    for item in json.as_array().unwrap() {
        assert!(item.is_object(), "Array item is not a JSON object");
        let obj = item.as_object().unwrap();
        assert!(
            obj.contains_key("name"),
            "Object does not contain key 'name'"
        );
        assert!(
            obj.contains_key("version"),
            "Object does not contain key 'version'"
        );
        assert!(
            obj.contains_key("license"),
            "Object does not contain key 'license'"
        );
    }
}
