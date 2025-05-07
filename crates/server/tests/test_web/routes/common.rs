use crate::test_web::test_request;

use rocket::http::Status;

#[rocket::async_test]
async fn test_root_route() {
    let response = test_request("get", "/", None, Status::Ok, None).await;

    assert_eq!(response.body, "Welcome to Koko!");
}
