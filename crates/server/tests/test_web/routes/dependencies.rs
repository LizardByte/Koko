use crate::test_web::test_request;

use rocket::http::Status;
use rocket::serde::json::{serde_json, Value};

#[rocket::async_test]
async fn test_get_dependencies_route() {
    let response = test_request("get", "/dependencies", None, Status::Ok, None).await;

    // ensure response is a json list of dictionaries, and each dictionary has the keys name, version, and license
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
