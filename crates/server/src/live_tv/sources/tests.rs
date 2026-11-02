//! Provider refreshes preserve explicit guide choices, source errors, and imported channel state.

use super::*;
use crate::live_tv::test_support::{
    Database,
    HttpFixture,
};

fn guide_xml() -> String {
    let start = chrono::Utc::now();
    let end = start + chrono::Duration::hours(1);
    format!(
        "<tv><channel id='station'><display-name>Workshop</display-name></channel><programme \
         channel='station' start='{} +0000' stop='{} +0000'><title>Weekend \
         Workshop</title></programme></tv>",
        start.format("%Y%m%d%H%M%S"),
        end.format("%Y%m%d%H%M%S")
    )
}

#[rocket::async_test]
async fn tvheadend_automatic_guide_and_explicit_iptv_xmltv_are_persisted_with_sizes() {
    let xml = guide_xml();
    let fixture = HttpFixture::new(move |path| {
        let body = if path.contains("xml") {
            xml.clone()
        } else {
            "#EXTM3U\n#EXTINF:-1 tvg-id=\"station\",Workshop\nhttp://example.com/live.ts\n".into()
        };
        (200, "text/plain", body.into_bytes())
    })
    .await;
    for (kind, guide_kind) in [
        ("tvheadend", "automatic"),
        ("m3u", "xmltv"),
    ] {
        let database = Database::new().await;
        let config = SourceConfig {
            url: format!("{}/playlist.m3u", fixture.url),
            guide_kind: guide_kind.into(),
            xmltv_url: format!("{}/guide.xml", fixture.url),
            refresh_hours: 12,
            ..Default::default()
        };
        let (source, _) = database.source(kind, config, "").await;
        let db = database.conn().await;
        refresh(&db, source.id).await.unwrap();
        let id = source.id;
        db.run(move |conn| {
            let channels = storage::channels(conn, false).unwrap();
            assert_eq!(channels.len(), 1);
            assert_eq!(channels[0].name, "Workshop");
            let programs = storage::guide(
                conn,
                crate::live_tv::now() - 60,
                crate::live_tv::now() + 7200,
            )
            .unwrap();
            assert_eq!(programs.len(), 1);
            assert_eq!(programs[0].title, "Weekend Workshop");
            let sizes = crate::live_tv::setup::feed_sizes(conn, id).unwrap();
            assert!(sizes.playlist_bytes.unwrap() > 0);
            assert!(sizes.guide_bytes.unwrap() > 0);
            let saved = storage::source(conn, id).unwrap();
            assert!(saved.last_error.is_none());
            assert!((saved.next_refresh - crate::live_tv::now() - 43200).abs() < 5);
        })
        .await;
    }
}

#[rocket::async_test]
async fn hdhomerun_filters_encrypted_channels_and_records_refresh_failures() {
    use std::sync::{
        Arc,
        atomic::{
            AtomicBool,
            Ordering,
        },
    };
    let failing = Arc::new(AtomicBool::new(false));
    let fail = failing.clone();
    let xml = guide_xml();
    let fixture = HttpFixture::new(move |path| {
        if fail.load(Ordering::Relaxed) {
            return (503, "text/plain", b"Unavailable".to_vec());
        }
        let body = if path.ends_with("lineup.json") {
            serde_json::json!([
                {"GuideNumber":"7.1","GuideName":"Workshop","URL":"http://example.com/clear.ts"},
                {"GuideNumber":"8","URL":"http://example.com/protected.ts","DRM":1},
                {"GuideNumber":"9","URL":"http://example.com/protected.ts","DRM":true},
                {"GuideNumber":"10"}
            ])
            .to_string()
        } else {
            xml.clone()
        };
        (200, "text/plain", body.into_bytes())
    })
    .await;
    let database = Database::new().await;
    let config = SourceConfig {
        url: fixture.url.clone(),
        guide_kind: "xmltv".into(),
        xmltv_url: format!("{}/guide.xml", fixture.url),
        ..Default::default()
    };
    let (source, _) = database.source("hdhomerun", config, "").await;
    let db = database.conn().await;
    refresh(&db, source.id).await.unwrap();
    let channels = db.run(|conn| storage::channels(conn, false)).await.unwrap();
    assert_eq!(channels.len(), 1);
    assert_eq!(channels[0].number, "7.1");
    assert_eq!(
        stream_url(&source, &channels[0]).await.unwrap(),
        "http://example.com/clear.ts"
    );
    assert!(
        keepalive(&source, &channels[0].stream_url)
            .unwrap()
            .is_none()
    );
    failing.store(true, Ordering::Relaxed);
    assert!(refresh(&db, source.id).await.is_err());
    let id = source.id;
    db.run(move |conn| {
        let saved = storage::source(conn, id).unwrap();
        assert!(saved.last_error.unwrap().contains("returned an error"));
        assert!((saved.next_refresh - crate::live_tv::now() - 3600).abs() < 5);
        diesel::update(s::table.find(id))
            .set(s::enabled.eq(false))
            .execute(conn)
            .unwrap();
    })
    .await;
    assert!(refresh(&db, id).await.unwrap_err().contains("disabled"));
}
