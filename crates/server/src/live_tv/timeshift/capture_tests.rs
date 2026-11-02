//! Capture ownership, decoder output, retained recordings, and resource release with synthetic TV.

use super::{
    BufferInfo,
    close,
    keep,
    play,
    start,
    status,
};
use crate::db::DbConn;
use crate::db::schema::{
    live_tv_programs as programs,
    live_tv_recordings as recordings,
};
use crate::live_tv::{
    SourceConfig,
    settings,
    stream,
    test_support::{
        Database,
        HttpFixture,
    },
};
use diesel::prelude::*;
use std::time::Duration;

// Twelve seconds of generated black video and silence, with two-second keyframe spacing.
const TRANSPORT: &[u8] = include_bytes!("../fixtures/sample.mpegts");

#[rocket::get("/<id>")]
async fn playback(id: &str) -> Result<stream::LiveStream, String> {
    play(id, 1, Some(0), "original").await
}

async fn captured(
    db: &DbConn,
    id: &str,
) -> BufferInfo {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let current = status(db, id, 1).await.unwrap();
            assert!(current.error.is_none(), "{:?}", current.error);
            if current.finished {
                return current;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap()
}

#[rocket::async_test]
async fn a_completed_capture_plays_real_mp4_and_is_removed_only_by_its_owner() {
    let fixture = HttpFixture::new(|_| (200, "video/mp2t", TRANSPORT.to_vec())).await;
    let database = Database::new().await;
    let (source, channel) = database
        .source(
            "m3u",
            SourceConfig {
                url: fixture.url.clone(),
                ..Default::default()
            },
            &format!("{}/live.ts", fixture.url),
        )
        .await;
    let db = database.conn().await;
    let initial = start(&db, &database.path, 1, channel.id, true)
        .await
        .unwrap();
    let current = captured(&db, &initial.session_id).await;
    assert_eq!(current.channel_id, channel.id);
    assert!(current.end_ms >= 10000);
    assert!(status(&db, &initial.session_id, 2).await.is_err());
    assert!(close(&initial.session_id, 2).await.is_err());
    assert!(
        play(&initial.session_id, 2, None, "original")
            .await
            .is_err()
    );
    assert!(
        keep(&db, &initial.session_id, 1, false)
            .await
            .err()
            .unwrap()
            .contains("ended")
    );
    let client = rocket::local::asynchronous::Client::tracked(
        rocket::build().mount("/", rocket::routes![playback]),
    )
    .await
    .unwrap();
    let response = client
        .get(format!("/{}", initial.session_id))
        .dispatch()
        .await;
    assert_eq!(response.status(), rocket::http::Status::Ok);
    assert_eq!(
        response.content_type(),
        Some(rocket::http::ContentType::MP4)
    );
    let video = response.into_bytes().await.unwrap();
    assert!(video.windows(4).any(|bytes| bytes == b"ftyp"));
    assert!(video.windows(4).any(|bytes| bytes == b"moof"));
    let directory = database
        .directory
        .path()
        .join("recordings/.buffers")
        .join(&initial.session_id);
    assert!(directory.is_dir());
    close(&initial.session_id, 1).await.unwrap();
    assert!(!directory.exists());
    assert!(!stream::source_in_use(source.id));
    assert!(status(&db, &initial.session_id, 1).await.is_err());
}

#[rocket::async_test]
async fn retaining_a_buffer_survives_closing_playback_and_keeps_current_program_metadata() {
    let (fixture, release) = HttpFixture::held(|_| (200, "video/mp2t", TRANSPORT.to_vec())).await;
    let database = Database::new().await;
    let (source, channel) = database
        .source(
            "m3u",
            SourceConfig {
                url: fixture.url.clone(),
                ..Default::default()
            },
            &format!("{}/live.ts", fixture.url),
        )
        .await;
    let db = database.conn().await;
    let source_id = source.id;
    db.run(move |conn| {
        diesel::insert_into(programs::table)
            .values((
                programs::source_id.eq(source_id),
                programs::guide_key.eq("station"),
                programs::title.eq("Weekend Workshop"),
                programs::starts_at.eq(crate::live_tv::now() - 60),
                programs::ends_at.eq(crate::live_tv::now() + 3600),
                programs::metadata_json.eq(r#"{"episode":"S1E2"}"#),
            ))
            .execute(conn)
            .unwrap();
    })
    .await;
    let initial = start(&db, &database.path, 1, channel.id, false)
        .await
        .unwrap();
    assert_eq!(initial.end_ms, 0);
    assert!(stream::source_in_use(source.id));
    let recording = keep(&db, &initial.session_id, 1, false).await.unwrap();
    assert_eq!(
        keep(&db, &initial.session_id, 1, true).await.unwrap(),
        recording
    );
    assert_eq!(
        status(&db, &initial.session_id, 1)
            .await
            .unwrap()
            .recording_id,
        Some(recording)
    );
    close(&initial.session_id, 1).await.unwrap();
    assert!(stream::source_in_use(source.id));
    release.send(true).unwrap();
    let saved = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let saved = db
                .run(move |conn| {
                    recordings::table
                        .find(recording)
                        .select((
                            recordings::state,
                            recordings::file_name,
                            recordings::shared,
                            recordings::title,
                            recordings::metadata_json,
                        ))
                        .first::<(String, Option<String>, bool, String, String)>(conn)
                })
                .await
                .unwrap();
            if saved.0 != "recording" {
                return saved;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(saved.0, "completed");
    assert!(saved.2);
    assert_eq!(saved.3, "Weekend Workshop");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&saved.4).unwrap()["episode"],
        "S1E2"
    );
    let file = saved.1.unwrap();
    assert!(tokio::fs::metadata(file).await.unwrap().len() > 188);
    tokio::time::timeout(Duration::from_secs(3), async {
        while stream::source_in_use(source.id) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}

#[rocket::async_test]
async fn permissions_and_cancelled_retention_are_checked_against_the_current_database() {
    let (fixture, release) = HttpFixture::held(|_| (200, "video/mp2t", TRANSPORT.to_vec())).await;
    let database = Database::new().await;
    let (_, channel) = database
        .source(
            "m3u",
            SourceConfig {
                url: fixture.url.clone(),
                ..Default::default()
            },
            &format!("{}/live.ts", fixture.url),
        )
        .await;
    let db = database.conn().await;
    let initial = start(&db, &database.path, 1, channel.id, false)
        .await
        .unwrap();
    db.run(|conn| settings::set_permission(conn, 1, false))
        .await
        .unwrap();
    assert!(!status(&db, &initial.session_id, 1).await.unwrap().can_keep);
    assert!(
        keep(&db, &initial.session_id, 1, true)
            .await
            .err()
            .unwrap()
            .contains("disabled")
    );
    db.run(|conn| settings::set_permission(conn, 1, true))
        .await
        .unwrap();
    let recording = keep(&db, &initial.session_id, 1, true).await.unwrap();
    db.run(move |conn| {
        assert!(
            !recordings::table
                .find(recording)
                .select(recordings::shared)
                .first::<bool>(conn)
                .unwrap()
        );
        diesel::update(recordings::table.find(recording))
            .set(recordings::state.eq("stopping"))
            .execute(conn)
    })
    .await
    .unwrap();
    release.send(true).unwrap();
    captured(&db, &initial.session_id).await;
    let final_state = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let state = db
                .run(move |conn| {
                    recordings::table
                        .find(recording)
                        .select(recordings::state)
                        .first::<String>(conn)
                })
                .await
                .unwrap();
            if state != "stopping" {
                return state;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(final_state, "cancelled");
    close(&initial.session_id, 1).await.unwrap();
}

#[rocket::async_test]
async fn cancelling_a_cold_capture_releases_its_tuner_and_terminates_pending_playback() {
    let (fixture, _release) = HttpFixture::held(|_| (200, "video/mp2t", TRANSPORT.to_vec())).await;
    let database = Database::new().await;
    let (source, channel) = database
        .source(
            "m3u",
            SourceConfig {
                url: fixture.url.clone(),
                ..Default::default()
            },
            &format!("{}/live.ts", fixture.url),
        )
        .await;
    let db = database.conn().await;
    let initial = start(&db, &database.path, 1, channel.id, false)
        .await
        .unwrap();
    let id = initial.session_id.clone();
    let pending = tokio::spawn(async move { play(&id, 1, None, "original").await.map(drop) });
    tokio::time::timeout(Duration::from_secs(3), close(&initial.session_id, 1))
        .await
        .unwrap()
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(3), pending)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    assert!(!stream::source_in_use(source.id));
    assert!(
        !database
            .directory
            .path()
            .join("recordings/.buffers")
            .join(&initial.session_id)
            .exists()
    );
}

#[rocket::async_test]
async fn an_unavailable_upstream_reports_its_http_status_and_releases_the_source() {
    let fixture = HttpFixture::new(|_| (404, "text/plain", b"No channel".to_vec())).await;
    let database = Database::new().await;
    let (source, channel) = database
        .source(
            "m3u",
            SourceConfig {
                url: fixture.url.clone(),
                ..Default::default()
            },
            &format!("{}/missing.ts", fixture.url),
        )
        .await;
    let db = database.conn().await;
    assert!(
        start(&db, &database.path, 1, channel.id, true)
            .await
            .err()
            .unwrap()
            .contains("404")
    );
    assert!(!stream::source_in_use(source.id));
}
