//! Real scanner and typed database regressions for document identity, reading state, and access.

use super::*;
use crate::db::schema::{
    media_files,
    media_items,
    media_libraries,
    users,
};
use diesel::Connection;

pub(crate) struct Fixture(pub(crate) PathBuf);
impl Fixture {
    pub(crate) fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "koko-reader-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        for name in [
            "Author/Book/book.epub",
            "Author/Book/book.rtf",
            "Author/Book/book.html",
            "Comics/Series/Issue 2.cbz",
            "Comics/Series/Issue 10.cbr",
        ] {
            let path = root.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let contents: &[u8] = if name.ends_with(".html") {
                b"<script>alert('untrusted book')</script>"
            } else {
                b"original book contents"
            };
            std::fs::write(path, contents).unwrap();
        }
        Self(root)
    }

    pub(crate) fn seed(
        &self,
        conn: &mut SqliteConnection,
    ) -> i32 {
        crate::db::run_pending_sqlite_migrations(conn).unwrap();
        for (id, username) in [(1, "alice"), (2, "bob")] {
            diesel::insert_into(users::table)
                .values((
                    users::id.eq(id),
                    users::username.eq(username),
                    users::password.eq("hash"),
                    users::admin.eq(false),
                ))
                .execute(conn)
                .unwrap();
        }
        sync_library_catalog(
            conn,
            &[MediaLibrarySettings {
                name: "Books".into(),
                path: self.0.to_string_lossy().into(),
                kind: MediaLibraryKind::Books,
                recursive: true,
                ..Default::default()
            }],
            &FfmpegSettings {
                ffprobe_path: "missing-reader-ffprobe".into(),
                ..Default::default()
            },
        )
        .unwrap();
        list_media_items(conn, None)
            .unwrap()
            .into_iter()
            .find(|item| item.relative_path.ends_with("book.epub"))
            .unwrap()
            .id
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn database() -> SqliteConnection {
    SqliteConnection::establish(":memory:").unwrap()
}

#[test]
fn refreshed_curated_collections_group_books_instead_of_authors_or_file_formats() {
    use crate::metadata::{
        StoredMetadataSnapshot,
        list_metadata_collection_summaries,
        upsert_item_metadata_snapshot,
    };
    let fixture = Fixture::new();
    let second = fixture.0.join("Author/Second Book/second.epub");
    std::fs::create_dir_all(second.parent().unwrap()).unwrap();
    std::fs::write(&second, b"second book").unwrap();
    let mut conn = database();
    let first = fixture.seed(&mut conn);
    let items = list_media_items(&mut conn, None).unwrap();
    let other_format = items
        .iter()
        .find(|item| item.relative_path.ends_with("book.rtf"))
        .unwrap()
        .id;
    let second = items
        .iter()
        .find(|item| item.relative_path.ends_with("second.epub"))
        .unwrap()
        .id;
    let snapshot = |work: &str, included: bool| {
        let collections = if included {
            vec![serde_json::json!({"key":"/collections/favorites", "title":"Favorites"})]
        } else {
            vec![]
        };
        StoredMetadataSnapshot {
            provider_id: MetadataProviderId::OpenLibrary,
            external_id: work.into(),
            media_type: Some("book".into()),
            title: Some(work.into()),
            overview: None,
            artwork_url: None,
            backdrop_url: None,
            release_year: None,
            locale_key: String::new(),
            provider_locale_key: None,
            provider_payload_json: Some(
                serde_json::json!({"key":format!("/{work}"),"title":work,
                    "koko_collections": collections
                })
                .to_string(),
            ),
        }
    };
    for (id, work) in [
        (first, "works/OL1W"),
        (other_format, "works/OL1W"),
        (second, "works/OL2W"),
    ] {
        upsert_item_metadata_snapshot(&mut conn, id, &snapshot(work, true)).unwrap();
    }
    let collections = list_metadata_collection_summaries(&mut conn, None).unwrap();
    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].item_count, 2);
    assert!(collections[0].item_ids.iter().all(|id| {
        items
            .iter()
            .any(|item| item.id == *id && item.item_type == "book_work")
    }));
    let collection_id = collections[0].id.clone();
    upsert_item_metadata_snapshot(&mut conn, other_format, &snapshot("works/OL1W", false)).unwrap();
    let mut with_obsolete = snapshot("works/OL1W", true);
    let mut payload: serde_json::Value =
        serde_json::from_str(with_obsolete.provider_payload_json.as_deref().unwrap()).unwrap();
    payload["koko_collections"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "key":"/collections/obsolete", "title":"Old membership"
        }));
    with_obsolete.provider_payload_json = Some(payload.to_string());
    upsert_item_metadata_snapshot(&mut conn, first, &with_obsolete).unwrap();
    assert_eq!(
        list_metadata_collection_summaries(&mut conn, None)
            .unwrap()
            .len(),
        2
    );
    let mut partial = snapshot("works/OL1W", false);
    partial.provider_payload_json = Some(
        serde_json::json!({
            "key":"/works/OL1W", "title":"Book",
            "koko_unavailable_collections":["/collections/favorites"]
        })
        .to_string(),
    );
    upsert_item_metadata_snapshot(&mut conn, first, &partial).unwrap();
    let partial_collections = list_metadata_collection_summaries(&mut conn, None).unwrap();
    assert_eq!(partial_collections.len(), 1);
    assert_eq!(partial_collections[0].item_count, 2);
    let mut incomplete = snapshot("works/OL1W", false);
    incomplete.provider_payload_json = Some(
        serde_json::json!({
            "key":"/works/OL1W", "title":"Book", "koko_collections_incomplete":true
        })
        .to_string(),
    );
    upsert_item_metadata_snapshot(&mut conn, first, &incomplete).unwrap();
    assert_eq!(
        list_metadata_collection_summaries(&mut conn, None).unwrap()[0].item_count,
        2
    );
    for id in [first, other_format] {
        upsert_item_metadata_snapshot(&mut conn, id, &snapshot("works/OL1W", false)).unwrap();
    }
    let collections = list_metadata_collection_summaries(&mut conn, None).unwrap();
    assert_eq!(collections[0].id, collection_id);
    assert_eq!(collections[0].item_count, 1);
    upsert_item_metadata_snapshot(&mut conn, second, &snapshot("works/OL2W", false)).unwrap();
    assert!(
        list_metadata_collection_summaries(&mut conn, None)
            .unwrap()
            .is_empty()
    );
}
fn position(revision: String) -> ReadingProgress {
    ReadingProgress {
        revision,
        locator: "epubcfi(/6/2!/4/2:0)".into(),
        fraction: 0.35,
        completed: false,
        page_count: None,
        bookmarks: vec![Bookmark {
            title: "Chapter one".into(),
            locator: "epubcfi(/6/2)".into(),
        }],
        annotations: vec![ReadingAnnotation {
            id: "private-highlight".into(),
            locator: "epubcfi(/6/2!/4/2,:0,:12)".into(),
            quote: "Selected passage".into(),
            note: "My private thoughts".into(),
            color: "yellow".into(),
            rects: vec![],
        }],
    }
}

#[test]
fn reading_annotations_accept_legacy_state_and_bound_private_payloads() {
    let legacy = serde_json::json!({"revision":"v1", "locator":"2", "fraction":0.2, "completed":false, "bookmarks":[]});
    let legacy: ReadingProgress = serde_json::from_value(legacy).unwrap();
    assert!(legacy.valid());
    assert!(legacy.annotations.is_empty());
    let original = position("v1".into());
    let mut state = original.clone();
    state.annotations[0].quote.clear();
    assert!(state.valid(), "a page note needs no selected text");
    state.annotations[0].note.clear();
    assert!(!state.valid());
    state = original.clone();
    state.annotations.push(state.annotations[0].clone());
    assert!(
        !state.valid(),
        "annotation IDs must be unique within the account's book"
    );
    for (field, value) in [
        ("color", "url(javascript:bad)".into()),
        ("note", "x".repeat(16_001)),
        ("locator", String::new()),
    ] {
        let mut payload = serde_json::to_value(&original).unwrap();
        payload["annotations"][0][field] = serde_json::json!(value);
        assert!(
            !serde_json::from_value::<ReadingProgress>(payload)
                .unwrap()
                .valid()
        );
    }
    state = original;
    state.annotations[0].rects = vec![AnnotationRect {
        x: 0.1,
        y: 0.2,
        width: 0.3,
        height: 0.1,
    }];
    assert!(state.valid());
    state.annotations[0].rects[0].width = 1.0;
    assert!(!state.valid());
    state.annotations[0].rects[0].width = f64::NAN;
    assert!(!state.valid());
}

#[test]
fn scanning_recognizes_extended_formats_and_separates_comic_series_from_authors() {
    let fixture = Fixture::new();
    let mut conn = database();
    fixture.seed(&mut conn);
    let items = list_media_items(&mut conn, None).unwrap();
    assert_eq!(
        items
            .iter()
            .filter(|item| item.item_type == "comic_series")
            .count(),
        1
    );
    let series = items
        .iter()
        .find(|item| item.item_type == "comic_series")
        .unwrap();
    assert!(series.parent_id.is_none());
    assert_eq!(
        items
            .iter()
            .filter(|item| item.item_type == "comic_issue" && item.parent_id == Some(series.id))
            .count(),
        2
    );
    assert_eq!(
        items.iter().filter(|item| item.item_type == "book").count(),
        3
    );
    for extension in [
        "EPUB", "kepub", "PDF", "azw3", "azw4", "fb2", "fb2.zip", "cb7", "cbt", "docx", "lit",
        "rtf", "txtz", "djvu", "pdb", "htmlz",
    ] {
        assert!(
            is_book(Path::new(&format!("test.{extension}"))),
            "{extension}"
        );
    }
    for extension in ["exe", "mp3", "zip", "kfx"] {
        assert!(!is_book(Path::new(&format!("test.{extension}"))));
    }
}

#[test]
fn positions_and_bookmarks_are_per_user_and_invalidated_by_content_changes() {
    let fixture = Fixture::new();
    let mut conn = database();
    let id = fixture.seed(&mut conn);
    let revision = source(&mut conn, id, 1).unwrap().unwrap().document.revision;
    let state = position(revision);
    assert!(state.valid());
    save_progress(&mut conn, 1, id, &state).unwrap();
    assert_eq!(
        status(&mut conn, id, Some(1)).unwrap().unwrap().fraction,
        0.35
    );
    assert!(status(&mut conn, id, Some(2)).unwrap().is_none());
    let mut detail = get_media_item(&mut conn, id, "").unwrap().unwrap();
    apply_user_playback_context_to_detail(&mut conn, Some(1), &mut detail).unwrap();
    assert_eq!(detail.reading_progress.as_ref().unwrap().fraction, 0.35);
    let mut work = get_media_item(&mut conn, detail.parent_id.unwrap(), "")
        .unwrap()
        .unwrap();
    apply_user_playback_context_to_detail(&mut conn, Some(1), &mut work).unwrap();
    assert!(
        work.children
            .iter()
            .any(|child| child.id == id && child.reading_progress.is_some())
    );
    let saved = source(&mut conn, id, 1)
        .unwrap()
        .unwrap()
        .document
        .progress
        .unwrap();
    assert_eq!(saved.bookmarks[0].title, "Chapter one");
    assert_eq!(saved.annotations[0].note, "My private thoughts");
    assert_eq!(saved.locator, state.locator);
    assert!(
        source(&mut conn, id, 2)
            .unwrap()
            .unwrap()
            .document
            .progress
            .is_none()
    );
    let visible = list_media_items(&mut conn, None).unwrap();
    assert_eq!(
        continue_reading(&mut conn, Some(1), &visible).unwrap()[0].id,
        id
    );
    assert!(
        continue_reading(&mut conn, Some(2), &visible)
            .unwrap()
            .is_empty()
    );
    assert!(
        continue_reading(&mut conn, Some(1), &[])
            .unwrap()
            .is_empty()
    );
    diesel::update(media_files::table)
        .set(media_files::file_hash.eq("changed-content"))
        .execute(&mut conn)
        .unwrap();
    assert!(status(&mut conn, id, Some(1)).unwrap().is_none());
    assert!(
        source(&mut conn, id, 1)
            .unwrap()
            .unwrap()
            .document
            .progress
            .is_none()
    );
    assert!(
        continue_reading(&mut conn, Some(1), &visible)
            .unwrap()
            .is_empty()
    );
    let parent = visible
        .iter()
        .find(|item| item.item_type == "book_work")
        .unwrap()
        .id;
    assert!(source(&mut conn, parent, 1).unwrap().is_none());
    diesel::update(media_items::table.find(id))
        .set(media_items::deleted_at.eq(Some(1_i64)))
        .execute(&mut conn)
        .unwrap();
    assert!(source(&mut conn, id, 1).unwrap().is_none());
}

#[test]
fn duplicate_reconciliation_retains_the_latest_position_for_each_account() {
    let fixture = Fixture::new();
    let mut conn = database();
    let id = fixture.seed(&mut conn);
    let other = list_media_items(&mut conn, None)
        .unwrap()
        .into_iter()
        .find(|item| item.relative_path.ends_with("book.rtf"))
        .unwrap()
        .id;
    let state = position("same-content".into());
    save_progress(&mut conn, 1, id, &state).unwrap();
    save_progress(&mut conn, 1, other, &state).unwrap();
    save_progress(&mut conn, 2, other, &state).unwrap();
    diesel::update(progress::table.find((1, other)))
        .set((
            progress::updated_at.eq(i64::MAX),
            progress::payload_json.eq(serde_json::to_string(&ReadingProgress {
                fraction: 0.8,
                ..state
            })
            .unwrap()),
        ))
        .execute(&mut conn)
        .unwrap();
    merge_progress(&mut conn, id, other).unwrap();
    let raw = progress::table
        .find((1, id))
        .select(progress::payload_json)
        .first::<String>(&mut conn)
        .unwrap();
    assert_eq!(
        serde_json::from_str::<ReadingProgress>(&raw)
            .unwrap()
            .fraction,
        0.8
    );
    assert_eq!(
        progress::table
            .filter(progress::media_item_id.eq(id))
            .count()
            .get_result::<i64>(&mut conn)
            .unwrap(),
        2
    );
}

#[test]
fn resetting_reading_state_retains_bookmarks_and_returns_the_action_to_unopened() {
    let fixture = Fixture::new();
    let mut conn = database();
    let id = fixture.seed(&mut conn);
    let revision = source(&mut conn, id, 1).unwrap().unwrap().document.revision;
    let mut state = position(revision);
    save_progress(&mut conn, 1, id, &state).unwrap();
    state.locator.clear();
    state.fraction = 0.0;
    state.completed = false;
    save_progress(&mut conn, 1, id, &state).unwrap();
    assert!(status(&mut conn, id, Some(1)).unwrap().is_none());
    assert_eq!(
        source(&mut conn, id, 1)
            .unwrap()
            .unwrap()
            .document
            .progress
            .unwrap()
            .annotations[0]
            .note,
        "My private thoughts"
    );
    assert_eq!(
        source(&mut conn, id, 1)
            .unwrap()
            .unwrap()
            .document
            .progress
            .unwrap()
            .bookmarks
            .len(),
        1
    );
    let visible = list_media_items(&mut conn, None).unwrap();
    assert!(
        continue_reading(&mut conn, Some(1), &visible)
            .unwrap()
            .is_empty()
    );
    state.page_count = Some(123);
    save_progress(&mut conn, 1, id, &state).unwrap();
    let status = status(&mut conn, id, Some(1)).unwrap().unwrap();
    assert!(!status.started);
    assert_eq!(status.page_count, Some(123));
    assert!(
        continue_reading(&mut conn, Some(1), &visible)
            .unwrap()
            .is_empty()
    );
    for count in [0, 100_001] {
        state.page_count = Some(count);
        assert!(!state.valid());
    }
}

#[test]
fn saved_page_counts_reach_recommendations_library_and_search_cards() {
    let fixture = Fixture::new();
    let mut conn = database();
    let id = fixture.seed(&mut conn);
    let revision = source(&mut conn, id, 1).unwrap().unwrap().document.revision;
    let mut state = position(revision);
    // Cached pagination survives resetting the position and does not start reading.
    state.locator.clear();
    state.fraction = 0.0;
    state.page_count = Some(41);
    save_progress(&mut conn, 1, id, &state).unwrap();
    let library =
        list_media_items_for_user_with_preferred_languages(&mut conn, Some(1), None, &[]).unwrap();
    let home = get_media_home(&mut conn, Some(1), None).unwrap();
    let search =
        search_media_items_for_user_with_preferred_languages(&mut conn, Some(1), "book", None, &[])
            .unwrap();
    for cards in [
        library,
        search,
        home.shelves
            .into_iter()
            .find(|shelf| shelf.id == "recommended")
            .unwrap()
            .items,
    ] {
        let status = cards
            .iter()
            .find(|item| item.id == id)
            .unwrap()
            .reading_progress
            .as_ref()
            .unwrap();
        assert_eq!(status.page_count, Some(41));
        assert!(!status.started);
        assert_eq!(status.fraction, 0.0);
        assert!(
            cards
                .iter()
                .filter(|item| item.id != id)
                .all(|item| item.reading_progress.is_none())
        );
    }
}

#[test]
fn card_page_counts_are_account_and_revision_specific_and_ignore_invalid_states() {
    let fixture = Fixture::new();
    let mut conn = database();
    let id = fixture.seed(&mut conn);
    let revision = source(&mut conn, id, 1).unwrap().unwrap().document.revision;
    let mut state = position(revision.clone());
    state.page_count = Some(41);
    state.completed = true;
    save_progress(&mut conn, 1, id, &state).unwrap();
    let mut cards = list_media_items(&mut conn, None).unwrap();
    apply_statuses(&mut conn, Some(1), &mut cards).unwrap();
    assert!(
        cards
            .iter()
            .find(|item| item.id == id)
            .unwrap()
            .reading_progress
            .as_ref()
            .unwrap()
            .completed
    );
    for user in [Some(2), None] {
        apply_statuses(&mut conn, user, &mut cards).unwrap();
        assert!(cards.iter().all(|item| item.reading_progress.is_none()));
    }
    for (revision, count) in [
        ("outdated-revision".into(), 41),
        (revision, 0),
    ] {
        state.revision = revision;
        state.page_count = Some(count);
        save_progress(&mut conn, 1, id, &state).unwrap();
        apply_statuses(&mut conn, Some(1), &mut cards).unwrap();
        assert!(cards.iter().all(|item| item.reading_progress.is_none()));
    }
}

#[tokio::test]
async fn native_books_need_no_converter_and_failed_conversion_preserves_originals() {
    let fixture = Fixture::new();
    let mut conn = database();
    let id = fixture.seed(&mut conn);
    let mut settings = crate::config::Settings::default();
    settings.general.data_dir = fixture.0.join("data").to_string_lossy().into();
    settings.reader.converter_path = "missing-koko-reader-converter".into();
    let native = content(source(&mut conn, id, 1).unwrap().unwrap(), &settings)
        .await
        .unwrap();
    assert_eq!(std::fs::read(native).unwrap(), b"original book contents");
    let other = list_media_items(&mut conn, None)
        .unwrap()
        .into_iter()
        .find(|item| item.relative_path.ends_with("book.rtf"))
        .unwrap()
        .id;
    let error = content(source(&mut conn, other, 1).unwrap().unwrap(), &settings)
        .await
        .unwrap_err();
    assert!(error.contains("Calibre"));
    assert_eq!(
        std::fs::read(fixture.0.join("Author/Book/book.rtf")).unwrap(),
        b"original book contents"
    );
}

#[tokio::test]
async fn reader_routes_enforce_auth_library_access_range_and_revision() {
    use rocket::{
        http::{
            ContentType,
            Header,
            Status,
        },
        local::asynchronous::Client,
    };
    let fixture = Arc::new(Fixture::new());
    let rocket = rocket::custom(rocket::Config::figment().merge((
        "databases.sqlite_db",
        serde_json::json!({"url": ":memory:", "pool_size": 1}),
    )))
    .attach(crate::db::DbConn::fairing())
    .mount(
        "/",
        rocket::routes![
            crate::web::routes::ebooks::document,
            crate::web::routes::ebooks::content,
            crate::web::routes::ebooks::progress,
            crate::web::routes::media::stream_item
        ],
    );
    let client = Client::tracked(rocket).await.unwrap();
    let db = crate::db::DbConn::get_one(client.rocket()).await.unwrap();
    let seed = fixture.clone();
    let id = db.run(move |conn| seed.seed(conn)).await;
    let html = db
        .run(|conn| {
            list_media_items(conn, None)
                .unwrap()
                .into_iter()
                .find(|item| item.relative_path.ends_with("book.html"))
                .unwrap()
                .id
        })
        .await;
    drop(db);
    let token = crate::auth::create_token("1", crate::auth::get_jwt_secret()).unwrap();
    let auth = || Header::new("Authorization", format!("Bearer {token}"));
    let url = format!("/api/v1/items/{id}/reader");
    assert_eq!(
        client.get(&url).dispatch().await.status(),
        Status::Unauthorized
    );
    let document = client
        .get(&url)
        .header(auth())
        .dispatch()
        .await
        .into_json::<serde_json::Value>()
        .await
        .unwrap();
    let revision = document["revision"].as_str().unwrap().to_string();
    let mut state = position(revision);
    let save = format!("{url}/progress");
    assert_eq!(
        client
            .put(&save)
            .header(auth())
            .header(ContentType::JSON)
            .json(&state)
            .dispatch()
            .await
            .status(),
        Status::NoContent
    );
    let second_token = crate::auth::create_token("2", crate::auth::get_jwt_secret()).unwrap();
    let second_auth = || Header::new("Authorization", format!("Bearer {second_token}"));
    let other_document = client
        .get(&url)
        .header(second_auth())
        .dispatch()
        .await
        .into_json::<serde_json::Value>()
        .await
        .unwrap();
    assert!(other_document["progress"].is_null());
    let mut other_state = serde_json::to_value(&state).unwrap();
    // A caller cannot select the owner by adding an account field to the payload.
    other_state["user_id"] = serde_json::json!(1);
    other_state["annotations"][0]["note"] = serde_json::json!("Second user's private note");
    other_state["bookmarks"][0]["title"] = serde_json::json!("Second user's name");
    assert_eq!(
        client
            .put(&save)
            .header(second_auth())
            .header(ContentType::JSON)
            .json(&other_state)
            .dispatch()
            .await
            .status(),
        Status::NoContent
    );
    for (header, expected) in [
        (auth(), "My private thoughts"),
        (second_auth(), "Second user's private note"),
    ] {
        let saved = client
            .get(&url)
            .header(header)
            .dispatch()
            .await
            .into_json::<serde_json::Value>()
            .await
            .unwrap();
        assert_eq!(saved["progress"]["annotations"][0]["note"], expected);
    }
    state.revision = "stale".into();
    assert_eq!(
        client
            .put(&save)
            .header(auth())
            .header(ContentType::JSON)
            .json(&state)
            .dispatch()
            .await
            .status(),
        Status::Conflict
    );
    state.revision = document["revision"].as_str().unwrap().into();
    state.fraction = 1.1;
    assert_eq!(
        client
            .put(&save)
            .header(auth())
            .header(ContentType::JSON)
            .json(&state)
            .dispatch()
            .await
            .status(),
        Status::BadRequest
    );
    let response = client
        .get(format!("{url}/content"))
        .header(auth())
        .header(Header::new("Range", "bytes=0-7"))
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::PartialContent);
    assert_eq!(response.content_type(), Some(ContentType::Binary));
    assert_eq!(
        response.headers().get_one("Content-Disposition"),
        Some("attachment")
    );
    assert_eq!(
        response.headers().get_one("X-Content-Type-Options"),
        Some("nosniff")
    );
    assert!(
        response
            .headers()
            .get_one("Content-Security-Policy")
            .unwrap()
            .contains("sandbox")
    );
    assert_eq!(response.into_bytes().await.unwrap(), b"original");
    let html_response = client
        .get(format!("/api/v1/items/{html}/reader/content"))
        .header(auth())
        .dispatch()
        .await;
    assert_eq!(html_response.status(), Status::Ok);
    assert_eq!(html_response.content_type(), Some(ContentType::Binary));
    assert!(
        html_response
            .headers()
            .get_one("Content-Security-Policy")
            .unwrap()
            .contains("sandbox")
    );
    assert_eq!(
        client
            .get(format!("/api/v1/items/{html}/stream"))
            .header(auth())
            .dispatch()
            .await
            .status(),
        Status::Conflict
    );
    let db = crate::db::DbConn::get_one(client.rocket()).await.unwrap();
    db.run(|conn| {
        diesel::update(media_libraries::table)
            .set(media_libraries::allowed_user_ids_json.eq("[2]"))
            .execute(conn)
            .unwrap();
    })
    .await;
    drop(db);
    assert_eq!(
        client.get(&url).header(auth()).dispatch().await.status(),
        Status::NotFound
    );
    assert_eq!(
        client
            .get(format!("{url}/content"))
            .header(auth())
            .dispatch()
            .await
            .status(),
        Status::NotFound
    );
}
