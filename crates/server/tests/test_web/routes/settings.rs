use rocket::http::Status;
use rocket::serde::json::{Value, json, serde_json};

use crate::test_utils::{create_test_client, make_request};
use koko::globals;

#[rocket::async_test]
async fn test_get_settings_route() {
    let client = create_test_client(Some("settings_route_get")).await;

    let response = make_request(
        Some(&client),
        "get",
        "/api/v1/settings",
        None,
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;

    let json: Value = serde_json::from_str(&response.body).unwrap();
    assert!(json.get("settings").is_some());
    assert!(json.get("settings_path").is_some());
}

#[rocket::async_test]
async fn test_add_and_remove_library_routes() {
    let client = create_test_client(Some("settings_route_add_remove")).await;

    let before_response = make_request(
        Some(&client),
        "get",
        "/api/v1/settings",
        None,
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;
    let before_json: Value = serde_json::from_str(&before_response.body).unwrap();
    let initial_library_count = before_json["settings"]["media"]["libraries"]
        .as_array()
        .unwrap()
        .len();

    let add_response = make_request(
        Some(&client),
        "post",
        "/api/v1/settings/libraries",
        Some(json!({
            "library": {
                "name": "Movies",
                "path": "C:/Media/Movies",
                "paths": ["C:/Media/Movies", "D:/Media/Movies"],
                "recursive": true,
                "kind": "movies",
                "metadata_providers": ["tmdb"]
            }
        })),
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;

    let add_json: Value = serde_json::from_str(&add_response.body).unwrap();
    let libraries = add_json["settings"]["media"]["libraries"]
        .as_array()
        .unwrap();
    assert_eq!(libraries.len(), initial_library_count + 1);
    assert_eq!(
        libraries[initial_library_count]["paths"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let remove_response = make_request(
        Some(&client),
        "delete",
        &format!("/api/v1/settings/libraries/{}", initial_library_count),
        None,
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;

    let remove_json: Value = serde_json::from_str(&remove_response.body).unwrap();
    let libraries_after_remove = remove_json["settings"]["media"]["libraries"]
        .as_array()
        .unwrap();
    assert_eq!(libraries_after_remove.len(), initial_library_count);
}

#[rocket::async_test]
async fn test_add_library_route_allows_duplicate_root_paths() {
    let client = create_test_client(Some("settings_route_add_duplicate")).await;

    make_request(
        Some(&client),
        "post",
        "/api/v1/settings/libraries",
        Some(json!({
            "library": {
                "name": "Movies",
                "path": "C:/Media/Movies",
                "paths": ["C:/Media/Movies"],
                "recursive": true,
                "kind": "movies",
                "metadata_providers": ["tmdb"]
            }
        })),
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;

    let duplicate = make_request(
        Some(&client),
        "post",
        "/api/v1/settings/libraries",
        Some(json!({
            "library": {
                "name": "Duplicate Movies",
                "path": "C:/Media/Movies",
                "paths": ["C:/Media/Movies"],
                "recursive": true,
                "kind": "movies",
                "metadata_providers": ["tmdb", "tvdb"]
            }
        })),
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;

    let duplicate_json: Value = serde_json::from_str(&duplicate.body).unwrap();
    let libraries = duplicate_json["settings"]["media"]["libraries"]
        .as_array()
        .unwrap();
    assert_eq!(libraries.len(), 2);
    assert_eq!(libraries[0]["path"].as_str().unwrap(), "C:/Media/Movies");
    assert_eq!(libraries[1]["path"].as_str().unwrap(), "C:/Media/Movies");
}

#[rocket::async_test]
async fn test_update_settings_with_empty_libraries_does_not_delete_catalog() {
    let client = create_test_client(Some("settings_route_update_preserves_libraries")).await;

    make_request(
        Some(&client),
        "post",
        "/api/v1/settings/libraries",
        Some(json!({
            "library": {
                "name": "Movies",
                "path": "C:/Media/Movies",
                "paths": ["C:/Media/Movies"],
                "recursive": true,
                "kind": "movies",
                "metadata_providers": ["tmdb"]
            }
        })),
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;

    let settings_response = make_request(
        Some(&client),
        "get",
        "/api/v1/settings",
        None,
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;
    let mut settings_json: Value = serde_json::from_str(&settings_response.body).unwrap();
    settings_json["settings"]["media"]["libraries"] = json!([]);

    let update_response = make_request(
        Some(&client),
        "put",
        "/api/v1/settings",
        Some(settings_json["settings"].clone()),
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;
    let update_json: Value = serde_json::from_str(&update_response.body).unwrap();
    assert_eq!(
        update_json["settings"]["media"]["libraries"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let after_response = make_request(
        Some(&client),
        "get",
        "/api/v1/settings",
        None,
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;
    let after_json: Value = serde_json::from_str(&after_response.body).unwrap();
    assert_eq!(
        after_json["settings"]["media"]["libraries"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[rocket::async_test]
async fn test_update_settings_with_partial_libraries_preserves_omitted_catalog_rows() {
    let client =
        create_test_client(Some("settings_route_update_preserves_omitted_libraries")).await;

    for (name, kind) in [
        ("Movies", "movies"),
        ("Shows", "shows"),
    ] {
        make_request(
            Some(&client),
            "post",
            "/api/v1/settings/libraries",
            Some(json!({
                "library": {
                    "name": name,
                    "path": format!("C:/Media/{name}"),
                    "paths": [format!("C:/Media/{name}")],
                    "recursive": true,
                    "kind": kind,
                    "metadata_providers": ["tmdb"]
                }
            })),
            None,
            Some(Status::Ok),
            Some(false),
        )
        .await;
    }

    let settings_response = make_request(
        Some(&client),
        "get",
        "/api/v1/settings",
        None,
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;
    let mut settings_json: Value = serde_json::from_str(&settings_response.body).unwrap();
    let first_library = settings_json["settings"]["media"]["libraries"][0].clone();
    settings_json["settings"]["media"]["libraries"] = json!([first_library]);

    let update_response = make_request(
        Some(&client),
        "put",
        "/api/v1/settings",
        Some(settings_json["settings"].clone()),
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;
    let update_json: Value = serde_json::from_str(&update_response.body).unwrap();
    assert_eq!(
        update_json["settings"]["media"]["libraries"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let after_response = make_request(
        Some(&client),
        "get",
        "/api/v1/settings",
        None,
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;
    let after_json: Value = serde_json::from_str(&after_response.body).unwrap();
    assert_eq!(
        after_json["settings"]["media"]["libraries"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[rocket::async_test]
async fn test_remove_missing_library_route() {
    let client = create_test_client(Some("settings_route_remove_missing")).await;

    make_request(
        Some(&client),
        "delete",
        "/api/v1/settings/libraries/999",
        None,
        None,
        Some(Status::NotFound),
        Some(false),
    )
    .await;
}

#[rocket::async_test]
async fn test_get_logs_route_filters_and_normalizes_paths() {
    let client = create_test_client(Some("settings_route_logs")).await;
    let unique = "settings_route_logs_unique_marker";

    std::fs::write(
        &globals::APP_PATHS.log_path,
        format!(
            "2026-04-22T11:05:02.631-04:00 [INFO] [rocket::server] [C:\\Users\\ReenigneArcher\\.cargo\\registry\\src\\index.crates.io-1949cf8c6b5b557f\\rocket-0.5.1\\src\\server.rs:134] Response succeeded. {unique}\n2026-04-22T11:10:02.636-04:00 [WARN] [koko] [crates\\server\\src\\lib.rs:32] Web server thread completed {unique}\n"
        ),
    )
    .unwrap();

    let filtered_response = make_request(
        Some(&client),
        "get",
        &format!("/api/v1/settings/logs?search={unique}&since=2026-04-22T11%3A06&limit=10"),
        None,
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;
    let filtered_json: Value = serde_json::from_str(&filtered_response.body).unwrap();
    let filtered_entries = filtered_json["entries"].as_array().unwrap();
    assert_eq!(filtered_entries.len(), 1);
    assert_eq!(filtered_entries[0]["level"].as_str().unwrap(), "WARN");
    assert_eq!(
        filtered_entries[0]["source_file_path"].as_str().unwrap(),
        "crates/server/src/lib.rs"
    );
    assert!(filtered_json["log_path"].as_str().unwrap().contains('/'));

    let normalized_response = make_request(
        Some(&client),
        "get",
        &format!("/api/v1/settings/logs?search={unique}&module=rocket::server&limit=10"),
        None,
        None,
        Some(Status::Ok),
        Some(false),
    )
    .await;
    let normalized_json: Value = serde_json::from_str(&normalized_response.body).unwrap();
    let normalized_entries = normalized_json["entries"].as_array().unwrap();
    assert_eq!(normalized_entries.len(), 1);
    assert_eq!(
        normalized_entries[0]["source_file_path"].as_str().unwrap(),
        "rocket-0.5.1/src/server.rs"
    );
}
