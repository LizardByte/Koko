//! Live TV setup permissions and personal DVR ownership must also hold at the HTTP boundary.

use crate::test_utils::{
    create_admin_test_client,
    login_user,
    schema::{
        live_tv_channels as c,
        live_tv_recordings as r,
        live_tv_sources as s,
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
async fn live_tv_routes_enforce_setup_roles_and_recording_ownership() {
    let (client, admin) = create_admin_test_client(Some("live_tv_permissions")).await;
    let password = format!("fixture-{:032x}", rand::random::<u128>());
    let password_hash = bcrypt::hash(&password, 4).unwrap();
    DbConn::get_one(client.rocket())
        .await
        .unwrap()
        .run(move |conn| {
            diesel::insert_into(users::table)
                .values((
                    users::id.eq(2),
                    users::username.eq("tv-viewer"),
                    users::password.eq(password_hash),
                    users::admin.eq(false),
                ))
                .execute(conn)
                .unwrap();
            diesel::insert_into(s::table)
                .values((
                    s::id.eq(1),
                    s::name.eq("IPTV"),
                    s::kind.eq("m3u"),
                    s::config_json.eq("secret-ref"),
                    s::capacity.eq(1),
                    s::next_refresh.eq(i64::MAX),
                ))
                .execute(conn)
                .unwrap();
            diesel::insert_into(c::table)
                .values((
                    c::id.eq(1),
                    c::source_id.eq(1),
                    c::source_key.eq("stable"),
                    c::name.eq("TV"),
                    c::number.eq("1"),
                    c::group_name.eq(""),
                    c::stream_url.eq("http://example.com/stream?token=private"),
                    c::guide_key.eq("station"),
                ))
                .execute(conn)
                .unwrap();
            for (id, user, title) in [
                (1, 1, "Admin recording"),
                (2, 2, "Viewer recording"),
            ] {
                diesel::insert_into(r::table)
                    .values((
                        r::id.eq(id),
                        r::user_id.eq(user),
                        r::channel_id.eq(1),
                        r::title.eq(title),
                        r::starts_at.eq(chrono::Utc::now().timestamp() + 3600),
                        r::ends_at.eq(chrono::Utc::now().timestamp() + 7200),
                        r::padding_before.eq(0),
                        r::padding_after.eq(0),
                        r::state.eq("scheduled"),
                        r::metadata_json.eq("{}"),
                        r::shared.eq(false),
                    ))
                    .execute(conn)
                    .unwrap();
            }
        })
        .await;
    let viewer = login_user(&client, "tv-viewer", &password, Some(Status::Ok))
        .await
        .unwrap();
    let private_stream = client
        .get("/api/v1/live-tv/recordings/1/stream")
        .cookie(Cookie::new("koko_media_auth", viewer.clone()))
        .dispatch()
        .await;
    assert_eq!(private_stream.status(), Status::BadRequest);
    let cookie_mutation = client
        .post("/api/v1/live-tv/recordings/1/cancel")
        .cookie(Cookie::new("koko_media_auth", viewer.clone()))
        .dispatch()
        .await;
    assert_eq!(cookie_mutation.status(), Status::Unauthorized);
    for path in [
        "/api/v1/live-tv/channels",
        "/api/v1/live-tv/guide",
        "/api/v1/live-tv/recordings",
        "/api/v1/live-tv/rules",
    ] {
        assert_eq!(
            client
                .get(path)
                .header(Header::new("Authorization", "Bearer invalid"))
                .dispatch()
                .await
                .status(),
            Status::Unauthorized
        );
    }
    for path in [
        "/api/v1/live-tv/sources",
        "/api/v1/live-tv/setup/channels",
    ] {
        assert_eq!(
            client
                .get(path)
                .header(Header::new("Authorization", format!("Bearer {viewer}")))
                .dispatch()
                .await
                .status(),
            Status::Forbidden
        );
    }
    let body = client
        .get("/api/v1/live-tv/channels")
        .header(Header::new("Authorization", format!("Bearer {viewer}")))
        .dispatch()
        .await
        .into_string()
        .await
        .unwrap();
    assert!(!body.contains("token=private"));
    assert!(!body.contains("stream_url"));
    let own: Value = client
        .get("/api/v1/live-tv/recordings")
        .header(Header::new("Authorization", format!("Bearer {viewer}")))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    assert_eq!(own["recordings"].as_array().unwrap().len(), 1);
    assert_eq!(own["recordings"][0]["title"], "Viewer recording");
    client
        .post("/api/v1/live-tv/recordings/1/cancel")
        .header(Header::new("Authorization", format!("Bearer {viewer}")))
        .dispatch()
        .await;
    let untouched = DbConn::get_one(client.rocket())
        .await
        .unwrap()
        .run(|conn| {
            r::table
                .find(1)
                .select(r::state)
                .first::<String>(conn)
                .unwrap()
        })
        .await;
    assert_eq!(untouched, "scheduled");
    let denied = client
        .put("/api/v1/live-tv/channels/1")
        .header(Header::new("Authorization", format!("Bearer {viewer}")))
        .header(ContentType::JSON)
        .body(json!({"enabled":false,"guide_override":null}).to_string())
        .dispatch()
        .await;
    assert_eq!(denied.status(), Status::Forbidden);
    let updated = client
        .put("/api/v1/live-tv/channels/1")
        .header(Header::new("Authorization", format!("Bearer {admin}")))
        .header(ContentType::JSON)
        .body(json!({"enabled":false,"guide_override":null}).to_string())
        .dispatch()
        .await;
    assert_eq!(updated.status(), Status::NoContent);
    let visible: Value = client
        .get("/api/v1/live-tv/channels")
        .header(Header::new("Authorization", format!("Bearer {viewer}")))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    assert_eq!(visible, json!([]));
    // A shared recording is visible across accounts without granting mutation ownership.
    DbConn::get_one(client.rocket())
        .await
        .unwrap()
        .run(|conn| {
            diesel::update(r::table.find(1))
                .set(r::shared.eq(true))
                .execute(conn)
                .unwrap();
        })
        .await;
    let shared: Value = client
        .get("/api/v1/live-tv/recordings")
        .header(Header::new("Authorization", format!("Bearer {viewer}")))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    assert_eq!(shared["recordings"].as_array().unwrap().len(), 2);
    assert_eq!(shared["can_record"], true);
    let forbidden = client
        .put("/api/v1/live-tv/dvr/permissions/1")
        .header(Header::new("Authorization", format!("Bearer {viewer}")))
        .header(ContentType::JSON)
        .body(json!({"can_record":false}).to_string())
        .dispatch()
        .await;
    assert_eq!(forbidden.status(), Status::Forbidden);
    let denied = client
        .put("/api/v1/live-tv/dvr/permissions/2")
        .header(Header::new("Authorization", format!("Bearer {admin}")))
        .header(ContentType::JSON)
        .body(json!({"can_record":false}).to_string())
        .dispatch()
        .await;
    assert_eq!(denied.status(), Status::NoContent);
    let denied_timer = client
        .post("/api/v1/live-tv/recordings")
        .header(Header::new("Authorization", format!("Bearer {viewer}")))
        .header(ContentType::JSON)
        .body(
            json!({
                "channel_id": 1,
                "title": "Denied timer",
                "starts_at": chrono::Utc::now().timestamp()+100,
                "ends_at": chrono::Utc::now().timestamp()+200
            })
            .to_string(),
        )
        .dispatch()
        .await;
    assert_eq!(denied_timer.status(), Status::BadRequest);
    assert!(
        denied_timer
            .into_string()
            .await
            .unwrap()
            .contains("administrator")
    );
    let folder = std::env::temp_dir().join(format!("koko-dvr-{:032x}", rand::random::<u128>()));
    let saved = client
        .put("/api/v1/live-tv/dvr/settings")
        .header(Header::new("Authorization", format!("Bearer {admin}")))
        .header(ContentType::JSON)
        .body(json!({"recording_dir":folder.to_string_lossy()}).to_string())
        .dispatch()
        .await;
    assert_eq!(saved.status(), Status::NoContent);
    let settings: Value = client
        .get("/api/v1/live-tv/dvr/settings")
        .header(Header::new("Authorization", format!("Bearer {admin}")))
        .dispatch()
        .await
        .into_json()
        .await
        .unwrap();
    assert_eq!(
        settings["settings"]["recording_dir"],
        folder.to_string_lossy().as_ref()
    );
    std::fs::remove_dir(folder).unwrap();
}
