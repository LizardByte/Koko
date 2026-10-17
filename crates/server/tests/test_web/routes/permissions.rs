use diesel::connection::SimpleConnection;
use koko::db::DbConn;
use rocket::http::{
    ContentType,
    Header,
    Method,
    Status,
};
use rocket::local::asynchronous::Client;
use serde_json::{
    Value,
    json,
};

use crate::test_utils::{
    create_admin_test_client,
    create_test_client,
    login_user,
};

const ADMIN_ROUTES: &[(Method, &str)] = &[
    (Method::Get, "/api/v1/settings"),
    (Method::Put, "/api/v1/settings"),
    (Method::Get, "/api/v1/settings/logs"),
    (Method::Post, "/api/v1/settings/metadata-cache/clear"),
    (Method::Post, "/api/v1/settings/libraries"),
    (Method::Delete, "/api/v1/settings/libraries/0"),
    (Method::Post, "/api/v1/scheduled-tasks/metadata_refresh/run"),
    (Method::Get, "/api/v1/system/activities"),
    (Method::Get, "/api/v1/users"),
    (Method::Put, "/api/v1/users/1"),
    (Method::Get, "/api/v1/libraries/1/files"),
    (Method::Post, "/api/v1/libraries/1/scan"),
    (Method::Delete, "/api/v1/libraries/1/missing"),
    (Method::Post, "/api/v1/libraries/1/metadata/refresh"),
    (Method::Get, "/api/v1/items/1/metadata/search?query=film"),
    (Method::Post, "/api/v1/items/1/metadata/link"),
    (Method::Delete, "/api/v1/items/1/metadata"),
    (Method::Post, "/api/v1/items/1/metadata/refresh"),
];

const USER_ROUTES: &[(Method, &str)] = &[
    (Method::Get, "/api/v1/system/capabilities"),
    (Method::Get, "/api/v1/metadata/providers"),
    (Method::Get, "/api/v1/metadata/locales"),
    (Method::Get, "/api/v1/libraries"),
    (Method::Get, "/api/v1/home"),
    (Method::Get, "/api/v1/items"),
    (Method::Get, "/api/v1/search?query=film"),
    (Method::Get, "/api/v1/items/1"),
    (Method::Get, "/api/v1/items/1/metadata"),
    (Method::Get, "/api/v1/items/1/playback"),
    (Method::Get, "/api/v1/items/1/artwork"),
    (Method::Get, "/api/v1/items/1/stream"),
    (Method::Get, "/api/v1/items/1/theme"),
    (Method::Get, "/api/v1/items/1/subtitles/0"),
    (Method::Post, "/api/v1/items/1/progress"),
    (Method::Post, "/api/v1/sessions"),
    (Method::Delete, "/api/v1/sessions/example"),
    (Method::Get, "/api/v1/sessions/example/stream"),
    (Method::Get, "/api/v1/people/1"),
    (Method::Get, "/api/v1/people/1/image"),
    (Method::Get, "/api/v1/user-profile-images/example.png"),
];

async fn response(
    client: &Client,
    method: Method,
    path: &str,
    token: Option<&str>,
    body: Value,
    expected: Status,
) -> String {
    let mut request = client
        .req(method, path)
        .header(ContentType::JSON)
        .body(body.to_string());
    if let Some(token) = token {
        request = request.header(Header::new("Authorization", format!("Bearer {token}")));
    }
    let response = request.dispatch().await;
    assert_eq!(response.status(), expected, "{method} {path}");
    response.into_string().await.unwrap_or_default()
}

fn session_request(item_id: i32) -> Value {
    json!({
        "item_id": item_id,
        "client_profile": {
            "client_type": "web",
            "client_name": "test",
            "supported_containers": ["mp4"],
            "supported_video_codecs": ["h264"],
            "supported_audio_codecs": ["aac"],
            "supported_subtitle_formats": [],
            "max_video_width": 0,
            "max_video_height": 0,
            "max_bitrate_kbps": 0,
            "supports_adaptive_streaming": false,
            "prefer_hls": false
        }
    })
}

async fn fixture() -> (Client, String, String) {
    let (client, admin) = create_admin_test_client(Some("permissions")).await;
    DbConn::get_one(client.rocket())
        .await
        .unwrap()
        .run(|conn| {
            conn.batch_execute(
                r#"
            INSERT INTO users (id, username, password, admin)
                SELECT 2, 'viewer', password, 0 FROM users WHERE id = 1;
            INSERT INTO users (id, username, password, admin)
                SELECT 3, 'other', password, 0 FROM users WHERE id = 1;
            DELETE FROM media_libraries;
            INSERT INTO media_libraries (id, name, path, kind, allowed_user_ids_json) VALUES
                (1, 'Open library', '/open', 'movies', '[]'),
                (2, 'Secret library', '/secret', 'movies', '[3]'),
                (3, 'Granted library', '/granted', 'movies', '[2]');
            INSERT INTO media_items
                (id, library_id, identity_key, item_type, display_title, playable) VALUES
                (1, 1, 'visible', 'movie', 'Visible film', 1),
                (2, 2, 'secret', 'movie', 'Secret film', 1),
                (3, 3, 'granted', 'movie', 'Granted film', 1);
            INSERT INTO item_metadata_links
                (id, media_item_id, provider_id, external_id, title, match_state) VALUES
                (1, 1, 'tmdb', '1', 'Visible film', 'matched'),
                (2, 2, 'tmdb', '2', 'Secret film', 'matched');
            INSERT INTO metadata_people (id, provider_id, external_id, name) VALUES
                (1, 'tmdb', '1', 'Shared person'), (2, 'tmdb', '2', 'Secret person');
            INSERT INTO metadata_person_credits (metadata_link_id, person_id, role) VALUES
                (1, 1, 'actor'), (2, 1, 'actor'), (2, 2, 'actor');
            INSERT INTO metadata_collections
                (id, provider_id, external_id, source_provider_id, source_external_id,
                 relation_kind, locale_key, name) VALUES
                (1, 'tmdb', '1', 'tmdb', '1', 'collection', 'en-US', 'Shared collection'),
                (2, 'tmdb', '2', 'tmdb', '2', 'collection', 'en-US', 'Secret collection');
            INSERT INTO metadata_collection_items
                (collection_id, media_item_id, metadata_link_id) VALUES
                (1, 1, 1), (1, 2, 2), (2, 2, 2);
        "#,
            )
            .unwrap();
        })
        .await;
    let viewer = login_user(&client, "viewer", "password123", Some(Status::Ok))
        .await
        .unwrap();
    (client, admin, viewer)
}

#[rocket::async_test]
async fn administrator_unlink_clears_the_tree_before_returning_without_provider_credentials() {
    let (client, admin, _) = fixture().await;
    let db = DbConn::get_one(client.rocket()).await.unwrap();
    db.run(|conn| {
        conn.batch_execute(
            "UPDATE media_items SET item_type='show' WHERE id=1;
            UPDATE item_metadata_links SET title='Wrong show',overview='Old metadata',
                last_refreshed_at=1 WHERE media_item_id=1;
            INSERT INTO media_items
                (id,library_id,parent_id,identity_key,item_type,display_title) VALUES
            (11,1,1,'season','season','Season 1'), (12,1,11,'episode','episode','Episode 1');
            INSERT INTO item_metadata_links
                (media_item_id,provider_id,external_id,title,last_refreshed_at) VALUES
            (11,'tmdb','old-season','Wrong season',1),
            (12,'tmdb','old-episode','Wrong episode',1);",
        )
        .unwrap();
    })
    .await;
    // Release the pooled connection before the route acquires its own.
    drop(db);
    response(
        &client,
        Method::Delete,
        "/api/v1/items/1/metadata",
        Some(&admin),
        json!({}),
        Status::NoContent,
    )
    .await;
    DbConn::get_one(client.rocket())
        .await
        .unwrap()
        .run(|conn| {
            for id in [1, 11, 12] {
                assert!(
                    koko::metadata::get_item_metadata_summaries(conn, id)
                        .unwrap()
                        .is_empty()
                );
                let item = koko::media::get_media_item_summary(conn, id)
                    .unwrap()
                    .unwrap();
                assert!(!item.has_metadata);
                assert!(item.overview.is_none());
            }
            assert_eq!(
                koko::media::get_media_item_summary(conn, 1)
                    .unwrap()
                    .unwrap()
                    .display_title,
                "Visible film"
            );
            assert_eq!(
                koko::metadata::get_item_metadata_summaries(conn, 2)
                    .unwrap()
                    .len(),
                1
            );
        })
        .await;
    response(
        &client,
        Method::Delete,
        "/api/v1/items/1/metadata",
        Some(&admin),
        json!({}),
        Status::NoContent,
    )
    .await;
    response(
        &client,
        Method::Delete,
        "/api/v1/items/11/metadata",
        Some(&admin),
        json!({}),
        Status::BadRequest,
    )
    .await;
    response(
        &client,
        Method::Delete,
        "/api/v1/items/999/metadata",
        Some(&admin),
        json!({}),
        Status::NotFound,
    )
    .await;
}

#[rocket::async_test]
async fn anonymous_requests_cannot_access_protected_routes() {
    let client = create_test_client(Some("anonymous_permissions")).await;
    for &(method, path) in ADMIN_ROUTES.iter().chain(USER_ROUTES) {
        response(&client, method, path, None, json!({}), Status::Unauthorized).await;
    }
    assert_eq!(
        client.get("/api/v1/bootstrap").dispatch().await.status(),
        Status::Ok
    );
}

#[rocket::async_test]
async fn ordinary_users_cannot_manage_server_even_for_accessible_libraries() {
    let (client, _, viewer) = fixture().await;
    for &(method, path) in ADMIN_ROUTES {
        response(
            &client,
            method,
            path,
            Some(&viewer),
            json!({}),
            Status::Forbidden,
        )
        .await;
    }
    response(
        &client,
        Method::Post,
        "/create_user",
        Some(&viewer),
        json!({
            "username": "intruder", "password": "password123", "admin": true
        }),
        Status::Unauthorized,
    )
    .await;
}

#[rocket::async_test]
async fn library_grants_filter_items_collections_people_and_counts() {
    let (client, admin, viewer) = fixture().await;
    for path in [
        "/api/v1/libraries",
        "/api/v1/items",
        "/api/v1/home",
        "/api/v1/search?query=film",
        "/api/v1/people/1",
    ] {
        let text = response(
            &client,
            Method::Get,
            path,
            Some(&viewer),
            json!({}),
            Status::Ok,
        )
        .await;
        assert!(
            !text.contains("Secret"),
            "Leaked hidden content through {path}: {text}"
        );
    }
    let hidden = response(
        &client,
        Method::Get,
        "/api/v1/search?query=Secret",
        Some(&viewer),
        json!({}),
        Status::Ok,
    )
    .await;
    assert_eq!(serde_json::from_str::<Value>(&hidden).unwrap(), json!([]));
    let shared = response(
        &client,
        Method::Get,
        "/api/v1/search?query=Shared",
        Some(&viewer),
        json!({}),
        Status::Ok,
    )
    .await;
    assert!(shared.contains("Shared person"));
    assert!(shared.contains("Shared collection"));
    assert!(!shared.contains("Secret"));
    let home = response(
        &client,
        Method::Get,
        "/api/v1/home",
        Some(&viewer),
        json!({}),
        Status::Ok,
    )
    .await;
    let home: Value = serde_json::from_str(&home).unwrap();
    assert_eq!(home["collections"].as_array().unwrap().len(), 1);
    assert_eq!(home["collections"][0]["item_ids"], json!([1]));
    assert_eq!(home["collections"][0]["item_count"], 1);
    for (token, expected_count) in [(&viewer, 2), (&admin, 3)] {
        let capabilities = response(
            &client,
            Method::Get,
            "/api/v1/system/capabilities",
            Some(token),
            json!({}),
            Status::Ok,
        )
        .await;
        assert_eq!(
            serde_json::from_str::<Value>(&capabilities).unwrap()["libraries_configured"],
            expected_count
        );
    }
    for path in [
        "/api/v1/items/1",
        "/api/v1/items/3",
        "/api/v1/home?library_id=3",
        "/api/v1/items?library_id=3",
    ] {
        response(
            &client,
            Method::Get,
            path,
            Some(&viewer),
            json!({}),
            Status::Ok,
        )
        .await;
    }
    for path in [
        "/api/v1/home?library_id=2",
        "/api/v1/items?library_id=2",
        "/api/v1/items/2",
        "/api/v1/items/2/metadata",
        "/api/v1/items/2/playback",
        "/api/v1/items/2/artwork",
        "/api/v1/items/2/stream",
        "/api/v1/items/2/theme",
        "/api/v1/items/2/subtitles/0",
        "/api/v1/people/2",
        "/api/v1/people/2/image",
    ] {
        response(
            &client,
            Method::Get,
            path,
            Some(&viewer),
            json!({}),
            Status::NotFound,
        )
        .await;
    }
    response(
        &client,
        Method::Post,
        "/api/v1/items/2/progress",
        Some(&viewer),
        json!({"position_ms": 10, "completed": false}),
        Status::NotFound,
    )
    .await;
    response(
        &client,
        Method::Post,
        "/api/v1/sessions",
        Some(&viewer),
        session_request(2),
        Status::NotFound,
    )
    .await;
    response(
        &client,
        Method::Get,
        "/api/v1/items/2",
        Some(&admin),
        json!({}),
        Status::Ok,
    )
    .await;
}

#[rocket::async_test]
async fn playback_sessions_and_live_account_changes_are_enforced() {
    let (client, admin, viewer) = fixture().await;
    let session = response(
        &client,
        Method::Post,
        "/api/v1/sessions",
        Some(&viewer),
        session_request(1),
        Status::Ok,
    )
    .await;
    let session: Value = serde_json::from_str(&session).unwrap();
    let id = session["session_id"].as_str().unwrap();
    response(
        &client,
        Method::Get,
        &format!("/api/v1/sessions/{id}/stream"),
        Some(&admin),
        json!({}),
        Status::NotFound,
    )
    .await;
    response(
        &client,
        Method::Delete,
        &format!("/api/v1/sessions/{id}"),
        Some(&admin),
        json!({}),
        Status::NotFound,
    )
    .await;
    let db = DbConn::get_one(client.rocket()).await.unwrap();
    db.run(|conn| {
        conn.batch_execute("UPDATE media_libraries SET allowed_user_ids_json = '[3]' WHERE id = 1")
            .unwrap()
    })
    .await;
    response(
        &client,
        Method::Get,
        &format!("/api/v1/sessions/{id}/stream"),
        Some(&viewer),
        json!({}),
        Status::NotFound,
    )
    .await;
    response(
        &client,
        Method::Delete,
        &format!("/api/v1/sessions/{id}"),
        Some(&viewer),
        json!({}),
        Status::NoContent,
    )
    .await;
    db.run(|conn| {
        conn.batch_execute(
            "UPDATE users SET admin = 0 WHERE id = 1; DELETE FROM users WHERE id = 2;",
        )
        .unwrap()
    })
    .await;
    response(
        &client,
        Method::Get,
        "/api/v1/settings",
        Some(&admin),
        json!({}),
        Status::Forbidden,
    )
    .await;
    response(
        &client,
        Method::Get,
        "/api/v1/items",
        Some(&viewer),
        json!({}),
        Status::Unauthorized,
    )
    .await;
}

#[rocket::async_test]
async fn media_cookie_cannot_authorize_api_or_management_and_logout_clears_it() {
    let (client, _, viewer) = fixture().await;
    assert!(
        client
            .cookies()
            .get("koko_media_auth")
            .unwrap()
            .http_only()
            .unwrap()
    );
    // The viewer's login cookie authenticates native media elements, then library grants still
    // apply.
    response(
        &client,
        Method::Get,
        "/api/v1/items/2/artwork",
        None,
        json!({}),
        Status::NotFound,
    )
    .await;
    response(
        &client,
        Method::Get,
        "/api/v1/items",
        None,
        json!({}),
        Status::Unauthorized,
    )
    .await;
    response(
        &client,
        Method::Get,
        "/api/v1/settings",
        None,
        json!({}),
        Status::Unauthorized,
    )
    .await;
    response(
        &client,
        Method::Get,
        "/api/v1/items/1/artwork",
        Some("invalid"),
        json!({}),
        Status::Unauthorized,
    )
    .await;
    let api_response = client
        .get("/api/v1/items")
        .header(Header::new("Authorization", format!("Bearer {viewer}")))
        .dispatch()
        .await;
    assert_eq!(
        api_response.headers().get_one("Cache-Control"),
        Some("private, no-store")
    );
    client.get("/logout").dispatch().await;
    response(
        &client,
        Method::Get,
        "/api/v1/items/1/artwork",
        None,
        json!({}),
        Status::Unauthorized,
    )
    .await;
}

#[rocket::async_test]
async fn malformed_library_permissions_fail_closed() {
    let (client, admin, viewer) = fixture().await;
    DbConn::get_one(client.rocket())
        .await
        .unwrap()
        .run(|conn| {
            conn.batch_execute(
                "UPDATE media_libraries SET allowed_user_ids_json = 'invalid' WHERE id = 2",
            )
            .unwrap();
        })
        .await;
    response(
        &client,
        Method::Get,
        "/api/v1/items/2",
        Some(&viewer),
        json!({}),
        Status::NotFound,
    )
    .await;
    response(
        &client,
        Method::Get,
        "/api/v1/items/2",
        Some(&admin),
        json!({}),
        Status::Ok,
    )
    .await;
}

#[rocket::async_test]
async fn concurrent_first_user_requests_cannot_create_two_administrators() {
    let client = create_test_client(Some("bootstrap_race")).await;
    let first = client
        .post("/create_user")
        .header(ContentType::JSON)
        .body(json!({"username": "first", "password": "password123", "admin": true}).to_string());
    let second = client
        .post("/create_user")
        .header(ContentType::JSON)
        .body(json!({"username": "second", "password": "password123", "admin": true}).to_string());
    let (first, second) = rocket::tokio::join!(first.dispatch(), second.dispatch());
    assert!(
        (first.status() == Status::Ok && second.status() == Status::Unauthorized)
            || (first.status() == Status::Unauthorized && second.status() == Status::Ok)
    );
}

#[rocket::async_test]
async fn authorized_native_media_and_range_requests_remain_usable() {
    use diesel::{
        RunQueryDsl,
        sql_query,
        sql_types::Text,
    };
    use std::fs;
    let (client, _, viewer) = fixture().await;
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::current_dir()
        .unwrap()
        .join("test_data")
        .join(format!("media_permissions_{unique}"));
    fs::create_dir_all(&root).unwrap();
    for (name, bytes) in [
        ("movie.mp4", "video"),
        ("poster.jpg", "poster"),
        ("theme.mp3", "theme"),
        ("movie.vtt", "subtitles"),
    ] {
        fs::write(root.join(name), bytes).unwrap();
    }
    let media_root = root.to_string_lossy().into_owned();
    DbConn::get_one(client.rocket())
        .await
        .unwrap()
        .run(move |conn| {
            sql_query("UPDATE media_libraries SET path = ?, paths_json = ? WHERE id = 1")
                .bind::<Text, _>(&media_root)
                .bind::<Text, _>(serde_json::to_string(&vec![&media_root]).unwrap())
                .execute(conn)
                .unwrap();
            sql_query(
                "INSERT INTO media_files
            (id, path, file_size, media_kind, container, video_codec, audio_codec)
            VALUES (1, ?, 5, 'video', 'mp4', 'h264', 'aac')",
            )
            .bind::<Text, _>(format!("{media_root}/movie.mp4"))
            .execute(conn)
            .unwrap();
            sql_query(
                "INSERT INTO media_file_libraries
            (media_file_id, library_id, source_root_path, relative_path, media_item_id)
            VALUES (1, 1, ?, 'movie.mp4', 1)",
            )
            .bind::<Text, _>(&media_root)
            .execute(conn)
            .unwrap();
            conn.batch_execute(
                "UPDATE media_items SET relative_path = 'movie.mp4',
            media_kind = 'video' WHERE id = 1",
            )
            .unwrap();
        })
        .await;
    for (path, expected) in [
        ("artwork", "poster"),
        ("theme", "theme"),
        ("subtitles/0", "subtitles"),
        ("stream", "video"),
    ] {
        let body = response(
            &client,
            Method::Get,
            &format!("/api/v1/items/1/{path}"),
            None,
            json!({}),
            Status::Ok,
        )
        .await;
        assert_eq!(body, expected);
    }
    let range = client
        .get("/api/v1/items/1/stream")
        .header(Header::new("Range", "bytes=1-3"))
        .dispatch()
        .await;
    assert_eq!(range.status(), Status::PartialContent);
    assert_eq!(range.into_string().await.unwrap(), "ide");
    let session = response(
        &client,
        Method::Post,
        "/api/v1/sessions",
        Some(&viewer),
        session_request(1),
        Status::Ok,
    )
    .await;
    let session: Value = serde_json::from_str(&session).unwrap();
    let id = session["session_id"].as_str().unwrap();
    let bytes = response(
        &client,
        Method::Get,
        &format!("/api/v1/sessions/{id}/stream"),
        None,
        json!({}),
        Status::Ok,
    )
    .await;
    assert_eq!(bytes, "video");
    response(
        &client,
        Method::Delete,
        &format!("/api/v1/sessions/{id}"),
        Some(&viewer),
        json!({}),
        Status::NoContent,
    )
    .await;
    client.get("/logout").dispatch().await;
    response(
        &client,
        Method::Get,
        "/api/v1/items/1/stream",
        None,
        json!({}),
        Status::Unauthorized,
    )
    .await;
    // Previously saved bearer tokens establish the native-media cookie at bootstrap too.
    response(
        &client,
        Method::Get,
        "/api/v1/bootstrap",
        Some(&viewer),
        json!({}),
        Status::Ok,
    )
    .await;
    response(
        &client,
        Method::Get,
        "/api/v1/items/1/stream",
        None,
        json!({}),
        Status::Ok,
    )
    .await;
    fs::remove_dir_all(root).unwrap();
}
