//! Podcast routes must enforce subscriptions even when accounts share the managed library.
use crate::test_utils::{
    create_admin_test_client,
    login_user,
    schema::{
        media_items as items,
        media_libraries as libraries,
        podcast_feed_aliases as aliases,
        podcast_feeds as feeds,
        podcast_items as metadata,
        podcast_subscriptions as subscriptions,
        users,
    },
};
use diesel::prelude::*;
use koko::db::DbConn;
use rocket::http::{
    ContentType,
    Cookie,
    Header,
    Status,
};
use serde_json::{
    Value,
    json,
};

#[rocket::async_test]
async fn podcast_routes_require_authentication_and_subscriptions_for_all_item_endpoints() {
    let (client, admin) = create_admin_test_client(Some("podcast_permissions")).await;
    DbConn::get_one(client.rocket())
        .await
        .unwrap()
        .run(seed)
        .await;
    let cached =
        std::env::temp_dir().join(format!("koko-podcast-route-{}.mp3", rand::random::<u64>()));
    std::fs::write(&cached, b"ID3!saved-episode").unwrap();
    let saved = cached.clone();
    DbConn::get_one(client.rocket())
        .await
        .unwrap()
        .run(move |conn| {
            let value = json!({
                "podcast_id": 1, "categories": [], "transcripts": [], "people": [],
                "local_download": {
                "state": "ready", "path": saved, "size_bytes": 17,
                "attempted_at": 1, "error": null
            }});
            diesel::update(metadata::table.find(2))
                .set(metadata::metadata_json.eq(value.to_string()))
                .execute(conn)
                .unwrap();
        })
        .await;
    let listener = login_user(&client, "listener", "listener-password", Some(Status::Ok))
        .await
        .unwrap();
    for path in [
        "/api/v1/podcasts",
        "/api/v1/podcasts/discover",
        "/api/v1/podcasts/home",
        "/api/v1/podcasts/settings",
        "/api/v1/podcasts/seasons/4/episodes",
        "/api/v1/podcasts/1/episodes",
        "/api/v1/podcasts/episodes/2/stream",
    ] {
        assert_eq!(
            client
                .get(path)
                .header(Header::new("Authorization", "Bearer invalid"))
                .dispatch()
                .await
                .status(),
            Status::Unauthorized,
            "{path}"
        );
    }
    // Admin has a subscription in this library but must still be denied another user's show.
    for path in [
        "/api/v1/items/1",
        "/api/v1/podcasts/seasons/4/episodes",
        "/api/v1/podcasts/1/episodes",
        "/api/v1/items/2",
        "/api/v1/items/2/playback",
        "/api/v1/items/2/metadata",
        "/api/v1/items/2/files",
        "/api/v1/items/2/credits",
        "/api/v1/items/2/similar",
        "/api/v1/items/2/artwork",
        "/api/v1/items/2/stream",
        "/api/v1/podcasts/episodes/2/stream",
    ] {
        assert_eq!(
            client
                .get(path)
                .header(Header::new("Authorization", format!("Bearer {admin}")))
                .dispatch()
                .await
                .status(),
            Status::NotFound,
            "{path}"
        );
    }
    assert_eq!(
        client
            .post("/api/v1/items/2/progress")
            .header(Header::new("Authorization", format!("Bearer {admin}")))
            .header(ContentType::JSON)
            .body(r#"{"position_ms":1000,"duration_ms":5000,"completed":false}"#)
            .dispatch()
            .await
            .status(),
        Status::NotFound
    );
    let response = client
        .get("/api/v1/podcasts")
        .header(Header::new("Authorization", format!("Bearer {listener}")))
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let rows: Vec<Value> = response.into_json().await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], 1);
    assert_eq!(
        client
            .get("/api/v1/items/2")
            .header(Header::new("Authorization", format!("Bearer {listener}")))
            .dispatch()
            .await
            .status(),
        Status::Ok
    );
    // Native audio elements authenticate with the media cookie, including after a session redirect.
    // A cached copy plays even though the original enclosure is unreachable.
    for (token, expected) in [
        (&admin, Status::NotFound),
        (&listener, Status::Ok),
    ] {
        assert_eq!(
            client
                .get("/api/v1/podcasts/episodes/2/stream")
                .cookie(Cookie::new("koko_media_auth", token.clone()))
                .dispatch()
                .await
                .status(),
            expected
        );
    }
    assert_eq!(
        client
            .get("/api/v1/podcasts")
            .cookie(Cookie::new("koko_media_auth", listener.clone()))
            .dispatch()
            .await
            .status(),
        Status::Unauthorized
    );
    for _ in 0..2 {
        let response = client
            .get("/api/v1/podcasts/episodes/2/stream")
            .header(Header::new("Authorization", format!("Bearer {listener}")))
            .header(Header::new("Range", "bytes=4-8"))
            .dispatch()
            .await;
        assert_eq!(response.status(), Status::PartialContent);
        assert_eq!(
            response.headers().get_one("Content-Range"),
            Some("bytes 4-8/17")
        );
        assert_eq!(response.into_bytes().await.unwrap(), b"saved");
    }
    for (token, expected) in [
        (&admin, Status::NotFound),
        (&listener, Status::NoContent),
    ] {
        let response = client
            .put("/api/v1/podcasts/1/preferences")
            .header(Header::new("Authorization", format!("Bearer {token}")))
            .header(ContentType::JSON)
            .body(r#"{"playback_order":"newest_first"}"#)
            .dispatch()
            .await;
        assert_eq!(response.status(), expected);
    }
    let response = client
        .get("/api/v1/podcasts/1/episodes")
        .header(Header::new("Authorization", format!("Bearer {listener}")))
        .dispatch()
        .await
        .into_json::<Value>()
        .await
        .unwrap();
    assert_eq!(response[0]["podcast"]["playback_order"], "newest_first");
    assert!(!response.to_string().contains("local_download"));
    assert!(!response.to_string().contains("koko-podcast-route"));
    // Adding an existing feed subscribes the second user without downloading or duplicating it.
    let response = client
        .post("/api/v1/podcasts")
        .header(Header::new("Authorization", format!("Bearer {admin}")))
        .header(ContentType::JSON)
        .body(r#"{"feed_url":"https://example.com/private"}"#)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.into_json::<Value>().await.unwrap()["item_id"], 1);
    assert_eq!(
        client
            .get("/api/v1/items/2/playback")
            .header(Header::new("Authorization", format!("Bearer {admin}")))
            .dispatch()
            .await
            .status(),
        Status::Ok
    );
    let rows = client
        .get("/api/v1/podcasts/1/episodes")
        .header(Header::new("Authorization", format!("Bearer {admin}")))
        .dispatch()
        .await
        .into_json::<Value>()
        .await
        .unwrap();
    assert_eq!(rows[0]["podcast"]["playback_order"], "oldest_first");
    let hidden = client
        .put("/api/v1/podcasts/settings")
        .header(Header::new("Authorization", format!("Bearer {listener}")))
        .header(ContentType::JSON)
        .body(r#"{"show_on_home":false,"show_in_sidebar":false}"#)
        .dispatch()
        .await;
    assert_eq!(hidden.status(), Status::NoContent);
    let own = client
        .get("/api/v1/podcasts/settings")
        .header(Header::new("Authorization", format!("Bearer {listener}")))
        .dispatch()
        .await
        .into_json::<Value>()
        .await
        .unwrap();
    assert_eq!(own["show_on_home"], false);
    assert_eq!(own["show_in_sidebar"], false);
    let other = client
        .get("/api/v1/podcasts/settings")
        .header(Header::new("Authorization", format!("Bearer {admin}")))
        .dispatch()
        .await
        .into_json::<Value>()
        .await
        .unwrap();
    assert_eq!(other["show_on_home"], true);
    assert_eq!(other["show_in_sidebar"], true);
    for (token, visible) in [
        (&listener, false),
        (&admin, true),
    ] {
        let bootstrap = client
            .get("/api/v1/bootstrap")
            .header(Header::new("Authorization", format!("Bearer {token}")))
            .dispatch()
            .await
            .into_json::<Value>()
            .await
            .unwrap();
        assert_eq!(bootstrap["podcast_settings"]["show_on_home"], visible);
        assert_eq!(bootstrap["podcast_settings"]["show_in_sidebar"], visible);
        assert_eq!(bootstrap["podcasts_enabled"], true);
    }

    let combined = client
        .get("/api/v1/items")
        .header(Header::new("Authorization", format!("Bearer {listener}")))
        .dispatch()
        .await
        .into_json::<Vec<Value>>()
        .await
        .unwrap();
    assert!(combined.is_empty());
    let own_library = client
        .get("/api/v1/podcasts")
        .header(Header::new("Authorization", format!("Bearer {listener}")))
        .dispatch()
        .await
        .into_json::<Vec<Value>>()
        .await
        .unwrap();
    assert_eq!(own_library.len(), 1);
    let settings = client
        .get("/api/v1/settings")
        .header(Header::new("Authorization", format!("Bearer {admin}")))
        .dispatch()
        .await
        .into_json::<Value>()
        .await
        .unwrap();
    let mut settings = settings["settings"].clone();
    settings["podcasts"]["enabled"] = Value::Bool(false);
    assert_eq!(
        client
            .put("/api/v1/settings")
            .header(Header::new("Authorization", format!("Bearer {listener}")))
            .header(ContentType::JSON)
            .body(settings.to_string())
            .dispatch()
            .await
            .status(),
        Status::Forbidden
    );
    assert_eq!(
        client
            .put("/api/v1/settings")
            .header(Header::new("Authorization", format!("Bearer {admin}")))
            .header(ContentType::JSON)
            .body(settings.to_string())
            .dispatch()
            .await
            .status(),
        Status::Ok
    );
    for path in [
        "/api/v1/podcasts",
        "/api/v1/podcasts/discover",
        "/api/v1/podcasts/home",
        "/api/v1/items/2",
        "/api/v1/items/2/playback",
        "/api/v1/podcasts/episodes/2/stream",
    ] {
        assert_eq!(
            client
                .get(path)
                .header(Header::new("Authorization", format!("Bearer {listener}")))
                .dispatch()
                .await
                .status(),
            Status::NotFound,
            "{path}"
        );
    }
    assert_eq!(
        client
            .post("/api/v1/podcasts")
            .header(Header::new("Authorization", format!("Bearer {listener}")))
            .header(ContentType::JSON)
            .body(r#"{"feed_url":"https://example.com/private"}"#)
            .dispatch()
            .await
            .status(),
        Status::NotFound
    );
    let bootstrap = client
        .get("/api/v1/bootstrap")
        .header(Header::new("Authorization", format!("Bearer {listener}")))
        .dispatch()
        .await
        .into_json::<Value>()
        .await
        .unwrap();
    assert_eq!(bootstrap["podcasts_enabled"], false);
    assert_eq!(bootstrap["podcast_settings"]["show_in_sidebar"], false);
    assert!(
        cached.exists(),
        "Disabling podcasts preserves existing downloads"
    );
    settings["podcasts"]["enabled"] = Value::Bool(true);
    assert_eq!(
        client
            .put("/api/v1/settings")
            .header(Header::new("Authorization", format!("Bearer {admin}")))
            .header(ContentType::JSON)
            .body(settings.to_string())
            .dispatch()
            .await
            .status(),
        Status::Ok
    );
    assert_eq!(
        client
            .get("/api/v1/items/2")
            .header(Header::new("Authorization", format!("Bearer {listener}")))
            .dispatch()
            .await
            .status(),
        Status::Ok
    );

    assert_eq!(
        client
            .delete("/api/v1/podcasts/1")
            .header(Header::new("Authorization", format!("Bearer {admin}")))
            .dispatch()
            .await
            .status(),
        Status::NoContent
    );
    assert_eq!(
        client
            .get("/api/v1/items/2")
            .header(Header::new("Authorization", format!("Bearer {admin}")))
            .dispatch()
            .await
            .status(),
        Status::NotFound
    );
    assert_eq!(
        client
            .get("/api/v1/items/2")
            .header(Header::new("Authorization", format!("Bearer {listener}")))
            .dispatch()
            .await
            .status(),
        Status::Ok
    );
    assert_eq!(
        client
            .post("/api/v1/podcasts/1/refresh")
            .header(Header::new("Authorization", format!("Bearer {admin}")))
            .dispatch()
            .await
            .status(),
        Status::NotFound
    );
    assert!(
        cached.exists(),
        "Another subscriber must retain the shared episode"
    );
    let response = client
        .get("/api/v1/podcasts/seasons/4/episodes?offset=0&limit=50")
        .header(Header::new("Authorization", format!("Bearer {listener}")))
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let page = response.into_json::<Value>().await.unwrap();
    assert_eq!(page["total"], 1);
    assert_eq!(page["items"][0]["id"], 2);
    let home = client
        .get("/api/v1/podcasts/home")
        .header(Header::new("Authorization", format!("Bearer {listener}")))
        .dispatch()
        .await
        .into_json::<Value>()
        .await
        .unwrap();
    for shelf in home["shelves"].as_array().unwrap() {
        for item in shelf["items"].as_array().unwrap() {
            assert_eq!(item["podcast"]["podcast_id"], 1);
        }
    }
    assert_eq!(
        client
            .delete("/api/v1/podcasts/1")
            .header(Header::new("Authorization", format!("Bearer {listener}")))
            .dispatch()
            .await
            .status(),
        Status::NoContent
    );
    assert!(
        !cached.exists(),
        "The final subscription must remove the saved episode"
    );
    DbConn::get_one(client.rocket())
        .await
        .unwrap()
        .run(|conn| {
            assert_eq!(
                items::table
                    .find(2)
                    .count()
                    .get_result::<i64>(conn)
                    .unwrap(),
                1
            );
            let json = metadata::table
                .find(2)
                .select(metadata::metadata_json)
                .first::<String>(conn)
                .unwrap();
            assert!(!json.contains("local_download"));
        })
        .await;
}

fn seed(conn: &mut diesel::SqliteConnection) {
    let password = bcrypt::hash("listener-password", 4).unwrap();
    diesel::insert_into(users::table)
        .values((
            users::id.eq(2),
            users::username.eq("listener"),
            users::password.eq(password),
        ))
        .execute(conn)
        .unwrap();
    diesel::insert_into(libraries::table)
        .values((
            libraries::id.eq(1),
            libraries::name.eq("Podcasts"),
            libraries::path.eq("koko://podcasts"),
            libraries::kind.eq("podcasts"),
            libraries::metadata_providers_json.eq("[]"),
        ))
        .execute(conn)
        .unwrap();
    for (id, title, kind, parent, playable) in [
        (1, "Private show", "podcast", None, false),
        (4, "2026", "podcast_season", Some(1), false),
        (2, "Private episode", "podcast_episode", Some(4), true),
        (3, "Admin's own show", "podcast", None, false),
    ] {
        diesel::insert_into(items::table)
            .values((
                items::id.eq(id),
                items::library_id.eq(1),
                items::identity_key.eq(format!("podcast-{id}")),
                items::item_type.eq(kind),
                items::display_title.eq(title),
                items::parent_id.eq(parent),
                items::playable.eq(playable),
                items::media_kind.eq("audio"),
            ))
            .execute(conn)
            .unwrap();
    }
    for (id, url) in [
        (1, "https://example.com/private"),
        (3, "https://example.com/admin"),
    ] {
        diesel::insert_into(feeds::table)
            .values((
                feeds::media_item_id.eq(id),
                feeds::feed_url.eq(url),
                feeds::checked_at.eq(i64::MAX),
            ))
            .execute(conn)
            .unwrap();
        diesel::insert_into(aliases::table)
            .values((aliases::feed_url.eq(url), aliases::podcast_id.eq(id)))
            .execute(conn)
            .unwrap();
    }
    for (user, id) in [(2, 1), (1, 3)] {
        diesel::insert_into(subscriptions::table)
            .values((
                subscriptions::user_id.eq(user),
                subscriptions::podcast_id.eq(id),
                subscriptions::added_at.eq(1),
            ))
            .execute(conn)
            .unwrap();
    }
    for (id, root, enclosure) in [
        (1, 1, None),
        (2, 1, Some("http://127.0.0.1/audio.mp3")),
        (3, 3, None),
        (4, 1, None),
    ] {
        let data = json!({
            "podcast_id":root,"author":"Publisher","description":"Show notes",
            "categories":[],"transcripts":[],"people":[]
        })
        .to_string();
        diesel::insert_into(metadata::table)
            .values((
                metadata::media_item_id.eq(id),
                metadata::podcast_id.eq(root),
                metadata::metadata_json.eq(data),
                metadata::enclosure_url.eq(enclosure),
                metadata::mime_type.eq("audio/mpeg"),
            ))
            .execute(conn)
            .unwrap();
    }
}
