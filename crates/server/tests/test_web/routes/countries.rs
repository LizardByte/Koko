//! All authenticated clients share the backend's offline country reference data.

use crate::test_utils::{
    create_admin_test_client,
    login_user,
    schema::users,
};
use diesel::prelude::*;
use koko::db::DbConn;
use rocket::http::{
    ContentType,
    Header,
    Status,
};
use serde_json::Value;
use std::collections::HashSet;

#[rocket::async_test]
async fn countries_require_authentication_and_are_available_to_ordinary_accounts() {
    let (client, admin) = create_admin_test_client(Some("country_reference_data")).await;
    assert_eq!(
        client.get("/api/v1/countries").dispatch().await.status(),
        Status::Unauthorized
    );
    let response = client
        .get("/api/v1/countries")
        .header(Header::new("Authorization", format!("Bearer {admin}")))
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::JSON));
    let countries: Vec<Value> = response.into_json().await.unwrap();
    assert_eq!(countries.len(), 249);
    let codes: HashSet<_> = countries
        .iter()
        .map(|country| {
            let code = country["code"].as_str().unwrap();
            assert!(code.len() == 2 && code.bytes().all(|c| c.is_ascii_lowercase()));
            assert!(!country["name"].as_str().unwrap().is_empty());
            code
        })
        .collect();
    assert_eq!(codes.len(), countries.len());
    assert_eq!(countries[0]["code"], "af");
    assert_eq!(countries[1]["code"], "ax");
    assert_eq!(countries[1]["name"], "Åland Islands");
    assert_eq!(countries[2]["code"], "al");
    for (code, name) in [
        ("ca", "Canada"),
        ("tr", "Türkiye"),
        ("us", "United States of America"),
    ] {
        assert!(
            countries
                .iter()
                .any(|country| country["code"] == code && country["name"] == name)
        );
    }
    // Ordinary accounts receive identical reference data even with no libraries or subscriptions.
    DbConn::get_one(client.rocket())
        .await
        .unwrap()
        .run(|conn| {
            diesel::update(users::table)
                .set(users::admin.eq(false))
                .execute(conn)
                .unwrap();
        })
        .await;
    let token = login_user(&client, "admin", "password123", Some(Status::Ok))
        .await
        .unwrap();
    let response = client
        .get("/api/v1/countries")
        .header(Header::new("Authorization", format!("Bearer {token}")))
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.into_json::<Vec<Value>>().await.unwrap(), countries);
}
