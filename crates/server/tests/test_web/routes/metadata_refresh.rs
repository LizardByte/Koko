use super::*;
use diesel::connection::SimpleConnection;

const SHOW: i32 = 195_000_001;
const SEASON: i32 = 195_000_002;
const EPISODE: i32 = 195_000_003;

#[tokio::test]
async fn album_link_and_refresh_retry_unmatched_tracks_before_fetching_lyrics() {
    for rematch in [true, false] {
        album_tracks_retry_case(rematch).await;
    }
}

async fn album_tracks_retry_case(rematch: bool) {
    use crate::db::schema::{
        media_items,
        music_item_metadata,
    };
    use diesel::{
        ExpressionMethods,
        QueryDsl,
        RunQueryDsl,
    };
    const TRACK: i32 = 202_000_001;
    const ALBUM: i32 = TRACK + 1;
    const AMBIGUOUS: i32 = TRACK + 2;
    const OUTSIDE: i32 = TRACK + 3;
    let (_rocket, db) = super::super::music::tests::lyrics_database(TRACK).await;
    let revision = db
        .run(move |conn| {
            for (id, parent, title, number) in [
                (AMBIGUOUS, Some(ALBUM), "Ambiguous", 2),
                (OUTSIDE, None, "Outside", 1),
            ] {
                diesel::insert_into(media_items::table)
                    .values((
                        media_items::id.eq(id),
                        media_items::library_id.eq(1),
                        media_items::parent_id.eq(parent),
                        media_items::identity_key.eq(title),
                        media_items::item_type.eq("track"),
                        media_items::display_title.eq(title),
                        media_items::duration_ms.eq(180000),
                    ))
                    .execute(conn)
                    .unwrap();
                diesel::insert_into(music_item_metadata::table)
                    .values((
                        music_item_metadata::media_item_id.eq(id),
                        music_item_metadata::artist.eq("Artist"),
                        music_item_metadata::album_artist.eq("Artist"),
                        music_item_metadata::album.eq("Album"),
                        music_item_metadata::track_number.eq(number),
                    ))
                    .execute(conn)
                    .unwrap();
            }
            diesel::update(music_item_metadata::table.find(TRACK))
                .set(music_item_metadata::track_number.eq(1))
                .execute(conn)
                .unwrap();
            diesel::update(media_items::table.find(TRACK))
                .set(media_items::display_title.eq("Track 01"))
                .execute(conn)
                .unwrap();
            if rematch {
                // This is the same atomic reset used before the manual link worker starts.
                metadata_jobs::reset(conn, ALBUM, false, |_| Ok(()))
                    .unwrap()
                    .1
            } else {
                use crate::db::schema::item_metadata_links;
                // Refresh leaves the album linked and must retry earlier unsuccessful matches.
                diesel::delete(
                    item_metadata_links::table.filter(item_metadata_links::media_item_id.eq(TRACK)),
                )
                .execute(conn)
                .unwrap();
                diesel::update(
                    media_items::table.filter(media_items::id.eq_any([TRACK, AMBIGUOUS, OUTSIDE])),
                )
                .set(media_items::metadata_match_attempted_at.eq(1_i64))
                .execute(conn)
                .unwrap();
                MetadataRevision::current(ALBUM)
            }
        })
        .await;
    let release_id = "d817f725-66c3-366d-8dd1-b22d49c8a766";
    let recording_id = "24cc8311-98fd-423a-bed1-97728f5eabc5";
    let recording = serde_json::json!({"id":recording_id,"title":"Song","length":180000,"score":100,
        "artist-credit":[{"artist":{"name":"Artist"}}],
        "releases":[{"id":release_id,"media":[{"position":1,"track-offset":0,
            "track":[{"number":"1","title":"Song","length":180000}]}]}]});
    let ambiguous = serde_json::json!({
        "id":"1f9e42d1-97dd-4226-a44b-4ec30b197c07", "title":"Ambiguous",
        "length":180000, "score":100,
        "releases":[{"id":release_id,"media":[{"position":1,"track-offset":1,
            "track":[{"number":"2","title":"Ambiguous","length":180000}]}]}]
    });
    let mut duplicate = ambiguous.clone();
    duplicate["id"] = serde_json::json!("cd83ca98-3d2e-4a83-abd0-92b117cbc080");
    let responses = vec![
        (
            format!("/ws/2/release/{release_id}"),
            None,
            serde_json::json!({
                "id":release_id, "title":"Album", "cover-art-archive":{"front":false}
            }),
        ),
        (
            "/ws/2/recording".into(),
            Some(format!("recording:\"Track 01\" AND reid:{release_id}")),
            serde_json::json!({"recordings":[]}),
        ),
        (
            "/ws/2/recording".into(),
            Some(format!("tnum:1 AND reid:{release_id}")),
            serde_json::json!({"count":1,"recordings":[recording.clone()]}),
        ),
        (format!("/ws/2/recording/{recording_id}"), None, recording),
        (
            "/ws/2/recording".into(),
            Some(format!("recording:\"Ambiguous\" AND reid:{release_id}")),
            serde_json::json!({"recordings":[ambiguous.clone(),duplicate.clone()]}),
        ),
        (
            "/ws/2/recording".into(),
            Some(format!("tnum:2 AND reid:{release_id}")),
            serde_json::json!({"count":3,"recordings":[ambiguous,duplicate]}),
        ),
    ];
    let (root, worker) = album_match_responses(responses);
    let (lyrics_url, lyrics_worker) = super::super::music::tests::mock_lyrics(vec![(
        "200 OK",
        r#"{"plainLyrics":"Matched song lyrics"}"#,
    )]);
    let target = MetadataRefreshTarget {
        revision,
        locale_key: None,
        item_id: ALBUM,
        library_id: 1,
        provider_id: MetadataProviderId::MusicBrainz,
        item_type: "album".into(),
        display_title: "Album".into(),
        relative_path: String::new(),
        external_id: release_id.into(),
        media_type: "release".into(),
        fetch_kind: MetadataRefreshFetchKind::Direct,
    };
    crate::metadata::TEST_MUSICBRAINZ_API_ROOT
        .scope(
            root,
            super::super::music::TEST_LYRICS_URL.scope(lyrics_url, async {
                assert!(
                    !execute_manual_metadata_refresh_target(&db, &target, &Settings::default())
                        .await
                );
            }),
        )
        .await;
    worker.join().unwrap();
    lyrics_worker.join().unwrap();
    db.run(move |conn| {
        assert_eq!(
            get_primary_item_metadata_link(conn, TRACK)
                .unwrap()
                .unwrap()
                .external_id,
            recording_id
        );
        assert_eq!(
            crate::media::lyrics::load(conn, TRACK)
                .unwrap()
                .unwrap()
                .plain_lyrics
                .as_deref(),
            Some("Matched song lyrics")
        );
        for id in [AMBIGUOUS, OUTSIDE] {
            assert!(get_primary_item_metadata_link(conn, id).unwrap().is_none());
            assert!(crate::media::lyrics::load(conn, id).unwrap().is_none());
        }
        assert!(get_media_item_summary(conn, OUTSIDE).unwrap().is_some());
    })
    .await;
    assert_eq!(
        search_music_track_metadata(&db, OUTSIDE, "Outside", None)
            .await
            .unwrap_err(),
        Status::BadRequest
    );
}

fn album_match_responses(
    responses: Vec<(String, Option<String>, serde_json::Value)>
) -> (String, std::thread::JoinHandle<()>) {
    use std::io::{
        Read,
        Write,
    };
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let root = format!("http://{}/ws/2", listener.local_addr().unwrap());
    let worker = std::thread::spawn(move || {
        for (path, query, body) in responses {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(std::time::Duration::from_millis(10))
                    }
                    Err(error) => panic!("Expected album matching request: {error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut bytes = [0; 16384];
            let size = stream.read(&mut bytes).unwrap();
            let request = String::from_utf8_lossy(&bytes[..size]);
            let url = reqwest::Url::parse(&format!(
                "http://localhost{}",
                request.split_whitespace().nth(1).unwrap()
            ))
            .unwrap();
            assert_eq!(url.path(), path);
            if let Some(query) = query {
                assert!(
                    url.query_pairs()
                        .any(|(key, value)| key == "query" && value == query)
                );
            }
            let body = body.to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: \
                 {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    (root, worker)
}

async fn wait_for_library_scans_and_metadata(library_ids: &[i32]) {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            let scans_done = !ACTIVE_LIBRARY_SCANS
                .lock()
                .await
                .iter()
                .any(|id| library_ids.contains(id));
            let metadata_done = !ACTIVE_LIBRARY_METADATA_REFRESHES
                .read()
                .await
                .iter()
                .any(|id| library_ids.contains(id));
            if scans_done && metadata_done {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

fn existing_scan_metadata(
    conn: &mut diesel::SqliteConnection,
    ids: &[i32],
) -> (Vec<(i32, Option<i64>)>, String) {
    use crate::db::schema::{
        item_metadata_links::dsl as links,
        media_items::dsl as items,
    };
    use diesel::{
        ExpressionMethods,
        QueryDsl,
        RunQueryDsl,
        SelectableHelper,
    };
    let attempts = items::media_items
        .filter(items::id.eq_any(ids))
        .order(items::id)
        .select((items::id, items::metadata_match_attempted_at))
        .load(conn)
        .unwrap();
    let metadata = links::item_metadata_links
        .filter(links::media_item_id.eq_any(ids))
        .order(links::id)
        .select(crate::db::models::ItemMetadataLink::as_select())
        .load::<crate::db::models::ItemMetadataLink>(conn)
        .unwrap();
    (attempts, format!("{metadata:?}"))
}

#[test]
fn scan_backfills_only_new_show_descendants_for_each_show_provider() {
    use diesel::Connection;
    let mut conn = diesel::SqliteConnection::establish(":memory:").unwrap();
    crate::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    conn.batch_execute(
        "INSERT INTO media_libraries (id,name,path,kind) VALUES
        (1,'Shows','shows','shows');
        INSERT INTO media_items (id,library_id,parent_id,identity_key,item_type,display_title) \
         VALUES
        (1,1,NULL,'tmdb-show','show','TMDB show'),
        (2,1,1,'tmdb-season','season','Season 1'),
        (3,1,2,'tmdb-old','episode','Old episode'),
        (4,1,2,'tmdb-new','episode','New episode'),
        (5,1,NULL,'tvdb-show','show','TVDB show'),
        (6,1,5,'tvdb-season','season','Season 1'),
        (7,1,6,'tvdb-old','episode','Old episode'),
        (8,1,6,'tvdb-new','episode','New episode');
        INSERT INTO item_metadata_links
            (media_item_id,provider_id,external_id,media_type,refresh_state) VALUES
        (1,'tmdb','123','tv','fresh'), (2,'tmdb','old-season','tv_season','error'),
        (3,'tmdb','old-episode','tv_episode','error'),
        (5,'tvdb','456','series','fresh'), (6,'tvdb','old-season','season','error'),
        (7,'tvdb','old-episode','episode','error');",
    )
    .unwrap();
    let empty = AutomaticMetadataScope::NewItems(HashSet::new());
    assert!(
        linked_shows_needing_descendant_backfill(&mut conn, Some(1), &empty)
            .unwrap()
            .is_empty()
    );
    for (new_episode, show, provider, external_id) in [
        (4, 1, MetadataProviderId::Tmdb, "123"),
        (8, 5, MetadataProviderId::Tvdb, "456"),
    ] {
        let scope = AutomaticMetadataScope::NewItems(HashSet::from([new_episode]));
        assert_eq!(
            linked_shows_needing_descendant_backfill(&mut conn, Some(1), &scope).unwrap(),
            vec![(show, provider, external_id.into())]
        );
    }
    let manual = AutomaticMetadataScope::AllUnmatched;
    assert_eq!(
        linked_shows_needing_descendant_backfill(&mut conn, Some(1), &manual)
            .unwrap()
            .len(),
        2
    );
}

fn target(
    item_id: i32,
    item_type: &str,
) -> MetadataRefreshTarget {
    MetadataRefreshTarget {
        revision: MetadataRevision::current(item_id),
        locale_key: Some("en-US".into()),
        item_id,
        library_id: SHOW,
        provider_id: MetadataProviderId::Tmdb,
        item_type: item_type.into(),
        display_title: item_type.into(),
        relative_path: item_type.into(),
        external_id: "1412".into(),
        media_type: "tv".into(),
        fetch_kind: MetadataRefreshFetchKind::Direct,
    }
}

#[tokio::test]
async fn automatic_descendants_publish_pending_work_and_count_child_failures() {
    let rocket = rocket::custom(rocket::Config::figment().merge((
        "databases.sqlite_db",
        serde_json::json!({"url": ":memory:", "pool_size": 1}),
    )))
    .attach(DbConn::fairing())
    .ignite()
    .await
    .unwrap();
    let db = DbConn::get_one(&rocket).await.unwrap();
    db.run(|conn| {
        crate::db::run_pending_sqlite_migrations(conn).unwrap();
        conn.batch_execute(
            "INSERT INTO media_libraries (id,name,path,kind) VALUES
            (195000001,'Shows','shows','shows');
            INSERT INTO media_items (id,library_id,parent_id,identity_key,item_type,display_title)
            VALUES (195000001,195000001,NULL,'show','show','Show'),
                (195000002,195000001,195000001,'season','season','Season 1'),
                (195000003,195000001,195000002,'episode','episode','Episode 1');",
        )
        .unwrap();
    })
    .await;
    let mut settings = Settings::default();
    // Fail every provider call immediately, without network traffic.
    settings.metadata.providers.clear();

    // Cover a match extending its library activity and background backfill
    // creating its own activity. Both must publish child work before fetching.
    for has_parent in [true, false] {
        let parent = if has_parent {
            Some(
                register_metadata_refresh_activity(
                    "library",
                    "automatic_match",
                    "Match show".into(),
                    Some(SHOW),
                    Some(SHOW),
                    vec![target(SHOW, "show")],
                )
                .await
                .unwrap()
                .0,
            )
        } else {
            None
        };
        let execution_block = ACTIVE_METADATA_REFRESH_EXECUTIONS.write().await;
        let observe = async {
            let activity_id = loop {
                let pending = db
                    .run(|conn| list_media_items(conn, Some(SHOW)))
                    .await
                    .unwrap();
                if pending
                    .iter()
                    .filter(|item| item.metadata_refresh_state.as_deref() == Some("pending"))
                    .count()
                    == 2
                {
                    let activity = current_system_activities()
                        .await
                        .into_iter()
                        .find(|activity| activity.library_id == Some(SHOW))
                        .unwrap();
                    assert!(activity.item_ids.contains(&SEASON));
                    assert!(activity.item_ids.contains(&EPISODE));
                    assert_eq!(activity.total_items, if has_parent { 3 } else { 2 });
                    assert_eq!(activity.completed_items, 0);
                    break activity.id;
                }
                tokio::task::yield_now().await;
            };
            drop(execution_block);
            activity_id
        };
        let (result, activity_id) =
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                tokio::join!(
                    execute_metadata_refresh_targets(
                        &db,
                        vec![
                            target(SEASON, "season"),
                            target(EPISODE, "episode")
                        ],
                        &settings,
                        parent.as_deref()
                    ),
                    observe
                )
            })
            .await
            .unwrap();
        result.unwrap();
        if let Some(parent) = parent {
            let activity = current_system_activities()
                .await
                .into_iter()
                .find(|entry| entry.id == parent)
                .unwrap();
            assert_eq!(activity.completed_items, 2);
            assert_eq!(activity.failed_items, 2);
            assert_eq!(activity.item_ids, vec![SHOW]);
            record_metadata_refresh_activity_progress(&parent, SHOW, false).await;
            let activity = current_system_activities()
                .await
                .into_iter()
                .find(|entry| entry.id == parent)
                .unwrap();
            assert_eq!(activity.completed_items, 3);
            assert_eq!(activity.failed_items, 2);
            complete_metadata_refresh_activity(&parent).await;
        }
        assert!(
            !current_system_activities()
                .await
                .iter()
                .any(|entry| entry.id == activity_id)
        );
        for id in [SEASON, EPISODE] {
            let rows = db
                .run(move |conn| get_item_metadata_summaries(conn, id))
                .await
                .unwrap();
            assert_eq!(rows[0].refresh_state, "error");
            assert!(!ACTIVE_METADATA_REFRESH_ITEMS.read().await.contains_key(&id));
        }
    }
}

#[tokio::test]
async fn added_libraries_of_every_kind_queue_scans_and_metadata_work() {
    use diesel::{
        ExpressionMethods,
        QueryDsl,
        RunQueryDsl,
    };
    const FIRST: i32 = 197_000_001;
    const IDS: [i32; 7] = [
        FIRST,
        FIRST + 1,
        FIRST + 2,
        FIRST + 3,
        FIRST + 4,
        FIRST + 5,
        FIRST + 6,
    ];
    const KINDS: [&str; 7] = [
        "music",
        "movies",
        "shows",
        "home_videos",
        "photos",
        "books",
        "mixed",
    ];
    let root = std::env::temp_dir().join(format!(
        "koko-auto-scan-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let album = root.join("Artist/Album");
    std::fs::create_dir_all(&album).unwrap();
    for number in 1..=10 {
        std::fs::write(album.join(format!("{number:02} Song.flac")), b"audio").unwrap();
    }
    let season = root.join("Show/Season 1");
    std::fs::create_dir_all(&season).unwrap();
    std::fs::write(season.join("Show - S01E01 - Pilot.mkv"), b"video").unwrap();
    std::fs::write(root.join("Movie (2020).mkv"), b"video").unwrap();
    std::fs::write(root.join("Photo.jpg"), b"image").unwrap();
    std::fs::write(root.join("Book.epub"), b"book").unwrap();
    let rocket = rocket::custom(rocket::Config::figment().merge((
        "databases.sqlite_db",
        serde_json::json!({"url": root.join("catalog.db"), "pool_size": 9}),
    )))
    .attach(DbConn::fairing())
    .ignite()
    .await
    .unwrap();
    let db = DbConn::get_one(&rocket).await.unwrap();
    let library_path = root.to_string_lossy().to_string();
    db.run(move |conn| {
        crate::db::configure_sqlite_connection(conn).unwrap();
        crate::db::run_pending_sqlite_migrations(conn).unwrap();
        conn.batch_execute(
            "INSERT INTO users (id,username,password,admin) VALUES (1,'viewer','hash',0)",
        )
        .unwrap();
        for (id, kind) in IDS.into_iter().zip(KINDS) {
            diesel::sql_query(
                "INSERT INTO media_libraries (id,name,path,kind,metadata_providers_json) VALUES \
                 (?, 'Library', ?, ?, '[\"open_library\"]')",
            )
            .bind::<diesel::sql_types::Integer, _>(id)
            .bind::<diesel::sql_types::Text, _>(&library_path)
            .bind::<diesel::sql_types::Text, _>(kind)
            .execute(conn)
            .unwrap();
        }
    })
    .await;
    let mut settings = Settings::default();
    settings.ffmpeg.ffprobe_path = "missing-auto-scan-test-ffprobe".into();
    // An unimplemented provider exercises the complete queue without network calls.
    settings.metadata.providers.clear();
    let scan_guard = CATALOG_SCAN_EXECUTION.lock().await;
    for id in IDS.into_iter().chain([FIRST]) {
        let scan_db = DbConn::get_one(&rocket).await.unwrap();
        scan_db
            .run(|conn| crate::db::configure_sqlite_connection(conn).unwrap())
            .await;
        start_library_scan(
            scan_db,
            settings.clone(),
            id,
            "Library".into(),
            "library_added",
        )
        .await;
    }
    let queued = current_system_activities()
        .await
        .into_iter()
        .filter(|activity| activity.library_id.is_some_and(|id| IDS.contains(&id)))
        .collect::<Vec<_>>();
    assert_eq!(queued.len(), IDS.len());
    assert!(
        queued
            .iter()
            .all(|activity| activity.state == "queued" && activity.source == "library_added")
    );
    drop(scan_guard);
    wait_for_library_scans_and_metadata(&IDS).await;
    db.run(|conn| {
        use crate::db::schema::media_items::dsl as items;
        for (id, kind) in IDS.into_iter().zip(KINDS) {
            let scanned = list_media_items(conn, Some(id)).unwrap();
            assert!(
                !scanned.is_empty(),
                "{kind} library must scan automatically"
            );
            let home = crate::media::get_media_home(conn, Some(1), Some(id)).unwrap();
            let continue_title = match kind {
                "music" => "Continue listening",
                "books" => "Continue reading",
                _ => "Continue watching",
            };
            assert_eq!(home.shelves[0].title, continue_title);
            if kind == "music" {
                assert_eq!(home.shelves[1].items.len(), 1);
                assert_eq!(home.shelves[1].items[0].item_type, "album");
            }
            if kind == "shows" {
                assert!(scanned.iter().any(|item| item.item_type == "show"));
                assert!(scanned.iter().any(|item| item.item_type == "episode"));
            }
            if kind != "music" && kind != "movies" {
                continue;
            }
            let attempted = items::media_items
                .filter(items::library_id.eq(id))
                .filter(items::metadata_match_attempted_at.is_not_null())
                .count()
                .get_result::<i64>(conn)
                .unwrap();
            assert_eq!(
                attempted as usize,
                scanned.len(),
                "{kind} items must be matched after scanning"
            );
            if kind != "music" {
                continue;
            }
            assert_eq!(scanned.len(), 12);
            assert_eq!(
                scanned
                    .iter()
                    .filter(|item| item.item_type == "track")
                    .count(),
                10
            );
            assert_eq!(
                attempted, 12,
                "every item must be matched, beyond the normal eight-item batch"
            );
        }
    })
    .await;
    assert!(
        !current_system_activities()
            .await
            .iter()
            .any(|activity| activity.library_id.is_some_and(|id| IDS.contains(&id)))
    );

    // Seed edited, fresh, failed, and never-attempted older items. Scanning must
    // preserve every metadata field and must not retry older failed matches.
    let (old_ids, before) = db
        .run(|conn| {
            use crate::db::schema::media_items::dsl as items;
            let ids = items::media_items
                .filter(items::library_id.eq_any(IDS))
                .select(items::id)
                .load::<i32>(conn)
                .unwrap();
            for id in &ids {
                diesel::sql_query(
                    "UPDATE media_items SET metadata_match_attempted_at =
                CASE WHEN id % 3 = 0 THEN NULL ELSE 123 END WHERE id = ?",
                )
                .bind::<diesel::sql_types::Integer, _>(id)
                .execute(conn)
                .unwrap();
                if id % 3 == 0 {
                    continue;
                }
                diesel::sql_query(
                    "INSERT INTO item_metadata_links
                (media_item_id,provider_id,external_id,media_type,title,overview,refresh_state,
                 last_refreshed_at,next_refresh_at,refresh_error,updated_at)
                SELECT id,'tmdb',CAST(id AS TEXT),
                CASE WHEN item_type='show' THEN 'tv' ELSE 'movie' END,
                'My edited title','My edited overview',
                CASE WHEN id % 2 = 0 THEN 'error' ELSE 'fresh' END,
                100,1,'Original error',101 FROM media_items WHERE id = ?",
                )
                .bind::<diesel::sql_types::Integer, _>(id)
                .execute(conn)
                .unwrap();
            }
            let before = existing_scan_metadata(conn, &ids);
            (ids, before)
        })
        .await;
    for add_files in [false, true] {
        if add_files {
            let new_album = root.join("Artist/Second Album");
            std::fs::create_dir_all(&new_album).unwrap();
            for number in 1..=10 {
                std::fs::write(new_album.join(format!("{number:02} New.flac")), b"audio").unwrap();
            }
            std::fs::write(root.join("Second movie.mkv"), b"video").unwrap();
            std::fs::write(season.join("Show - S01E02 - New.mkv"), b"video").unwrap();
            std::fs::write(root.join("Second photo.jpg"), b"image").unwrap();
            std::fs::write(root.join("Second book.epub"), b"book").unwrap();
            // Even a changed existing file should keep its linked metadata.
            std::fs::write(root.join("Movie (2020).mkv"), b"changed video").unwrap();
        }
        if add_files {
            assert!(begin_library_metadata_refresh(FIRST).await);
        }
        for id in IDS {
            let scan_db = DbConn::get_one(&rocket).await.unwrap();
            start_library_scan(
                scan_db,
                settings.clone(),
                id,
                "Library".into(),
                "manual_library_scan",
            )
            .await;
        }
        if add_files {
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    let count = db
                        .run(|conn| list_media_items(conn, Some(FIRST)).unwrap().len())
                        .await;
                    if count > 12 {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            finish_library_metadata_refresh(FIRST).await;
        }
        wait_for_library_scans_and_metadata(&IDS).await;
        let original_ids = old_ids.clone();
        let (after, new_items) = db
            .run(move |conn| {
                let after = existing_scan_metadata(conn, &original_ids);
                let new_items = list_media_items(conn, None)
                    .unwrap()
                    .into_iter()
                    .filter(|item| !original_ids.contains(&item.id))
                    .collect::<Vec<_>>();
                (after, new_items)
            })
            .await;
        assert_eq!(
            before, after,
            "scan must leave existing metadata and attempts unchanged"
        );
        assert_eq!(!new_items.is_empty(), add_files);
        if add_files {
            let new_ids = new_items.iter().map(|item| item.id).collect::<Vec<_>>();
            let attempts = db
                .run(move |conn| existing_scan_metadata(conn, &new_ids).0)
                .await;
            for id in [FIRST, FIRST + 1] {
                let eligible = new_items
                    .iter()
                    .filter(|item| item.library_id == id)
                    .map(|item| item.id)
                    .collect::<Vec<_>>();
                assert!(!eligible.is_empty());
                assert!(
                    attempts
                        .iter()
                        .filter(|(id, _)| eligible.contains(id))
                        .all(|(_, attempted)| attempted.is_some_and(|timestamp| timestamp > 123))
                );
            }
            assert_eq!(
                new_items
                    .iter()
                    .filter(|item| item.library_id == FIRST)
                    .count(),
                11
            );
        }
    }
    drop(db);
    drop(rocket);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn cancelled_parent_cannot_claim_descendant_activity_ownership() {
    const CANCELLED: i32 = 196_000_001;
    let (activity_id, _) = register_metadata_refresh_activity(
        "item",
        "automatic_match",
        "Cancelled".into(),
        None,
        Some(CANCELLED),
        vec![target(CANCELLED, "show")],
    )
    .await
    .unwrap();
    cancel_metadata_refresh_activity(&activity_id).await;
    assert!(
        extend_metadata_refresh_activity(&activity_id, vec![target(CANCELLED, "season")])
            .await
            .is_empty()
    );
    assert!(
        !ACTIVE_METADATA_REFRESH_ITEMS
            .read()
            .await
            .contains_key(&CANCELLED)
    );
}

#[tokio::test]
async fn refreshing_tracks_keep_album_and_artist_busy_without_inflating_progress() {
    const ARTIST: i32 = 198_000_001;
    const ALBUM: i32 = ARTIST + 1;
    const TRACK: i32 = ARTIST + 2;
    let rocket = rocket::custom(rocket::Config::figment().merge((
        "databases.sqlite_db",
        serde_json::json!({"url": ":memory:", "pool_size": 1}),
    )))
    .attach(DbConn::fairing())
    .ignite()
    .await
    .unwrap();
    let db = DbConn::get_one(&rocket).await.unwrap();
    db.run(|conn| {
        crate::db::run_pending_sqlite_migrations(conn).unwrap();
        conn.batch_execute(
            "INSERT INTO media_libraries (id,name,path,kind) VALUES \
             (198000001,'Music','music','music');
            INSERT INTO media_items (id,library_id,parent_id,identity_key,item_type,display_title) \
             VALUES
            (198000001,198000001,NULL,'artist','artist','Artist'),
            (198000002,198000001,198000001,'album','album','Album'),
            (198000003,198000001,198000002,'track','track','Song');
            INSERT INTO item_metadata_links \
             (media_item_id,provider_id,external_id,media_type,match_state) VALUES
            (198000001,'musicbrainz','f5093c06-23e3-404f-aeaa-40f72885ee3a','artist','matched'),
            \
             (198000002,'musicbrainz','d817f725-66c3-366d-8dd1-b22d49c8a766','release-group','\
             matched');",
        )
        .unwrap();
    })
    .await;
    db.run(|conn| {
        use crate::db::schema::item_metadata_links::dsl as links;
        use diesel::{
            ExpressionMethods,
            QueryDsl,
            RunQueryDsl,
        };
        diesel::update(
            links::item_metadata_links.filter(links::media_item_id.eq_any([ARTIST, ALBUM])),
        )
        .set(links::last_refreshed_at.eq(Some(1_i64)))
        .execute(conn)
        .unwrap();
        let context = music_match_context(conn, TRACK).unwrap();
        assert_eq!(
            context.artist_id.as_deref(),
            Some("f5093c06-23e3-404f-aeaa-40f72885ee3a")
        );
        assert_eq!(
            context.release_group_id.as_deref(),
            Some("d817f725-66c3-366d-8dd1-b22d49c8a766")
        );
    })
    .await;
    let (activity_id, _) = register_metadata_refresh_activity_for_items(
        "library",
        "automatic_match",
        "Music".into(),
        Some(ARTIST),
        None,
        None,
        [ARTIST, ALBUM, TRACK]
            .map(|id| (id, MetadataRevision::current(id)))
            .to_vec(),
    )
    .await
    .unwrap();
    for id in [ARTIST, ALBUM] {
        record_metadata_refresh_activity_progress(&activity_id, id, false).await;
    }
    let activity = system_activities_with_ancestors(&db)
        .await
        .unwrap()
        .into_iter()
        .find(|activity| activity.id == activity_id)
        .unwrap();
    assert_eq!(activity.total_items, 3);
    assert_eq!(activity.completed_items, 2);
    assert!(activity.item_ids.contains(&ARTIST));
    assert!(activity.item_ids.contains(&ALBUM));
    assert!(activity.item_ids.contains(&TRACK));
    record_metadata_refresh_activity_progress(&activity_id, TRACK, false).await;
    let activity = system_activities_with_ancestors(&db)
        .await
        .unwrap()
        .into_iter()
        .find(|activity| activity.id == activity_id)
        .unwrap();
    assert!(activity.item_ids.is_empty());
    complete_metadata_refresh_activity(&activity_id).await;
}

#[test]
fn library_selected_musicbrainz_is_available_with_legacy_global_flag_off() {
    let mut settings = Settings::default();
    settings
        .metadata
        .providers
        .iter_mut()
        .find(|provider| provider.id == MetadataProviderId::Fanart)
        .unwrap()
        .api_key = Some("personal-test-key".into());
    let musicbrainz = settings
        .metadata
        .providers
        .iter()
        .find(|provider| provider.id == MetadataProviderId::MusicBrainz)
        .unwrap();
    assert!(!musicbrainz.enabled);
    let available = available_automatic_metadata_providers(&settings);
    assert!(available.contains(&MetadataProviderId::MusicBrainz));
    assert!(!available.contains(&MetadataProviderId::Tmdb));
    assert!(!available.contains(&MetadataProviderId::OpenLibrary));
    assert!(!available.contains(&MetadataProviderId::Fanart));
}

#[tokio::test]
async fn unmatched_albums_are_processed_before_already_linked_tracks() {
    const ARTIST: i32 = 199_000_001;
    const ALBUM: i32 = ARTIST + 1;
    const TRACK: i32 = ARTIST + 2;
    let rocket = rocket::custom(rocket::Config::figment().merge((
        "databases.sqlite_db",
        serde_json::json!({"url": ":memory:", "pool_size": 1}),
    )))
    .attach(DbConn::fairing())
    .ignite()
    .await
    .unwrap();
    let db = DbConn::get_one(&rocket).await.unwrap();
    db.run(|conn| {
        crate::db::run_pending_sqlite_migrations(conn).unwrap();
        conn.batch_execute(
            "INSERT INTO media_libraries (id,name,path,kind)
            VALUES (199000001,'Music','music','music');
            INSERT INTO media_items
            (id,library_id,parent_id,identity_key,item_type,display_title,modified_at) VALUES
            (199000001,199000001,NULL,'artist','artist','Artist',1),
            (199000002,199000001,199000001,'album','album','Album',2),
            (199000003,199000001,199000002,'track','track','Song',999);
            INSERT INTO music_item_metadata (media_item_id,artist,album_artist,album) VALUES
            (199000001,'Artist','Artist',''),
            (199000002,'Artist','Artist','Album'),
            (199000003,'Artist','Artist','Album');
            INSERT INTO item_metadata_links
            (media_item_id,provider_id,external_id,media_type,match_state) VALUES
            (199000001,'musicbrainz','46510d46-ce20-4dde-b9ea-207e474353f4','artist','matched'),
            (199000003,'musicbrainz','f5093c06-23e3-404f-aeaa-40f72885ee3a',
             'recording','matched');",
        )
        .unwrap();
    })
    .await;
    let roots = load_library_refresh_targets(&db, ARTIST).await.unwrap();
    let mut work = roots
        .into_iter()
        .map(AutomaticMetadataWork::Refresh)
        .collect::<Vec<_>>();
    let (items, candidates) = db
        .run(|conn| {
            let candidates = metadata_jobs::load(
                conn,
                |conn| list_automatic_metadata_refresh_candidates(conn, Some(ARTIST), usize::MAX),
                |candidate| candidate.item_id,
            )
            .unwrap();
            (list_media_items(conn, Some(ARTIST)).unwrap(), candidates)
        })
        .await;
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].0.item_id, ALBUM);
    work.extend(
        candidates
            .into_iter()
            .map(|(candidate, revision)| AutomaticMetadataWork::Match(candidate, revision)),
    );
    order_music_metadata_work(&items, &mut work);
    assert_eq!(
        work.iter()
            .map(AutomaticMetadataWork::item_id)
            .collect::<Vec<_>>(),
        vec![ARTIST, ALBUM, TRACK]
    );
}
