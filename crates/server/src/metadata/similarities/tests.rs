//! Provider pagination, refresh persistence, and migration coverage with local HTTP fixtures.

use super::*;
use crate::db::schema::{
    item_metadata_links as links,
    media_items as items,
    media_libraries as libraries,
};
use serde_json::json;
use tokio::io::{
    AsyncReadExt,
    AsyncWriteExt,
};

async fn respond(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    payload: Value,
) {
    let body = payload.to_string();
    stream
        .write_all(
            format!(
                "HTTP/1.1 {status} Response\r\nContent-Type: application/json\r\nContent-Length: \
                 {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
}

async fn server(
    count: usize,
    response: fn(&str) -> (u16, Value),
) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let mut requests = Vec::new();
        for _ in 0..count {
            let (mut stream, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let mut bytes = Vec::new();
            while !bytes.windows(4).any(|chunk| chunk == b"\r\n\r\n") {
                let mut buffer = [0; 4096];
                let count = stream.read(&mut buffer).await.unwrap();
                assert!(count > 0);
                bytes.extend_from_slice(&buffer[..count]);
            }
            let request = String::from_utf8(bytes).unwrap();
            let (status, payload) = response(&request);
            respond(&mut stream, status, payload).await;
            requests.push(request);
        }
        requests
    });
    (url, task)
}

fn snapshot(
    provider: MetadataProviderId,
    kind: &str,
    id: &str,
    payload: Value,
) -> StoredMetadataSnapshot {
    StoredMetadataSnapshot {
        provider_id: provider,
        external_id: id.into(),
        media_type: Some(kind.into()),
        title: Some("Seed".into()),
        overview: None,
        artwork_url: None,
        backdrop_url: None,
        release_year: None,
        locale_key: "en-US".into(),
        provider_locale_key: Some("en-US".into()),
        provider_payload_json: Some(payload.to_string()),
    }
}

fn database() -> SqliteConnection {
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    crate::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    crate::db::configure_sqlite_connection(&mut conn).unwrap();
    diesel::insert_into(libraries::table)
        .values((
            libraries::id.eq(1),
            libraries::name.eq("Test"),
            libraries::path.eq("test"),
            libraries::kind.eq("movies"),
        ))
        .execute(&mut conn)
        .unwrap();
    diesel::insert_into(items::table)
        .values((
            items::id.eq(1),
            items::library_id.eq(1),
            items::identity_key.eq("seed"),
            items::item_type.eq("movie"),
            items::display_title.eq("Seed"),
        ))
        .execute(&mut conn)
        .unwrap();
    conn
}

fn settings() -> MetadataSettings {
    let mut settings = MetadataSettings::default();
    settings
        .providers
        .iter_mut()
        .find(|provider| provider.id == MetadataProviderId::Tmdb)
        .unwrap()
        .api_key = Some("test-key".into());
    settings
}

#[tokio::test]
async fn tmdb_refresh_saves_every_page_including_items_absent_from_the_library() {
    for kind in ["movie", "tv"] {
        let (url, task) = server(3, |request| {
            assert!(request.contains("api_key=test-key"));
            assert!(request.contains("/recommendations?"));
            if request.contains("page=2") {
                (200, json!({"total_pages":3,"results":[{"id":4}]}))
            } else if request.contains("page=3") {
                (
                    200,
                    json!({"total_pages":3,"results":[{"id":3},{"id":5},{"id":0},{"id":1}]}),
                )
            } else {
                (200, json!({"total_pages":3,"results":[{"id":2},{"id":3}]}))
            }
        })
        .await;
        let mut snapshot = snapshot(MetadataProviderId::Tmdb, kind, "1", json!({}));
        TEST_TMDB_ROOT
            .scope(url, enrich_snapshot(&settings(), &mut snapshot))
            .await;
        assert_eq!(task.await.unwrap().len(), 3);
        let mut conn = database();
        crate::metadata::upsert_item_metadata_snapshot(&mut conn, 1, &snapshot).unwrap();
        let saved = rows::table
            .order(rows::rank)
            .select((rows::media_type, rows::external_id))
            .load::<(String, String)>(&mut conn)
            .unwrap();
        assert_eq!(
            saved,
            ["2", "3", "4", "5"].map(|id| (kind.to_string(), id.to_string()))
        );
    }
}

#[tokio::test]
async fn excluded_snapshots_skip_provider_discovery_and_discard_saved_ids() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let mut conn = database();
    for (provider, kind) in [
        (MetadataProviderId::Tmdb, "tv_season"),
        (MetadataProviderId::Tmdb, "tv_episode"),
        (MetadataProviderId::Tmdb, "season"),
        (MetadataProviderId::Tmdb, "episode"),
        (MetadataProviderId::MusicBrainz, "recording"),
        (MetadataProviderId::MusicBrainz, "track"),
    ] {
        let original = snapshot(
            MetadataProviderId::Tmdb,
            "movie",
            "1",
            movie_similarities("2"),
        );
        let link = crate::metadata::upsert_item_metadata_snapshot(&mut conn, 1, &original).unwrap();
        assert_eq!(rows::table.count().get_result::<i64>(&mut conn).unwrap(), 1);
        let mut child = snapshot(
            provider,
            kind,
            "1",
            json!({"name":"Child", "artist-credit":[
                {"artist":{"id":"00000000-0000-0000-0000-000000000001"}}
            ]}),
        );
        let payload = child.provider_payload_json.clone();
        TEST_TMDB_ROOT
            .scope(
                url.clone(),
                TEST_LISTENBRAINZ_ROOT.scope(url.clone(), enrich_snapshot(&settings(), &mut child)),
            )
            .await;
        assert_eq!(child.provider_payload_json, payload);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), listener.accept())
                .await
                .is_err()
        );
        // A stale payload must not reintroduce child recommendations either.
        child.provider_payload_json = original.provider_payload_json;
        sync(&mut conn, link.id, &child, false).unwrap();
        assert_eq!(rows::table.count().get_result::<i64>(&mut conn).unwrap(), 0);
    }
}

#[tokio::test]
async fn tmdb_pages_overlap_with_a_global_concurrency_bound_and_keep_provider_order() {
    use std::sync::atomic::{
        AtomicUsize,
        Ordering,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let peak_for_server = Arc::clone(&peak);
    let third_done = Arc::new(tokio::sync::Notify::new());
    let task = tokio::spawn(async move {
        let mut workers = tokio::task::JoinSet::new();
        for _ in 0..9 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak_for_server);
            let third_done = Arc::clone(&third_done);
            workers.spawn(async move {
                let mut buffer = [0; 4096];
                let count = stream.read(&mut buffer).await.unwrap();
                let request = String::from_utf8_lossy(&buffer[..count]);
                assert!(request.contains("/recommendations?"));
                let target = request.split_whitespace().nth(1).unwrap();
                let url = reqwest::Url::parse(&format!("http://localhost{target}")).unwrap();
                let page: u32 = url
                    .query_pairs()
                    .find(|(key, _)| key == "page")
                    .unwrap()
                    .1
                    .parse()
                    .unwrap();
                let concurrent = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(concurrent, Ordering::SeqCst);
                // Serial pagination cannot complete this exchange: page 3 must finish before page
                // 2.
                if page == 2 {
                    third_done.notified().await;
                }
                respond(
                    &mut stream,
                    200,
                    json!({"total_pages": 9, "results": [{"id": page}]}),
                )
                .await;
                if page == 3 {
                    third_done.notify_one();
                }
                active.fetch_sub(1, Ordering::SeqCst);
            });
        }
        while let Some(result) = workers.join_next().await {
            result.unwrap();
        }
    });
    let mut snapshot = snapshot(MetadataProviderId::Tmdb, "movie", "99", json!({}));
    tokio::time::timeout(
        Duration::from_secs(5),
        TEST_TMDB_ROOT.scope(url, enrich_snapshot(&settings(), &mut snapshot)),
    )
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
    assert!((2..=TMDB_PAGE_CONCURRENCY).contains(&peak.load(Ordering::SeqCst)));
    let payload: Value =
        serde_json::from_str(snapshot.provider_payload_json.as_deref().unwrap()).unwrap();
    let saved: Vec<SimilarItem> = serde_json::from_value(payload[PAYLOAD_KEY].clone()).unwrap();
    assert_eq!(
        saved
            .iter()
            .map(|item| item.identity.external_id.clone())
            .collect::<Vec<_>>(),
        (1..=9).map(|page| page.to_string()).collect::<Vec<_>>()
    );
    assert_eq!(
        saved.iter().map(|item| item.rank).collect::<Vec<_>>(),
        (0..9).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn stalled_discovery_has_a_total_deadline_and_preserves_previous_ids() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buffer = [0; 4096];
        assert!(stream.read(&mut buffer).await.unwrap() > 0);
        std::future::pending::<()>().await;
        drop(stream);
    });
    let mut conn = database();
    let mut snapshot = snapshot(
        MetadataProviderId::Tmdb,
        "movie",
        "1",
        movie_similarities("99"),
    );
    crate::metadata::upsert_item_metadata_snapshot(&mut conn, 1, &snapshot).unwrap();
    snapshot.title = Some("Updated metadata".into());
    snapshot.provider_payload_json = Some("{}".into());
    tokio::time::timeout(
        Duration::from_secs(2),
        TEST_TMDB_ROOT.scope(
            url,
            enrich_snapshot_with_timeout(&settings(), &mut snapshot, Duration::from_millis(100)),
        ),
    )
    .await
    .unwrap();
    task.abort();
    crate::metadata::upsert_item_metadata_snapshot(&mut conn, 1, &snapshot).unwrap();
    assert_eq!(
        rows::table
            .select(rows::external_id)
            .load::<String>(&mut conn)
            .unwrap(),
        ["99"]
    );
    assert_eq!(
        links::table
            .select(links::title)
            .first::<Option<String>>(&mut conn)
            .unwrap()
            .as_deref(),
        Some("Updated metadata")
    );
}

#[test]
fn large_id_lists_are_saved_in_batches_instead_of_one_statement_per_recommendation() {
    use diesel::connection::InstrumentationEvent;
    use std::sync::atomic::{
        AtomicUsize,
        Ordering,
    };
    let mut conn = database();
    let mut snapshot = snapshot(MetadataProviderId::Tmdb, "movie", "1", json!({}));
    crate::metadata::upsert_item_metadata_snapshot(&mut conn, 1, &snapshot).unwrap();
    let link_id = links::table
        .select(links::id)
        .first::<i32>(&mut conn)
        .unwrap();
    let items: Vec<_> = (0..1024)
        .map(|rank| SimilarItem {
            identity: Identity::new("tmdb", "movie", &(rank + 2).to_string()),
            rank,
        })
        .collect();
    snapshot.provider_payload_json = Some(json!({"koko_similar_items": items}).to_string());
    let queries = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&queries);
    conn.set_instrumentation(move |event: InstrumentationEvent<'_>| {
        if matches!(event, InstrumentationEvent::StartQuery { .. }) {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    });
    sync(&mut conn, link_id, &snapshot, false).unwrap();
    assert_eq!(queries.load(Ordering::SeqCst), 1 + 1024 / INSERT_BATCH_SIZE);
    assert_eq!(
        rows::table.count().get_result::<i64>(&mut conn).unwrap(),
        1024
    );
}

#[tokio::test]
async fn listenbrainz_refresh_keeps_artist_matches_for_artists_and_albums() {
    let artist = "00000000-0000-0000-0000-000000000001";
    for kind in [
        "artist",
        "release",
        "release-group",
    ] {
        let (url, task) = server(1, |request| {
            assert!(request.contains("/similar-artists/json"));
            assert!(request.contains("artist_mbids="));
            (
                200,
                json!([
                    {"artist_mbid": "00000000-0000-0000-0000-000000000020"},
                    {"artist_mbid": "00000000-0000-0000-0000-000000000020"},
                    {"artist_mbid": "invalid"}
                ]),
            )
        })
        .await;
        let mut snapshot = snapshot(
            MetadataProviderId::MusicBrainz,
            kind,
            artist,
            json!({"id":artist,"title":"Album","artist-credit":[
                {"artist":{"id":artist}}, {"artist":{"id":artist}}
            ]}),
        );
        TEST_LISTENBRAINZ_ROOT
            .scope(url, enrich_snapshot(&settings(), &mut snapshot))
            .await;
        assert_eq!(task.await.unwrap().len(), 1);
        let payload: Value =
            serde_json::from_str(snapshot.provider_payload_json.as_deref().unwrap()).unwrap();
        let saved: Vec<SimilarItem> = serde_json::from_value(payload[PAYLOAD_KEY].clone()).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(
            saved[0].identity,
            Identity::new(
                "musicbrainz",
                "artist",
                "00000000-0000-0000-0000-000000000020"
            )
        );
    }
}

#[tokio::test]
async fn incomplete_provider_refresh_does_not_replace_previous_results_with_a_partial_list() {
    let (url, task) = server(2, |request| {
        if request.contains("page=2") {
            (503, json!({"error":"offline"}))
        } else {
            (200, json!({"total_pages":2,"results":[{"id":2}]}))
        }
    })
    .await;
    let mut snapshot = snapshot(MetadataProviderId::Tmdb, "movie", "1", json!({}));
    TEST_TMDB_ROOT
        .scope(url, enrich_snapshot(&settings(), &mut snapshot))
        .await;
    task.await.unwrap();
    assert!(
        !snapshot
            .provider_payload_json
            .unwrap()
            .contains(PAYLOAD_KEY)
    );
}

fn movie_similarities(id: &str) -> Value {
    json!({"koko_similar_items": [{
        "provider_id": "tmdb", "media_type": "movie", "external_id": id, "rank": 0
    }]})
}

#[test]
fn refresh_preserves_failures_and_clears_changed_or_unlinked_identities() {
    let mut conn = database();
    let mut snapshot = snapshot(
        MetadataProviderId::Tmdb,
        "movie",
        "1",
        movie_similarities("99"),
    );
    crate::metadata::upsert_item_metadata_snapshot(&mut conn, 1, &snapshot).unwrap();
    assert_eq!(rows::table.count().get_result::<i64>(&mut conn).unwrap(), 1);
    snapshot.provider_payload_json = Some("{}".into());
    crate::metadata::upsert_item_metadata_snapshot(&mut conn, 1, &snapshot).unwrap();
    assert_eq!(rows::table.count().get_result::<i64>(&mut conn).unwrap(), 1);
    snapshot.external_id = "2".into();
    crate::metadata::upsert_item_metadata_snapshot(&mut conn, 1, &snapshot).unwrap();
    assert_eq!(rows::table.count().get_result::<i64>(&mut conn).unwrap(), 0);
    snapshot.provider_payload_json = Some(movie_similarities("100").to_string());
    crate::metadata::upsert_item_metadata_snapshot(&mut conn, 1, &snapshot).unwrap();
    assert_eq!(
        rows::table
            .select(rows::external_id)
            .first::<String>(&mut conn)
            .unwrap(),
        "100"
    );
    snapshot.provider_payload_json = Some(json!({"koko_similar_items":[]}).to_string());
    crate::metadata::upsert_item_metadata_snapshot(&mut conn, 1, &snapshot).unwrap();
    assert_eq!(rows::table.count().get_result::<i64>(&mut conn).unwrap(), 0);
    snapshot.provider_payload_json = Some(movie_similarities("101").to_string());
    crate::metadata::upsert_item_metadata_snapshot(&mut conn, 1, &snapshot).unwrap();
    diesel::delete(links::table).execute(&mut conn).unwrap();
    assert_eq!(rows::table.count().get_result::<i64>(&mut conn).unwrap(), 0);
}

#[test]
fn similarity_migration_can_be_reverted_and_reapplied_without_changing_metadata_links() {
    use diesel::connection::SimpleConnection;
    let mut conn = database();
    let snapshot = snapshot(MetadataProviderId::Tmdb, "movie", "1", json!({}));
    crate::metadata::upsert_item_metadata_snapshot(&mut conn, 1, &snapshot).unwrap();
    conn.batch_execute(include_str!(
        "../../../sql/migrations/54af66a9dbe7_item_similarities/down.sql"
    ))
    .unwrap();
    conn.batch_execute(include_str!(
        "../../../sql/migrations/54af66a9dbe7_item_similarities/up.sql"
    ))
    .unwrap();
    assert_eq!(
        links::table.count().get_result::<i64>(&mut conn).unwrap(),
        1
    );
    assert_eq!(rows::table.count().get_result::<i64>(&mut conn).unwrap(), 0);
}
