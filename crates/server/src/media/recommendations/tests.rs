//! Recommendation regressions use a migrated database and typed application fixtures.

use super::*;
use crate::db::schema::{
    item_similarities,
    media_items as item_rows,
    media_libraries as libraries,
    users,
};
use crate::metadata::{
    StoredMetadataSnapshot,
    upsert_item_metadata_snapshot,
};
use diesel::Connection;
use serde_json::json;

pub(crate) fn database() -> SqliteConnection {
    database_at(":memory:")
}

fn database_at(path: &str) -> SqliteConnection {
    let mut conn = SqliteConnection::establish(path).unwrap();
    crate::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    crate::db::configure_sqlite_connection(&mut conn).unwrap();
    for (id, admin) in [
        (1, true),
        (2, false),
        (3, false),
    ] {
        diesel::insert_into(users::table)
            .values((
                users::id.eq(id),
                users::username.eq(format!("user{id}")),
                users::password.eq("hash"),
                users::admin.eq(admin),
            ))
            .execute(&mut conn)
            .unwrap();
    }
    for (id, allowed) in [
        (1, "[]"),
        (2, "[3]"),
        (3, "[2]"),
    ] {
        diesel::insert_into(libraries::table)
            .values((
                libraries::id.eq(id),
                libraries::name.eq(format!("library{id}")),
                libraries::path.eq(format!("library{id}")),
                libraries::kind.eq("movies"),
                libraries::allowed_user_ids_json.eq(allowed),
            ))
            .execute(&mut conn)
            .unwrap();
    }
    conn
}

fn item(
    conn: &mut SqliteConnection,
    id: i32,
    library: i32,
    parent: Option<i32>,
    kind: &str,
) {
    diesel::insert_into(item_rows::table)
        .values((
            item_rows::id.eq(id),
            item_rows::library_id.eq(library),
            item_rows::parent_id.eq(parent),
            item_rows::identity_key.eq(format!("item{id}")),
            item_rows::item_type.eq(kind),
            item_rows::display_title.eq(format!("Item {id}")),
        ))
        .execute(conn)
        .unwrap();
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
        title: Some(format!("Title {id}")),
        overview: None,
        artwork_url: None,
        backdrop_url: None,
        release_year: None,
        locale_key: "en-US".into(),
        provider_locale_key: Some("en-US".into()),
        provider_payload_json: Some(payload.to_string()),
    }
}

fn link(
    conn: &mut SqliteConnection,
    id: i32,
    kind: &str,
    external_id: &str,
    payload: Value,
) {
    let provider = if [
        "artist",
        "release",
        "recording",
    ]
    .contains(&kind)
    {
        MetadataProviderId::MusicBrainz
    } else {
        MetadataProviderId::Tmdb
    };
    upsert_item_metadata_snapshot(conn, id, &snapshot(provider, kind, external_id, payload))
        .unwrap();
}

fn recommendations(
    conn: &mut SqliteConnection,
    id: i32,
    user: i32,
) -> Vec<i32> {
    list(conn, Some(user), id, &["en-US".into()])
        .unwrap()
        .into_iter()
        .map(|item| item.id)
        .collect()
}

#[test]
fn saved_results_match_later_additions_and_exclude_private_wrong_type_or_missing_items() {
    let mut conn = database();
    item(&mut conn, 10, 1, None, "movie");
    item(&mut conn, 20, 1, None, "movie");
    item(&mut conn, 30, 2, None, "movie");
    item(&mut conn, 40, 1, None, "show");
    item(&mut conn, 50, 1, None, "movie");
    link(
        &mut conn,
        10,
        "movie",
        "10",
        json!({"genres":[{"id":1,"name":"Fantasy"}], "koko_similar_items":[
            {"provider_id":"tmdb","media_type":"movie","external_id":"30","rank":0},
            {"provider_id":"tmdb","media_type":"movie","external_id":"60","rank":1},
            {"provider_id":"tmdb","media_type":"movie","external_id":"40","rank":2},
            {"provider_id":"tmdb","media_type":"movie","external_id":"50","rank":3}
        ]}),
    );
    for id in [20, 30, 40, 50] {
        link(
            &mut conn,
            id,
            if id == 40 { "tv" } else { "movie" },
            &id.to_string(),
            json!({"genres":[{"id":1,"name":"Fantasy"}]}),
        );
    }
    diesel::update(item_rows::table.find(50))
        .set(item_rows::missing_since.eq(Some(100_i64)))
        .execute(&mut conn)
        .unwrap();
    assert_eq!(
        item_similarities::table
            .count()
            .get_result::<i64>(&mut conn)
            .unwrap(),
        4
    );
    assert_eq!(recommendations(&mut conn, 10, 2), [20]);
    assert!(list(&mut conn, Some(2), 30, &[]).is_err());
    item(&mut conn, 60, 3, None, "movie");
    link(&mut conn, 60, "movie", "60", json!({}));
    assert_eq!(recommendations(&mut conn, 10, 2), [60, 20]);
    assert_eq!(recommendations(&mut conn, 10, 1), [30, 60, 20]);
    diesel::update(libraries::table.find(3))
        .set(libraries::allowed_user_ids_json.eq("[3]"))
        .execute(&mut conn)
        .unwrap();
    assert_eq!(recommendations(&mut conn, 10, 2), [20]);
}

#[test]
fn music_matches_saved_artist_ids_for_artists_and_albums_without_recommending_tracks() {
    let mut conn = database();
    let a = "00000000-0000-0000-0000-000000000001";
    let b = "00000000-0000-0000-0000-000000000002";
    let c = "00000000-0000-0000-0000-000000000003";
    let d = "00000000-0000-0000-0000-000000000004";
    for (id, parent, kind, external, provider_kind) in [
        (10, None, "artist", a, "artist"),
        (11, Some(10), "album", a, "release"),
        (12, Some(11), "track", c, "recording"),
        (20, None, "artist", b, "artist"),
        (21, Some(20), "album", b, "release"),
        (22, Some(21), "track", d, "recording"),
    ] {
        item(&mut conn, id, 1, parent, kind);
        link(&mut conn, id, provider_kind, external, json!({}));
    }
    link(
        &mut conn,
        10,
        "artist",
        a,
        json!({"koko_similar_items": [{
            "provider_id": "musicbrainz", "media_type": "artist", "external_id": b, "rank": 0
        }]}),
    );
    link(
        &mut conn,
        12,
        "recording",
        c,
        json!({"koko_similar_items": [{
            "provider_id": "musicbrainz", "media_type": "recording", "external_id": d, "rank": 5
        }]}),
    );
    assert_eq!(recommendations(&mut conn, 10, 2), [20]);
    assert_eq!(recommendations(&mut conn, 11, 2), [21]);
    assert!(recommendations(&mut conn, 12, 2).is_empty());
    item(&mut conn, 23, 1, Some(21), "track");
    link(&mut conn, 23, "recording", b, json!({}));
    assert!(recommendations(&mut conn, 12, 2).is_empty());
    // A matched album can carry artist IDs even when no parent artist was linked.
    diesel::delete(links::table.filter(links::media_item_id.eq(20)))
        .execute(&mut conn)
        .unwrap();
    let album_link = links::table
        .filter(links::media_item_id.eq(21))
        .select(links::id)
        .first::<i32>(&mut conn)
        .unwrap();
    diesel::insert_into(external::table)
        .values((
            external::metadata_link_id.eq(album_link),
            external::source.eq("musicbrainz_artist"),
            external::external_id.eq(b),
        ))
        .execute(&mut conn)
        .unwrap();
    assert_eq!(recommendations(&mut conn, 11, 2), [21]);
}

#[test]
fn track_genres_support_parent_matches_but_track_refreshes_do_not_rebuild_them() {
    let mut conn = database();
    for (id, parent, kind) in [
        (10, None, "artist"),
        (11, Some(10), "album"),
        (12, Some(11), "track"),
        (20, None, "artist"),
        (21, Some(20), "album"),
        (22, Some(21), "track"),
    ] {
        item(&mut conn, id, 1, parent, kind);
    }
    for (id, mbid) in [
        (12, "00000000-0000-0000-0000-000000000001"),
        (22, "00000000-0000-0000-0000-000000000002"),
    ] {
        link(&mut conn, id, "recording", mbid, json!({}));
        diesel::update(links::table.filter(links::media_item_id.eq(id)))
            .set(links::genres_json.eq("[\"Fantasy\"]"))
            .execute(&mut conn)
            .unwrap();
    }
    for id in 100..200 {
        item(
            &mut conn,
            id,
            1,
            Some(if id % 2 == 0 { 11 } else { 21 }),
            "track",
        );
    }
    refresh(&mut conn, &[11, 21]).unwrap();
    assert_eq!(recommendations(&mut conn, 10, 2), [20]);
    assert_eq!(recommendations(&mut conn, 11, 2), [21]);
    // Only two artist and two album matches are ranked, regardless of their song counts.
    assert_eq!(
        cached::table.count().get_result::<i64>(&mut conn).unwrap(),
        4
    );
    for (seed, candidate) in [(12, 22), (22, 12)] {
        diesel::insert_into(cached::table)
            .values((
                cached::item_id.eq(seed),
                cached::similar_item_id.eq(candidate),
                cached::provider_priority.eq(0),
                cached::provider_rank.eq(0),
                cached::local_score.eq(10),
                cached::year_distance.eq(0_i64),
            ))
            .execute(&mut conn)
            .unwrap();
        assert!(recommendations(&mut conn, seed, 2).is_empty());
    }
    diesel::update(links::table.filter(links::media_item_id.eq(22)))
        .set(links::genres_json.eq("[\"Comedy\"]"))
        .execute(&mut conn)
        .unwrap();
    refresh(&mut conn, &[12, 22]).unwrap();
    assert_eq!(
        cached::table.count().get_result::<i64>(&mut conn).unwrap(),
        4
    );
    assert_eq!(recommendations(&mut conn, 11, 2), [21]);
    refresh(&mut conn, &[21]).unwrap();
    assert!(recommendations(&mut conn, 11, 2).is_empty());
}

#[test]
fn books_use_inherited_subjects_and_authors_and_keep_works_editions_and_recordings_separate() {
    let mut conn = database();
    for (id, parent, kind) in [
        (1, None, "book_author"),
        (2, Some(1), "book_work"),
        (3, Some(2), "book"),
        (4, Some(2), "audiobook"),
        (5, Some(1), "book_work"),
        (6, Some(5), "book"),
        (7, None, "book_author"),
        (8, Some(7), "book_work"),
        (9, Some(8), "book"),
    ] {
        item(&mut conn, id, 1, parent, kind);
    }
    for (id, genre) in [
        (2, "Fantasy"),
        (5, "Science fiction"),
        (8, "Fantasy"),
    ] {
        link(&mut conn, id, "book", &id.to_string(), json!({}));
        diesel::update(links::table.filter(links::media_item_id.eq(id)))
            .set(links::genres_json.eq(json!([genre]).to_string()))
            .execute(&mut conn)
            .unwrap();
    }
    refresh(&mut conn, &[2, 5, 8]).unwrap();
    assert_eq!(recommendations(&mut conn, 3, 2), [6, 9]);
    assert_eq!(recommendations(&mut conn, 2, 2), [5, 8]);
    assert!(recommendations(&mut conn, 4, 2).is_empty());
    // Same-author fallback still works when a work has no provider subject metadata.
    diesel::delete(links::table.filter(links::media_item_id.eq(5)))
        .execute(&mut conn)
        .unwrap();
    assert!(recommendations(&mut conn, 3, 2).contains(&6));
}

#[test]
fn recommendations_are_bounded_deduplicated_and_empty_without_meaningful_matches() {
    let mut conn = database();
    item(&mut conn, 1, 1, None, "movie");
    item(&mut conn, 2, 1, None, "movie");
    assert!(recommendations(&mut conn, 1, 2).is_empty());
    for id in 1..=20 {
        if id > 2 {
            item(&mut conn, id, 1, None, "movie");
        }
        link(
            &mut conn,
            id,
            "movie",
            &if id == 2 { 1 } else { id }.to_string(),
            json!({"genres":[{"id":1,"name":"Fantasy"}]}),
        );
    }
    let result = recommendations(&mut conn, 1, 2);
    assert_eq!(result.len(), LIMIT);
    assert!(!result.contains(&1));
    assert!(!result.contains(&2));
    diesel::update(item_rows::table.find(result[0]))
        .set(item_rows::deleted_at.eq(Some(100_i64)))
        .execute(&mut conn)
        .unwrap();
    assert!(!recommendations(&mut conn, 1, 2).contains(&result[0]));
}

#[test]
fn local_matches_are_persisted_and_reads_do_not_recalculate_changed_metadata() {
    let mut conn = database();
    for (id, genre) in [
        (1, "Fantasy"),
        (2, "Fantasy"),
        (3, "Comedy"),
    ] {
        item(&mut conn, id, 1, None, "movie");
        link(
            &mut conn,
            id,
            "movie",
            &id.to_string(),
            json!({"genres":[{"name":genre}]}),
        );
    }
    assert_eq!(recommendations(&mut conn, 1, 2), [2]);
    assert_eq!(
        cached::table
            .filter(cached::item_id.eq(1))
            .select(cached::similar_item_id)
            .load::<i32>(&mut conn)
            .unwrap(),
        [2]
    );
    for (id, genre) in [(2, "Comedy"), (3, "Fantasy")] {
        diesel::update(links::table.filter(links::media_item_id.eq(id)))
            .set(links::genres_json.eq(json!([genre]).to_string()))
            .execute(&mut conn)
            .unwrap();
    }
    // Displaying an item is a read of the last snapshot, not a lazy fallback rebuild.
    assert_eq!(recommendations(&mut conn, 1, 2), [2]);
    refresh(&mut conn, &[2, 3]).unwrap();
    assert_eq!(recommendations(&mut conn, 1, 2), [3]);
    crate::metadata::clear_item_metadata_tree(&mut conn, 3, true).unwrap();
    assert!(recommendations(&mut conn, 1, 2).is_empty());
}

#[test]
fn cached_local_matches_filter_access_and_current_type_before_the_display_limit() {
    let mut conn = database();
    for id in 1..=28 {
        item(
            &mut conn,
            id,
            if (2..=15).contains(&id) { 2 } else { 1 },
            None,
            "movie",
        );
        link(
            &mut conn,
            id,
            "movie",
            &id.to_string(),
            json!({"genres":[{"name":"Fantasy"}]}),
        );
    }
    assert_eq!(
        cached::table
            .filter(cached::item_id.eq(1))
            .count()
            .get_result::<i64>(&mut conn)
            .unwrap(),
        27
    );
    let visible = recommendations(&mut conn, 1, 2);
    assert_eq!(visible.len(), LIMIT);
    assert!(visible.iter().all(|id| *id >= 16));
    diesel::update(item_rows::table.find(visible[0]))
        .set(item_rows::item_type.eq("show"))
        .execute(&mut conn)
        .unwrap();
    let changed = recommendations(&mut conn, 1, 2);
    assert_eq!(changed.len(), LIMIT);
    assert!(!changed.contains(&visible[0]));
    // A newly granted library takes effect without changing the cached matches.
    diesel::update(libraries::table.find(2))
        .set(libraries::allowed_user_ids_json.eq("[2]"))
        .execute(&mut conn)
        .unwrap();
    assert!(
        recommendations(&mut conn, 1, 2)
            .iter()
            .any(|id| (2..=15).contains(id))
    );
}

#[test]
fn page_query_work_does_not_grow_with_unrelated_catalog_items() {
    use diesel::connection::InstrumentationEvent;
    use std::sync::{
        Arc,
        atomic::{
            AtomicUsize,
            Ordering,
        },
    };
    let mut conn = database();
    for id in [1, 2] {
        item(&mut conn, id, 1, None, "movie");
        link(
            &mut conn,
            id,
            "movie",
            &id.to_string(),
            json!({"genres":[{"name":"Fantasy"}]}),
        );
    }
    let queries = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&queries);
    conn.set_instrumentation(move |event: InstrumentationEvent<'_>| {
        if matches!(event, InstrumentationEvent::StartQuery { .. }) {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    });
    assert_eq!(recommendations(&mut conn, 1, 2), [2]);
    let small_catalog_queries = queries.swap(0, Ordering::SeqCst);
    for id in 3..=603 {
        item(&mut conn, id, 1, None, "movie");
    }
    queries.store(0, Ordering::SeqCst);
    assert_eq!(recommendations(&mut conn, 1, 2), [2]);
    assert_eq!(queries.load(Ordering::SeqCst), small_catalog_queries);
}

#[test]
fn combined_migration_preserves_metadata_links_and_deleting_items_cascades_cached_matches() {
    use diesel::connection::SimpleConnection;
    let mut conn = database();
    for id in [1, 2] {
        item(&mut conn, id, 1, None, "movie");
        link(
            &mut conn,
            id,
            "movie",
            &id.to_string(),
            json!({"genres":[{"name":"Fantasy"}]}),
        );
    }
    link(
        &mut conn,
        1,
        "movie",
        "1",
        json!({"genres":[{"name":"Fantasy"}],"koko_similar_items":[
            {"provider_id":"tmdb","media_type":"movie","external_id":"99","rank":0}
        ]}),
    );
    conn.batch_execute(include_str!(
        "../../../sql/migrations/54af66a9dbe7_item_similarities/down.sql"
    ))
    .unwrap();
    conn.batch_execute(include_str!(
        "../../../sql/migrations/54af66a9dbe7_item_similarities/up.sql"
    ))
    .unwrap();
    assert_eq!(
        item_similarities::table
            .count()
            .get_result::<i64>(&mut conn)
            .unwrap(),
        0
    );
    assert_eq!(
        links::table.count().get_result::<i64>(&mut conn).unwrap(),
        2
    );
    refresh(&mut conn, &[1]).unwrap();
    assert!(!recommendations(&mut conn, 1, 2).is_empty());
    diesel::delete(item_rows::table.find(2))
        .execute(&mut conn)
        .unwrap();
    assert_eq!(
        cached::table.count().get_result::<i64>(&mut conn).unwrap(),
        0
    );
}

#[test]
#[ignore = "requires a disposable KOKO_PERF_DB catalog snapshot"]
fn profile_saved_catalog_requests() {
    use std::time::Instant;
    let path =
        std::env::var("KOKO_PERF_DB").expect("set KOKO_PERF_DB to a disposable database copy");
    let mut conn = SqliteConnection::establish(&path).unwrap();
    let admin = users::table
        .filter(users::admin.eq(true))
        .select(users::id)
        .first::<i32>(&mut conn)
        .unwrap();
    let languages = ["en-US".into()];
    let start = Instant::now();
    let home =
        get_media_home_with_preferred_languages(&mut conn, Some(admin), None, &languages).unwrap();
    println!(
        "all_libraries home_ms={:.2} shelves={}",
        start.elapsed().as_secs_f64() * 1000.0,
        home.shelves.len()
    );
    let library_ids = if let Ok(ids) = std::env::var("KOKO_PERF_LIBRARIES") {
        ids.split(',')
            .map(|id| id.trim().parse::<i32>().expect("numeric library IDs"))
            .collect()
    } else {
        libraries::table
            .filter(libraries::kind.eq("movies"))
            .select(libraries::id)
            .load::<i32>(&mut conn)
            .unwrap()
    };
    for library in library_ids {
        let Some(id) = item_rows::table
            .filter(item_rows::library_id.eq(library))
            .filter(item_rows::item_type.eq("movie"))
            .filter(item_rows::deleted_at.is_null())
            .select(item_rows::id)
            .first::<i32>(&mut conn)
            .optional()
            .unwrap()
        else {
            continue;
        };
        let start = Instant::now();
        let mut detail = get_media_item_with_preferred_languages(&mut conn, id, "", &languages)
            .unwrap()
            .unwrap();
        apply_user_playback_context_to_detail(&mut conn, Some(admin), &mut detail).unwrap();
        println!(
            "library={library} item={id} details_ms={:.2}",
            start.elapsed().as_secs_f64() * 1000.0
        );
        let start = Instant::now();
        let matches = crate::metadata::get_item_metadata_summaries(&mut conn, id).unwrap();
        println!(
            "library={library} metadata_ms={:.2} matches={}",
            start.elapsed().as_secs_f64() * 1000.0,
            matches.len()
        );
        let start = Instant::now();
        let items = list_media_items_for_user_with_preferred_languages(
            &mut conn,
            Some(admin),
            Some(library),
            &languages,
        )
        .unwrap();
        println!(
            "library={library} items_ms={:.2} count={}",
            start.elapsed().as_secs_f64() * 1000.0,
            items.len()
        );
        let start = Instant::now();
        get_media_home_with_preferred_languages(&mut conn, Some(admin), Some(library), &languages)
            .unwrap();
        println!(
            "library={library} home_ms={:.2}",
            start.elapsed().as_secs_f64() * 1000.0
        );
        let start = Instant::now();
        list(&mut conn, Some(admin), id, &languages).unwrap();
        println!(
            "library={library} similar_ms={:.2}",
            start.elapsed().as_secs_f64() * 1000.0
        );
        let start = Instant::now();
        refresh(&mut conn, &[id]).unwrap();
        println!(
            "library={library} fallback_refresh_ms={:.2}",
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
}

#[test]
fn movie_refresh_retains_cross_library_matches_without_loading_unrelated_types() {
    use diesel::connection::InstrumentationEvent;
    use std::sync::{
        Arc,
        atomic::{
            AtomicUsize,
            Ordering,
        },
    };
    let mut conn = database();
    for (id, library) in [(1, 1), (2, 3)] {
        item(&mut conn, id, library, None, "movie");
        link(
            &mut conn,
            id,
            "movie",
            &id.to_string(),
            json!({"genres":[{"name":"Fantasy"}]}),
        );
    }
    let queries = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&queries);
    conn.set_instrumentation(move |event: InstrumentationEvent<'_>| {
        if matches!(event, InstrumentationEvent::StartQuery { .. }) {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    });
    refresh(&mut conn, &[1]).unwrap();
    let small_catalog_queries = queries.load(Ordering::SeqCst);
    for id in 3..=603 {
        item(&mut conn, id, 1, None, "track");
    }
    queries.store(0, Ordering::SeqCst);
    refresh(&mut conn, &[1]).unwrap();
    assert_eq!(queries.load(Ordering::SeqCst), small_catalog_queries);
    assert_eq!(recommendations(&mut conn, 1, 2), [2]);
}

#[test]
fn show_refresh_retains_provider_and_local_matches_without_loading_tv_children() {
    use diesel::connection::InstrumentationEvent;
    use std::sync::{
        Arc,
        atomic::{
            AtomicUsize,
            Ordering,
        },
    };
    let mut conn = database();
    for (id, library, genre) in [
        (1, 1, "Fantasy"),
        (2, 3, "Comedy"),
        (3, 1, "Fantasy"),
    ] {
        item(&mut conn, id, library, None, "show");
        link(
            &mut conn,
            id,
            "tv",
            &id.to_string(),
            json!({"genres":[{"name":genre}], "koko_similar_items": if id == 1 {
                json!([{"provider_id":"tmdb", "media_type":"tv", "external_id":"2", "rank":0}])
            } else { json!([]) }}),
        );
    }
    assert_eq!(recommendations(&mut conn, 1, 2), [2, 3]);
    let queries = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&queries);
    conn.set_instrumentation(move |event: InstrumentationEvent<'_>| {
        if matches!(event, InstrumentationEvent::StartQuery { .. }) {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    });
    refresh(&mut conn, &[1]).unwrap();
    let show_only_queries = queries.load(Ordering::SeqCst);
    let mut changed_ids = vec![1];
    for id in 100..400 {
        let parent = if id % 2 == 0 { 1 } else { 3 };
        item(&mut conn, id, 1, Some(parent), "season");
        item(&mut conn, id + 1000, 1, Some(id), "episode");
        changed_ids.extend([id, id + 1000]);
    }
    queries.store(0, Ordering::SeqCst);
    refresh(&mut conn, &[1]).unwrap();
    assert_eq!(queries.load(Ordering::SeqCst), show_only_queries);
    // A scan can report shows and their children in the same batch.
    refresh(&mut conn, &changed_ids).unwrap();
    assert_eq!(recommendations(&mut conn, 1, 2), [2, 3]);
    let cached_ids = cached::table
        .select((cached::item_id, cached::similar_item_id))
        .load::<(i32, i32)>(&mut conn)
        .unwrap();
    assert!(!cached_ids.is_empty());
    assert!(
        cached_ids
            .iter()
            .all(|(seed, candidate)| *seed < 100 && *candidate < 100)
    );
}

#[test]
fn tv_child_refresh_discards_legacy_matches_without_rebuilding_show_matches() {
    let mut conn = database();
    for id in [1, 2] {
        item(&mut conn, id, 1, None, "show");
        item(&mut conn, id + 10, 1, Some(id), "season");
        item(&mut conn, id + 20, 1, Some(id + 10), "episode");
        link(
            &mut conn,
            id,
            "tv",
            &id.to_string(),
            json!({"genres":[{"name":"Fantasy"}]}),
        );
    }
    assert_eq!(recommendations(&mut conn, 1, 2), [2]);
    for (seed, candidate) in [
        (11, 12),
        (12, 11),
        (21, 22),
        (22, 21),
    ] {
        diesel::insert_into(cached::table)
            .values((
                cached::item_id.eq(seed),
                cached::similar_item_id.eq(candidate),
                cached::provider_priority.eq(0),
                cached::provider_rank.eq(0),
                cached::local_score.eq(10),
                cached::year_distance.eq(0_i64),
            ))
            .execute(&mut conn)
            .unwrap();
        assert!(recommendations(&mut conn, seed, 2).is_empty());
    }
    // Stale show facts make an accidental parent rebuild observable.
    diesel::update(links::table.filter(links::media_item_id.eq(2)))
        .set(links::genres_json.eq("[\"Comedy\"]"))
        .execute(&mut conn)
        .unwrap();
    refresh(&mut conn, &[11, 21]).unwrap();
    assert_eq!(recommendations(&mut conn, 1, 2), [2]);
    assert_eq!(
        cached::table.count().get_result::<i64>(&mut conn).unwrap(),
        2
    );
    refresh(&mut conn, &[2]).unwrap();
    assert!(recommendations(&mut conn, 1, 2).is_empty());
}

#[test]
fn refresh_keeps_a_writable_snapshot_when_another_connection_attempts_a_write() {
    use diesel::connection::{
        InstrumentationEvent,
        SimpleConnection,
    };
    use std::sync::{
        Arc,
        atomic::{
            AtomicUsize,
            Ordering,
        },
    };
    let path = std::env::temp_dir().join(format!(
        "koko-recommendation-contention-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut conn = database_at(path.to_str().unwrap());
    for id in [1, 2] {
        item(&mut conn, id, 1, None, "movie");
        link(
            &mut conn,
            id,
            "movie",
            &id.to_string(),
            json!({"genres":[{"name":"Fantasy"}]}),
        );
    }
    let mut contender = SqliteConnection::establish(path.to_str().unwrap()).unwrap();
    crate::db::configure_sqlite_connection(&mut contender).unwrap();
    contender.batch_execute("PRAGMA busy_timeout = 0").unwrap();
    let outcome = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&outcome);
    let mut reads = 0;
    conn.set_instrumentation(move |event: InstrumentationEvent<'_>| {
        let InstrumentationEvent::StartQuery { query, .. } = event else {
            return;
        };
        if !query.to_string().starts_with("SELECT") {
            return;
        }
        reads += 1;
        if reads != 2 {
            return;
        }
        // The first ranking read has established the snapshot. An interleaved commit would make
        // a deferred WAL transaction fail when it later tries to write its matches.
        let result = diesel::update(item_rows::table.find(2))
            .set(item_rows::display_title.eq("Concurrent writer"))
            .execute(&mut contender);
        let state = match result {
            Ok(_) => 1,
            Err(diesel::result::Error::DatabaseError(_, info))
                if info.message().contains("locked") =>
            {
                2
            }
            other => panic!("Unexpected concurrent write result: {other:?}"),
        };
        observed.store(state, Ordering::SeqCst);
    });
    refresh(&mut conn, &[1]).unwrap();
    assert_eq!(outcome.load(Ordering::SeqCst), 2);
    conn.set_instrumentation(None::<fn(InstrumentationEvent<'_>)>);
    assert_eq!(recommendations(&mut conn, 1, 2), [2]);
    drop(conn);
    std::fs::remove_file(path).unwrap();
}
