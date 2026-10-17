use diesel::connection::SimpleConnection;
use diesel::{
    Connection,
    SqliteConnection,
};
use koko::config::MetadataProviderId;
use koko::metadata::{
    StoredMetadataSnapshot,
    get_item_metadata_summaries,
    set_item_metadata_refresh_state_for_locale,
    upsert_item_metadata_snapshot,
};
use std::io::{
    Read,
    Write,
};

fn connection() -> SqliteConnection {
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    koko::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    conn.batch_execute(
        "INSERT INTO media_libraries (id,name,path,kind) VALUES (1,'Review','review','movies'); \
         INSERT INTO media_items (id,library_id,identity_key,item_type,display_title) VALUES \
         (1,1,'review','movie','Movie');",
    )
    .unwrap();
    conn
}

fn snapshot(locale: &str) -> StoredMetadataSnapshot {
    StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tmdb,
        external_id: "603".into(),
        media_type: Some("movie".into()),
        title: Some("Movie".into()),
        overview: None,
        artwork_url: None,
        backdrop_url: None,
        release_year: Some(1999),
        locale_key: locale.into(),
        provider_locale_key: Some(locale.into()),
        provider_payload_json: None,
    }
}

#[test]
fn clearing_metadata_removes_every_locale_and_descendant_but_preserves_the_catalog() {
    use diesel::RunQueryDsl;
    let mut conn = connection();
    conn.batch_execute(
        "UPDATE media_items SET item_type='show' WHERE id=1;
         INSERT INTO media_items (id,library_id,parent_id,identity_key,item_type,display_title) \
         VALUES
         (2,1,1,'season','season','Season 1'), (3,1,2,'episode','episode','Episode 1'),
         (4,1,NULL,'other','movie','Other');
         INSERT INTO playback_progress (media_item_id,position_ms) VALUES (3,12345);",
    )
    .unwrap();
    for item_id in 1..=4 {
        for locale in ["en-US", "es-ES"] {
            upsert_item_metadata_snapshot(&mut conn, item_id, &snapshot(locale)).unwrap();
        }
    }
    let secondary = StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Themerr,
        ..snapshot("en-US")
    };
    koko::metadata::upsert_item_metadata_link(
        &mut conn,
        2,
        &secondary,
        &Default::default(),
        "secondary",
        None,
    )
    .unwrap();
    conn.batch_execute(
        "INSERT INTO item_metadata_external_ids (metadata_link_id,source,external_id)
         SELECT id,'imdb','tt123' FROM item_metadata_links WHERE media_item_id=3;
         INSERT INTO item_metadata_people (metadata_link_id,name)
         SELECT id,'Actor' FROM item_metadata_links WHERE media_item_id=3;",
    )
    .unwrap();

    assert_eq!(
        koko::metadata::clear_item_metadata_tree(&mut conn, 1, false).unwrap(),
        vec![1, 2, 3]
    );
    for item_id in 1..=3 {
        assert!(
            get_item_metadata_summaries(&mut conn, item_id)
                .unwrap()
                .is_empty()
        );
        let item = koko::media::get_media_item_summary(&mut conn, item_id)
            .unwrap()
            .unwrap();
        assert!(!item.has_metadata);
        assert!(item.overview.is_none());
        assert!(item.metadata_refresh_error.is_none());
    }
    assert_eq!(get_item_metadata_summaries(&mut conn, 4).unwrap().len(), 2);
    #[derive(diesel::QueryableByName)]
    struct Remaining {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        external_ids: i64,
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        people: i64,
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        catalog: i64,
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        position: i64,
    }
    let remaining = diesel::sql_query(
        "SELECT
        (SELECT COUNT(*) FROM item_metadata_external_ids WHERE external_id='tt123') AS \
         external_ids,
        (SELECT COUNT(*) FROM item_metadata_people) AS people,
        (SELECT COUNT(*) FROM media_items) AS catalog,
        (SELECT position_ms FROM playback_progress LIMIT 1) AS position",
    )
    .get_result::<Remaining>(&mut conn)
    .unwrap();
    assert_eq!(remaining.external_ids, 0);
    assert_eq!(remaining.people, 0);
    assert_eq!(remaining.catalog, 4);
    assert_eq!(remaining.position, 12345);
}

#[test]
fn explicit_unlink_is_hidden_and_suppresses_automatic_matching_until_rematched() {
    let mut conn = connection();
    upsert_item_metadata_snapshot(&mut conn, 1, &snapshot("en-US")).unwrap();
    koko::metadata::clear_item_metadata_tree(&mut conn, 1, true).unwrap();
    assert!(
        get_item_metadata_summaries(&mut conn, 1)
            .unwrap()
            .is_empty()
    );
    assert!(
        koko::metadata::get_primary_item_metadata_link(&mut conn, 1)
            .unwrap()
            .is_none()
    );
    assert!(
        koko::media::get_preferred_item_metadata_link(&mut conn, 1)
            .unwrap()
            .is_none()
    );
    assert!(
        !koko::media::get_media_item_summary(&mut conn, 1)
            .unwrap()
            .unwrap()
            .has_metadata
    );
    assert!(
        koko::metadata::list_due_item_metadata_links(&mut conn, i64::MAX, 8)
            .unwrap()
            .is_empty()
    );
    koko::metadata::clear_item_metadata_tree(&mut conn, 1, false).unwrap();
    let mut replacement = snapshot("en-US");
    replacement.external_id = "604".into();
    upsert_item_metadata_snapshot(&mut conn, 1, &replacement).unwrap();
    let matches = get_item_metadata_summaries(&mut conn, 1).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].external_id, "604");
}

fn artwork_dir() -> std::path::PathBuf {
    let path =
        std::env::temp_dir().join(format!("koko-metadata-artwork-{}", rand::random::<u64>()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn png() -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::RgbImage::new(1, 1)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

fn serve_artwork(
    status: u16,
    body: Vec<u8>,
) -> (String, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/poster.png", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0; 2048];
        assert!(stream.read(&mut buffer).unwrap() > 0);
        write!(
            stream,
            "HTTP/1.1 {status} Result\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(&body).unwrap();
    });
    (url, server)
}

#[tokio::test]
async fn metadata_artwork_rejects_http_errors_and_invalid_images() {
    let dir = artwork_dir();
    for status in [404, 503, 200] {
        let (url, server) = serve_artwork(status, b"<html>unavailable</html>".to_vec());
        assert!(
            koko::metadata::cache_artwork(&url, &dir, "poster")
                .await
                .is_err()
        );
        server.join().unwrap();
        assert!(!koko::metadata::expected_artwork_cache_path(&url, &dir, "poster").exists());
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn metadata_artwork_refetches_previously_corrupted_cache_entries() {
    let dir = artwork_dir();
    let bytes = png();
    // A truncated PNG can still have a valid dimensions header.
    for corrupted in [
        b"old cached 404".as_slice(),
        &bytes[..33],
    ] {
        let (url, server) = serve_artwork(200, bytes.clone());
        let path = koko::metadata::expected_artwork_cache_path(&url, &dir, "poster");
        std::fs::write(&path, corrupted).unwrap();
        assert_eq!(
            koko::metadata::cache_artwork(&url, &dir, "poster")
                .await
                .unwrap(),
            path
        );
        server.join().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn metadata_artwork_keeps_shared_images_after_a_replacement_is_ready() {
    let dir = artwork_dir();
    let old = koko::metadata::expected_artwork_cache_path(
        "http://old.example/poster.png",
        &dir,
        "poster",
    );
    std::fs::write(&old, png()).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let unavailable = format!("http://{}/poster.png", listener.local_addr().unwrap());
    drop(listener);
    assert!(
        koko::metadata::cache_artwork(&unavailable, &dir, "poster")
            .await
            .is_err()
    );
    assert!(old.exists());
    let (url, server) = serve_artwork(200, png());
    let replacement = koko::metadata::cache_artwork(&url, &dir, "poster")
        .await
        .unwrap();
    server.join().unwrap();
    assert!(replacement.is_file());
    assert!(old.exists());
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn metadata_asset_persistence_keeps_serving_the_previous_artwork_on_failure() {
    let dir = artwork_dir();
    let data_dir = dir.to_str().unwrap();
    let mut record = snapshot("en-US");
    let item_dir = koko::metadata::managed_metadata_asset_dir(
        data_dir,
        record.provider_id.clone(),
        &record.external_id,
        record.media_type.as_deref(),
        &record.locale_key,
    );
    std::fs::create_dir_all(&item_dir).unwrap();
    let old = koko::metadata::expected_artwork_cache_path(
        "http://old.example/poster.png",
        &item_dir,
        "tmdb_poster",
    );
    std::fs::write(&old, png()).unwrap();
    let (url, server) = serve_artwork(503, Vec::new());
    record.artwork_url = Some(url);
    let (poster, _, _) = koko::metadata::persist_item_metadata_assets(&record, 1, data_dir)
        .await
        .unwrap();
    server.join().unwrap();
    assert_eq!(poster, Some(old.clone()));

    let (url, server) = serve_artwork(200, png());
    record.artwork_url = Some(url);
    let (poster, _, _) = koko::metadata::persist_item_metadata_assets(&record, 1, data_dir)
        .await
        .unwrap();
    server.join().unwrap();
    assert!(poster.unwrap().is_file());
    assert!(old.exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn obsolete_artwork_downloads_and_cache_hits_cannot_delete_newer_covers() {
    let dir = artwork_dir();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let old_url = format!("http://{}/old.png", listener.local_addr().unwrap());
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let old_server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = [0; 2048];
        assert!(socket.read(&mut request).unwrap() > 0);
        started_tx.send(()).unwrap();
        release_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        let bytes = png();
        write!(
            socket,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            bytes.len()
        )
        .unwrap();
        socket.write_all(&bytes).unwrap();
    });
    let old_job = tokio::spawn({
        let dir = dir.clone();
        let old_url = old_url.clone();
        async move {
            koko::metadata::cache_artwork(&old_url, &dir, "poster")
                .await
                .unwrap()
        }
    });
    started_rx.await.unwrap();
    let (new_url, new_server) = serve_artwork(200, png());
    let current = koko::metadata::cache_artwork(&new_url, &dir, "poster")
        .await
        .unwrap();
    new_server.join().unwrap();
    release_tx.send(()).unwrap();
    let old = old_job.await.unwrap();
    old_server.join().unwrap();
    assert!(
        current.is_file(),
        "An obsolete download must not remove the current cover"
    );
    assert_eq!(
        koko::metadata::cache_artwork(&old_url, &dir, "poster")
            .await
            .unwrap(),
        old
    );
    assert!(
        current.is_file(),
        "A cached obsolete URL must not remove the current cover"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn metadata_refresh_state_changes_only_the_requested_translation() {
    let mut conn = connection();
    for locale in ["en-US", "es-ES"] {
        upsert_item_metadata_snapshot(&mut conn, 1, &snapshot(locale)).unwrap();
    }
    set_item_metadata_refresh_state_for_locale(
        &mut conn,
        koko::metadata::MetadataRefreshStateUpdate {
            item_id: 1,
            provider_id: MetadataProviderId::Tmdb,
            external_id: "603",
            media_type: Some("movie"),
            locale_key: "es-ES",
            refresh_state: "error",
            refresh_error: Some("failed"),
        },
    )
    .unwrap();
    let rows = get_item_metadata_summaries(&mut conn, 1).unwrap();
    assert_eq!(
        rows.iter()
            .find(|row| row.locale_key == "en-US")
            .unwrap()
            .refresh_state,
        "fresh"
    );
    assert_eq!(
        rows.iter()
            .find(|row| row.locale_key == "es-ES")
            .unwrap()
            .refresh_state,
        "error"
    );
    set_item_metadata_refresh_state_for_locale(
        &mut conn,
        koko::metadata::MetadataRefreshStateUpdate {
            item_id: 1,
            provider_id: MetadataProviderId::Tmdb,
            external_id: "603",
            media_type: Some("movie"),
            locale_key: "fr-FR",
            refresh_state: "pending",
            refresh_error: None,
        },
    )
    .unwrap();
    let rows = get_item_metadata_summaries(&mut conn, 1).unwrap();
    assert_eq!(
        rows.iter()
            .find(|row| row.locale_key == "fr-FR")
            .unwrap()
            .refresh_state,
        "pending"
    );
}

#[test]
fn refresh_placeholder_links_do_not_claim_metadata_is_available() {
    let mut conn = connection();
    let data_dir = std::env::temp_dir();
    for state in ["pending", "error"] {
        set_item_metadata_refresh_state_for_locale(
            &mut conn,
            koko::metadata::MetadataRefreshStateUpdate {
                item_id: 1,
                provider_id: MetadataProviderId::Tmdb,
                external_id: "603",
                media_type: Some("movie"),
                locale_key: "en-US",
                refresh_state: state,
                refresh_error: Some("metadata has not been fetched"),
            },
        )
        .unwrap();
        let summary = koko::media::get_media_item_summary(&mut conn, 1)
            .unwrap()
            .unwrap();
        assert!(!summary.has_metadata);
        assert_eq!(summary.metadata_refresh_state.as_deref(), Some(state));
        let detail = koko::media::get_media_item(&mut conn, 1, data_dir.to_str().unwrap())
            .unwrap()
            .unwrap();
        assert!(!detail.has_metadata);
        assert_eq!(detail.metadata_refresh_state.as_deref(), Some(state));
    }

    upsert_item_metadata_snapshot(&mut conn, 1, &snapshot("en-US")).unwrap();
    for state in ["fresh", "pending", "error"] {
        set_item_metadata_refresh_state_for_locale(
            &mut conn,
            koko::metadata::MetadataRefreshStateUpdate {
                item_id: 1,
                provider_id: MetadataProviderId::Tmdb,
                external_id: "603",
                media_type: Some("movie"),
                locale_key: "en-US",
                refresh_state: state,
                refresh_error: None,
            },
        )
        .unwrap();
        assert!(
            koko::media::get_media_item_summary(&mut conn, 1)
                .unwrap()
                .unwrap()
                .has_metadata
        );
        assert!(
            koko::media::get_media_item(&mut conn, 1, data_dir.to_str().unwrap())
                .unwrap()
                .unwrap()
                .has_metadata
        );
    }
}

#[test]
fn failed_metadata_refreshes_do_not_starve_later_due_items() {
    let mut conn = connection();
    for id in 1..=9 {
        if id > 1 {
            conn.batch_execute(&format!(
                "INSERT INTO media_items (id,library_id,identity_key,item_type,display_title) \
                 VALUES ({id},1,'movie-{id}','movie','Movie');"
            ))
            .unwrap();
        }
        upsert_item_metadata_snapshot(&mut conn, id, &snapshot("en-US")).unwrap();
    }
    conn.batch_execute("UPDATE item_metadata_links SET next_refresh_at = 0")
        .unwrap();
    let now = koko::utils::current_timestamp();
    for row in koko::metadata::list_due_item_metadata_links(&mut conn, now, 8).unwrap() {
        set_item_metadata_refresh_state_for_locale(
            &mut conn,
            koko::metadata::MetadataRefreshStateUpdate {
                item_id: row.media_item_id,
                provider_id: MetadataProviderId::Tmdb,
                external_id: "603",
                media_type: Some("movie"),
                locale_key: "en-US",
                refresh_state: "error",
                refresh_error: Some("404"),
            },
        )
        .unwrap();
    }
    let next = koko::metadata::list_due_item_metadata_links(&mut conn, now, 8).unwrap();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].media_item_id, 9);
    assert_eq!(
        koko::metadata::list_due_item_metadata_links(&mut conn, now + 301, 9)
            .unwrap()
            .len(),
        9
    );
}

#[test]
fn metadata_rematching_invalidates_old_secondary_extras() {
    let mut conn = connection();
    upsert_item_metadata_snapshot(&mut conn, 1, &snapshot("en-US")).unwrap();
    let mut secondary = snapshot("en-US");
    secondary.provider_id = MetadataProviderId::Themerr;
    secondary.external_id = "movie:tmdb:603".into();
    let details = koko::metadata::ProviderMetadataDetails {
        theme_song_url: Some("https://www.youtube.com/watch?v=abcdefghijk".into()),
        ..Default::default()
    };
    koko::metadata::upsert_item_metadata_link(
        &mut conn,
        1,
        &secondary,
        &details,
        "secondary",
        None,
    )
    .unwrap();
    assert_eq!(get_item_metadata_summaries(&mut conn, 1).unwrap().len(), 2);
    let mut corrected = snapshot("en-US");
    corrected.external_id = "604".into();
    upsert_item_metadata_snapshot(&mut conn, 1, &corrected).unwrap();
    assert_eq!(get_item_metadata_summaries(&mut conn, 1).unwrap().len(), 1);
    koko::metadata::upsert_item_metadata_link(
        &mut conn,
        1,
        &secondary,
        &details,
        "secondary",
        None,
    )
    .unwrap();
    set_item_metadata_refresh_state_for_locale(
        &mut conn,
        koko::metadata::MetadataRefreshStateUpdate {
            item_id: 1,
            provider_id: MetadataProviderId::Tmdb,
            external_id: "605",
            media_type: Some("movie"),
            locale_key: "en-US",
            refresh_state: "pending",
            refresh_error: None,
        },
    )
    .unwrap();
    assert_eq!(get_item_metadata_summaries(&mut conn, 1).unwrap().len(), 1);
}

#[test]
fn metadata_secondary_misses_clear_only_the_affected_provider_and_locale() {
    let mut conn = connection();
    for locale in ["en-US", "es-ES"] {
        let mut secondary = snapshot(locale);
        secondary.provider_id = MetadataProviderId::TrailerDb;
        koko::metadata::upsert_item_metadata_link(
            &mut conn,
            1,
            &secondary,
            &Default::default(),
            "secondary",
            None,
        )
        .unwrap();
    }
    koko::metadata::clear_item_secondary_metadata(
        &mut conn,
        1,
        MetadataProviderId::TrailerDb,
        "es-ES",
    )
    .unwrap();
    let rows = get_item_metadata_summaries(&mut conn, 1).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].locale_key, "en-US");
}

#[test]
fn secondary_collection_themes_are_cleared_after_a_miss_or_rematch() {
    use diesel::RunQueryDsl;
    #[derive(diesel::QueryableByName)]
    struct CountRow {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        count: i64,
    }
    let mut conn = connection();
    let details = koko::metadata::ProviderMetadataDetails {
        collections: ["234", "235"]
            .map(|id| koko::metadata::ProviderMetadataCollection {
                external_id: id.into(),
                name: Some(format!("Collection {id}")),
                overview: None,
                artwork_url: None,
                backdrop_url: None,
                theme_song_url: None,
            })
            .into(),
        ..Default::default()
    };
    koko::metadata::upsert_item_metadata_link(
        &mut conn,
        1,
        &snapshot("en-US"),
        &details,
        "primary",
        None,
    )
    .unwrap();
    let references = koko::media::get_item_youtube_theme_collection_references(
        &mut conn,
        1,
        MetadataProviderId::Themerr,
    )
    .unwrap();
    assert_eq!(references.len(), 2);
    for (id, item_type, database_id, external_id) in &references {
        koko::metadata::upsert_secondary_collection_theme_song_url(
            &mut conn,
            *id,
            MetadataProviderId::Themerr,
            item_type,
            database_id,
            external_id,
            "https://www.youtube.com/watch?v=abcdefghijk",
        )
        .unwrap();
    }
    let secondary_memberships = |conn: &mut SqliteConnection| {
        diesel::sql_query(
            "SELECT COUNT(*) AS count FROM metadata_collection_items i JOIN metadata_collections \
             c ON c.id = i.collection_id WHERE i.media_item_id = 1 AND c.relation_kind = \
             'secondary'",
        )
        .get_result::<CountRow>(conn)
        .unwrap()
        .count
    };
    assert_eq!(secondary_memberships(&mut conn), 2);
    koko::metadata::clear_secondary_collection_metadata(
        &mut conn,
        references[0].0,
        MetadataProviderId::Themerr,
    )
    .unwrap();
    assert_eq!(secondary_memberships(&mut conn), 1);
    assert_eq!(
        diesel::sql_query(
            "SELECT COUNT(*) AS count FROM metadata_collections WHERE relation_kind = 'primary'"
        )
        .get_result::<CountRow>(&mut conn)
        .unwrap()
        .count,
        2
    );

    let mut corrected = snapshot("en-US");
    corrected.external_id = "604".into();
    upsert_item_metadata_snapshot(&mut conn, 1, &corrected).unwrap();
    assert_eq!(secondary_memberships(&mut conn), 0);
}
