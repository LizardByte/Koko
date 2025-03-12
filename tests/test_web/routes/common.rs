use crate::test_web::test_route;

use rocket::http::Status;

#[rocket::async_test]
async fn test_root_route() {
    let response = test_route("/", Status::Ok).await;

    assert_eq!(response.body, "Welcome to Koko!");
}
