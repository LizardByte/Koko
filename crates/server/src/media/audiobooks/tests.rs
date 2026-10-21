//! Catalog regressions using typed Diesel fixtures and real scanner inventories.

use super::*;
use crate::db::schema::{
    media_files,
    media_items,
    users,
};
use diesel::{
    Connection,
    TextExpressionMethods,
};

struct Library(std::path::PathBuf);
impl Library {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "koko-audiobooks-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(
        &self,
        path: &str,
    ) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"audiobook fixture").unwrap();
    }

    fn settings(&self) -> MediaLibrarySettings {
        MediaLibrarySettings {
            name: "Books".into(),
            path: self.0.to_string_lossy().into(),
            kind: MediaLibraryKind::Books,
            recursive: true,
            ..Default::default()
        }
    }

    fn scan(
        &self,
        conn: &mut SqliteConnection,
    ) {
        sync_library_catalog(
            conn,
            &[self.settings()],
            &FfmpegSettings {
                ffprobe_path: "missing-audiobook-test-ffprobe".into(),
                ..Default::default()
            },
        )
        .unwrap();
    }
}
impl Drop for Library {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn database() -> SqliteConnection {
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    crate::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    for (id, name) in [(1, "alice"), (2, "bob")] {
        diesel::insert_into(users::table)
            .values((
                users::id.eq(id),
                users::username.eq(name),
                users::password.eq("hash"),
                users::admin.eq(false),
            ))
            .execute(&mut conn)
            .unwrap();
    }
    conn
}

#[test]
fn librivox_is_a_new_book_default_and_existing_provider_choices_remain_editable() {
    let mut settings = MediaLibrarySettings {
        kind: MediaLibraryKind::Books,
        metadata_providers: vec![
            MetadataProviderId::Libex,
            MetadataProviderId::OpenLibrary,
            MetadataProviderId::AudioSilo,
        ],
        ..Default::default()
    };
    settings.normalize();
    assert!(
        !settings
            .metadata_providers
            .contains(&MetadataProviderId::LibriVox)
    );
    settings.metadata_providers = vec![MetadataProviderId::Libex];
    settings.normalize();
    assert_eq!(settings.metadata_providers, [MetadataProviderId::Libex]);
    settings.metadata_providers.clear();
    settings.normalize();
    assert_eq!(
        settings.metadata_providers,
        [
            MetadataProviderId::ComicVine,
            MetadataProviderId::OpenLibrary,
            MetadataProviderId::Libex,
            MetadataProviderId::LibriVox,
            MetadataProviderId::AudioSilo
        ]
    );
    for id in &settings.metadata_providers {
        assert!(
            crate::config::MetadataSettings::default()
                .providers
                .iter()
                .find(|provider| provider.id == *id)
                .unwrap()
                .enabled
        );
    }
    assert_eq!(
        MetadataProviderId::from_storage_value("librivox"),
        Some(MetadataProviderId::LibriVox)
    );
}

#[test]
fn librivox_can_complete_a_work_only_match_but_cannot_replace_a_selected_narration() {
    use crate::metadata::{
        StoredMetadataSnapshot,
        upsert_item_metadata_snapshot,
    };
    let library = Library::new();
    library.write("Author/Book/01.mp3");
    let mut conn = database();
    library.scan(&mut conn);
    let id = book(&mut conn).id;
    let snapshot = |provider_id, external: &str, payload| StoredMetadataSnapshot {
        provider_id,
        external_id: external.into(),
        media_type: Some("audiobook".into()),
        title: Some("Book".into()),
        overview: None,
        artwork_url: None,
        backdrop_url: None,
        release_year: None,
        locale_key: String::new(),
        provider_locale_key: None,
        provider_payload_json: Some(serde_json::to_string(&payload).unwrap()),
    };
    let libex = snapshot(
        MetadataProviderId::Libex,
        "us/B000000001",
        serde_json::json!({"asin":"B000000001", "region":"us", "title":"Book",
            "authors":[{"name":"Author"}]}),
    );
    upsert_item_metadata_snapshot(&mut conn, id, &libex).unwrap();
    let candidates =
        list_automatic_metadata_refresh_candidates(&mut conn, None, usize::MAX).unwrap();
    assert_eq!(
        candidates
            .iter()
            .find(|item| item.item_id == id)
            .unwrap()
            .metadata_providers,
        [MetadataProviderId::LibriVox]
    );
    let lv = snapshot(
        MetadataProviderId::LibriVox,
        "6112/recording",
        serde_json::json!({
            "id":"6112", "title":"Book", "authors":[{"first_name":"Author"}],
            "language":"English", "num_sections":"1", "totaltimesecs":60,
            "koko_recording_selected":true,
        "sections":[{"section_number":"1", "title":"Chapter One", "playtime":"60",
            "listen_url":"https://archive.org/book/01.mp3",
            "readers":[{"display_name":"LibriVox Reader"}]}]
        }),
    );
    upsert_item_metadata_snapshot(&mut conn, id, &lv).unwrap();
    let metadata = book(&mut conn).audiobook.unwrap();
    assert_eq!(
        metadata.recording.unwrap().provider_id,
        MetadataProviderId::LibriVox
    );
    assert_eq!(metadata.narrator, "LibriVox Reader");
    assert_eq!(metadata.chapters.len(), 1);
    assert_eq!(metadata.chapter_files, ["01.mp3"]);
    assert!(
        !list_automatic_metadata_refresh_candidates(&mut conn, None, usize::MAX)
            .unwrap()
            .iter()
            .any(|candidate| candidate.item_id == id)
    );
    let selected = snapshot(
        MetadataProviderId::Libex,
        "us/B000000001/recording",
        serde_json::json!({
            "asin":"B000000001", "region":"us", "title":"Book", "koko_recording_selected":true,
            "narrators":[{"name":"Chosen Reader"}]
        }),
    );
    upsert_item_metadata_snapshot(&mut conn, id, &selected).unwrap();
    select_recording_source(
        &mut conn,
        id,
        &MetadataProviderId::Libex,
        "us/B000000001/recording",
    )
    .unwrap();
    upsert_item_metadata_snapshot(&mut conn, id, &lv).unwrap();
    let metadata = book(&mut conn).audiobook.unwrap();
    assert_eq!(
        metadata.recording.unwrap().provider_id,
        MetadataProviderId::Libex
    );
    assert_eq!(metadata.narrator, "Chosen Reader");
    assert!(metadata.chapter_files.is_empty());
    select_recording_source(
        &mut conn,
        id,
        &MetadataProviderId::LibriVox,
        "6112/recording",
    )
    .unwrap();
    assert_eq!(
        book(&mut conn).audiobook.unwrap().narrator,
        "LibriVox Reader"
    );
    select_recording_source(&mut conn, id, &MetadataProviderId::LibriVox, "6112").unwrap();
    assert!(book(&mut conn).audiobook.unwrap().recording.is_none());
}

#[test]
fn author_profiles_are_matchable_and_independent_of_book_metadata_and_covers() {
    use crate::db::schema::{
        audiobook_provider_metadata,
        item_metadata_links,
    };
    use crate::metadata::{
        StoredMetadataSnapshot,
        upsert_item_metadata_snapshot,
    };
    let library = Library::new();
    library.write("Author/Book One/01.mp3");
    library.write("Author/Book Two/01.mp3");
    let mut conn = database();
    library.scan(&mut conn);
    let items = list_media_items(&mut conn, None).unwrap();
    let author = items
        .iter()
        .find(|item| item.item_type == "book_author")
        .unwrap();
    let work = items
        .iter()
        .find(|item| item.item_type == "book_work")
        .unwrap();
    assert!(
        list_automatic_metadata_refresh_candidates(&mut conn, None, usize::MAX)
            .unwrap()
            .iter()
            .any(|candidate| candidate.item_id == author.id && candidate.audiobook.is_none())
    );
    let profile = StoredMetadataSnapshot {
        provider_id: MetadataProviderId::OpenLibrary,
        external_id: "authors/OL1A".into(),
        media_type: Some("author".into()),
        title: Some("Canonical Author".into()),
        overview: Some("Author biography".into()),
        artwork_url: Some("https://covers.openlibrary.org/a/id/123-L.jpg?default=false".into()),
        backdrop_url: None,
        release_year: None,
        locale_key: String::new(),
        provider_locale_key: None,
        provider_payload_json: Some(
            serde_json::json!({"key": "/authors/OL1A",
            "name": "Canonical Author", "birth_date": "1900"})
            .to_string(),
        ),
    };
    upsert_item_metadata_snapshot(&mut conn, author.id, &profile).unwrap();
    let detail = get_media_item(&mut conn, author.id, "").unwrap().unwrap();
    assert_eq!(detail.display_title, "Canonical Author");
    assert_eq!(detail.overview.as_deref(), Some("Author biography"));
    assert_eq!(detail.tagline.as_deref(), Some("Born 1900"));
    assert!(detail.poster_url.is_some());
    assert!(detail.audiobook.is_none());
    assert_eq!(
        audiobook_provider_metadata::table
            .filter(audiobook_provider_metadata::media_item_id.eq(author.id))
            .count()
            .get_result::<i64>(&mut conn)
            .unwrap(),
        0
    );
    for child in items.iter().filter(|item| item.id != author.id) {
        assert!(
            get_preferred_item_artwork_metadata_link_for_languages(
                &mut conn,
                child.id,
                &[],
                ArtworkKind::Poster
            )
            .unwrap()
            .is_none()
        );
        assert!(
            get_media_item(&mut conn, child.id, "")
                .unwrap()
                .unwrap()
                .overview
                .is_none()
        );
    }
    let work_snapshot = StoredMetadataSnapshot {
        external_id: "works/OL1W".into(),
        media_type: Some("book".into()),
        title: Some("Written Work".into()),
        artwork_url: None,
        provider_payload_json: Some(
            serde_json::json!({"key": "/works/OL1W", "title": "Written Work"}).to_string(),
        ),
        ..profile
    };
    let work_link = upsert_item_metadata_snapshot(&mut conn, work.id, &work_snapshot).unwrap();
    let ids = crate::metadata::clear_item_metadata_tree(&mut conn, author.id, true).unwrap();
    assert_eq!(ids, vec![author.id]);
    assert_eq!(
        item_metadata_links::table
            .find(work_link.id)
            .select(item_metadata_links::media_item_id)
            .first::<i32>(&mut conn)
            .unwrap(),
        work.id
    );
    let candidates =
        list_automatic_metadata_refresh_candidates(&mut conn, None, usize::MAX).unwrap();
    assert!(
        !candidates
            .iter()
            .any(|candidate| candidate.item_id == author.id)
    );
    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.item_type == "book_work")
    );
    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.item_type == "audiobook")
    );
}
fn book(conn: &mut SqliteConnection) -> MediaItemDetail {
    let item = list_media_items(conn, None)
        .unwrap()
        .into_iter()
        .find(|item| item.item_type == "audiobook")
        .unwrap();
    get_media_item(conn, item.id, "").unwrap().unwrap()
}

#[test]
fn untagged_five_disc_recording_is_one_book_with_45_ordered_files() {
    let library = Library::new();
    for disc in 1..=5 {
        for track in 1..=9 {
            library.write(&format!("Author/Book/CD {disc}/Track {track}.mp3"));
        }
    }
    library.write("Author/Book/Book.epub");
    library.write("ignore.mp4");
    let mut conn = database();
    library.scan(&mut conn);
    let detail = book(&mut conn);
    assert_eq!(detail.display_title, "Book");
    assert_eq!(detail.audiobook.as_ref().unwrap().author, "Author");
    assert_eq!(detail.children.len(), 45);
    assert_eq!(
        detail.children[0].relative_path,
        "Author/Book/CD 1/Track 1.mp3"
    );
    assert_eq!(
        detail.children[8].relative_path,
        "Author/Book/CD 1/Track 9.mp3"
    );
    assert_eq!(
        detail.children[9].relative_path,
        "Author/Book/CD 2/Track 1.mp3"
    );
    assert!(detail.children.iter().all(|part| part.playable
        && part.music.is_none()
        && part.audiobook.as_ref().unwrap().chapters.is_empty()));
    let before = detail
        .children
        .iter()
        .map(|part| part.id)
        .collect::<Vec<_>>();
    library.scan(&mut conn);
    assert_eq!(
        book(&mut conn)
            .children
            .iter()
            .map(|part| part.id)
            .collect::<Vec<_>>(),
        before
    );
    let items = list_media_items(&mut conn, None).unwrap();
    assert_eq!(
        items.iter().filter(|item| item.item_type == "book").count(),
        1
    );
    assert_eq!(items.len(), 49);
    assert_eq!(
        detail
            .hierarchy
            .iter()
            .map(|item| item.item_type.as_str())
            .collect::<Vec<_>>(),
        ["book_author", "book_work"]
    );
    let work = get_media_item(&mut conn, detail.parent_id.unwrap(), "")
        .unwrap()
        .unwrap();
    assert_eq!(work.children.len(), 2);
    assert!(work.children.iter().any(|item| item.item_type == "book"));
    assert_eq!(
        list_library_settings(&mut conn).unwrap()[0].metadata_providers,
        vec![
            MetadataProviderId::ComicVine,
            MetadataProviderId::OpenLibrary,
            MetadataProviderId::Libex,
            MetadataProviderId::LibriVox,
            MetadataProviderId::AudioSilo
        ]
    );
    let recent = get_media_home(&mut conn, Some(1), None)
        .unwrap()
        .shelves
        .into_iter()
        .find(|shelf| shelf.id == "recently_added")
        .unwrap();
    assert_eq!(
        recent
            .items
            .iter()
            .filter(|item| item.item_type == "audiobook")
            .count(),
        1
    );
    assert!(
        !recent
            .items
            .iter()
            .any(|item| item.item_type == "audiobook_part")
    );
}

#[test]
fn cd_directories_in_the_library_root_belong_to_one_recording() {
    let library = Library::new();
    for disc in 1..=5 {
        for track in 1..=9 {
            library.write(&format!("CD {disc}/Track {track}.mp3"));
        }
    }
    let mut conn = database();
    library.scan(&mut conn);
    let items = list_media_items(&mut conn, None).unwrap();
    assert_eq!(
        items
            .iter()
            .filter(|item| item.item_type == "audiobook")
            .count(),
        1
    );
    let detail = book(&mut conn);
    assert_eq!(
        detail.display_title,
        library.0.file_name().unwrap().to_str().unwrap()
    );
    assert_eq!(detail.children.len(), 45);
    assert_eq!(detail.children[9].relative_path, "CD 2/Track 1.mp3");
}

#[test]
fn resume_uses_latest_file_and_progress_is_private_to_each_listener() {
    let library = Library::new();
    for track in 1..=3 {
        library.write(&format!("Author/Book/{track}.mp3"));
    }
    let mut conn = database();
    library.scan(&mut conn);
    let mut detail = book(&mut conn);
    let ids = detail
        .children
        .iter()
        .map(|part| part.id)
        .collect::<Vec<_>>();
    upsert_playback_progress(&mut conn, 1, ids[0], 12_345, Some(60_000), false).unwrap();
    apply_user_playback_context_to_detail(&mut conn, Some(1), &mut detail).unwrap();
    assert_eq!(
        detail
            .playback_target
            .as_ref()
            .map(|target| (target.item_id, target.start_ms)),
        Some((ids[0], 12_345))
    );
    let home = get_media_home(&mut conn, Some(1), None).unwrap();
    assert_eq!(
        home.shelves[0]
            .items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        vec![detail.id]
    );
    assert!(
        get_media_home(&mut conn, Some(2), None).unwrap().shelves[0]
            .items
            .is_empty()
    );
    upsert_playback_progress(&mut conn, 1, ids[0], 60_000, Some(60_000), true).unwrap();
    let mut detail = book(&mut conn);
    apply_user_playback_context_to_detail(&mut conn, Some(1), &mut detail).unwrap();
    assert_eq!(detail.playback_target.unwrap().item_id, ids[1]);
    upsert_playback_progress(&mut conn, 1, ids[1], 60_000, Some(60_000), true).unwrap();
    upsert_playback_progress(&mut conn, 1, ids[2], 60_000, Some(60_000), true).unwrap();
    assert!(
        get_media_home(&mut conn, Some(1), None).unwrap().shelves[0]
            .items
            .is_empty()
    );
    let mut detail = book(&mut conn);
    apply_user_playback_context_to_detail(&mut conn, Some(1), &mut detail).unwrap();
    assert!(detail.playback_target.is_none());
    assert_eq!(detail.restart_playback_target.unwrap().item_id, ids[0]);
    diesel::update(media_items::table.filter(media_items::id.eq(ids[0])))
        .set(media_items::missing_since.eq(Some(1_i64)))
        .execute(&mut conn)
        .unwrap();
    let mut detail = book(&mut conn);
    apply_user_playback_context_to_detail(&mut conn, Some(2), &mut detail).unwrap();
    assert_eq!(detail.playback_target.unwrap().item_id, ids[1]);
}

#[test]
fn partially_tagged_librivox_files_use_filename_order_for_detail_and_resume() {
    let library = Library::new();
    for number in 0..7 {
        library.write(&format!(
            "Thomas Paine/Common Sense/commonsense_{number}_paine_64kb.mp3"
        ));
    }
    let mut conn = database();
    library.scan(&mut conn);
    diesel::update(media_files::table.filter(media_files::path.like("%commonsense_3_%")))
        .set(media_files::metadata_json.eq(r#"{"format":{"tags":{"track":"4/7"}}}"#))
        .execute(&mut conn)
        .unwrap();
    library.scan(&mut conn);
    let mut detail = book(&mut conn);
    let ordered = detail
        .children
        .iter()
        .map(|part| part.relative_path.as_str())
        .collect::<Vec<_>>();
    for (number, path) in ordered.iter().enumerate() {
        assert!(path.contains(&format!("commonsense_{number}_")));
    }
    let first = detail.children[0].id;
    apply_user_playback_context_to_detail(&mut conn, Some(1), &mut detail).unwrap();
    assert_eq!(detail.playback_target.unwrap().item_id, first);
}

#[test]
fn embedded_chapters_keep_mid_track_offsets_and_ignore_invalid_ranges() {
    let library = Library::new();
    library.write("Author/Book/01.m4b");
    let mut conn = database();
    library.scan(&mut conn);
    let payload = serde_json::json!({
        "format": {"tags": {"album": "Tagged Book",
            "composer": "Tagged Author",
            "narrator": "Reader",
            "series": "Series",
            "series_part": "2.5"}},
        "streams": [{"codec_type": "audio", "tags": {"language": "eng"}}],
        "chapters": [
            {"start_time": "35.25", "end_time": "60", "tags": {"title": "Chapter Two"}},
            {"start_time": "0", "end_time": "35.25", "tags": {"title": "Chapter One"}},
            {"start_time": "NaN", "end_time": "60"}, {"start_time": "70", "end_time": "65"},
            {"start_time": "55", "end_time": "Infinity"}
        ]
    });
    diesel::update(media_files::table)
        .set((
            media_files::metadata_json.eq(payload.to_string()),
            media_files::duration_ms.eq(Some(60_000_i64)),
        ))
        .execute(&mut conn)
        .unwrap();
    library.scan(&mut conn);
    let detail = book(&mut conn);
    let metadata = detail.audiobook.unwrap();
    assert_eq!(
        (
            metadata.title.as_str(),
            metadata.author.as_str(),
            metadata.narrator.as_str()
        ),
        ("Tagged Book", "Tagged Author", "Reader")
    );
    assert_eq!(metadata.language.as_deref(), Some("eng"));
    let part = detail.children[0].audiobook.as_ref().unwrap();
    assert_eq!(part.chapters.len(), 2);
    assert_eq!(part.chapters[1].start_ms, 35_250);
    assert_eq!(part.series_position.as_deref(), Some("2.5"));
}

#[test]
fn recordings_in_different_folders_and_roots_are_never_merged_by_title() {
    let library = Library::new();
    library.write("Author/Book English/01.mp3");
    library.write("Author/Book French/01.mp3");
    library.write("Standalone.m4b");
    let mut conn = database();
    library.scan(&mut conn);
    diesel::update(media_files::table)
        .set(media_files::metadata_json.eq(r#"{"format":{"tags":{"album":"Same Book"}}}"#))
        .execute(&mut conn)
        .unwrap();
    library.scan(&mut conn);
    let items = list_media_items(&mut conn, None).unwrap();
    assert_eq!(
        items
            .iter()
            .filter(|item| item.item_type == "audiobook")
            .count(),
        3
    );
    assert!(
        items
            .iter()
            .filter(|item| item.item_type == "audiobook")
            .all(|item| item.child_count == 1)
    );
}

#[test]
fn recording_selection_is_independent_of_provider_order_and_unlink_restores_local_facts() {
    use crate::db::schema::media_libraries;
    use crate::metadata::{
        StoredMetadataSnapshot,
        upsert_item_metadata_snapshot,
    };
    let library = Library::new();
    library.write("Author/Book/01.m4b");
    let mut conn = database();
    library.scan(&mut conn);
    let original = book(&mut conn);
    for (provider_id, title, payload) in [
        (
            MetadataProviderId::Libex,
            "Libex title",
            serde_json::json!({
                "title": "Libex title", "authors": [{"name": "Libex author"}],
                "narrators": [{"name": "A different narrator"}], "koko_recording_selected": false
            }),
        ),
        (
            MetadataProviderId::AudioSilo,
            "AudioSilo title",
            serde_json::json!({
                "id": "example", "title": "AudioSilo title",
                "authors": [{"name": "AudioSilo author"}],
                "koko_recording": {"id": "reader", "narrators": [{"name": "Selected reader"}]}
            }),
        ),
    ] {
        let snapshot = StoredMetadataSnapshot {
            provider_id: provider_id.clone(),
            external_id: if provider_id == MetadataProviderId::AudioSilo {
                "example/reader"
            } else {
                "example"
            }
            .into(),
            media_type: Some("audiobook".into()),
            title: Some(title.into()),
            overview: None,
            artwork_url: None,
            backdrop_url: None,
            release_year: None,
            locale_key: String::new(),
            provider_locale_key: None,
            provider_payload_json: Some(payload.to_string()),
        };
        upsert_item_metadata_snapshot(&mut conn, original.id, &snapshot).unwrap();
    }
    let detail = book(&mut conn);
    assert_eq!(detail.display_title, "Libex title");
    assert!(detail.audiobook.as_ref().unwrap().recording.is_none());
    assert!(detail.audiobook.as_ref().unwrap().narrator.is_empty());
    assert_eq!(detail.audiobook.unwrap().author, "Libex author");
    select_recording_source(
        &mut conn,
        original.id,
        &MetadataProviderId::AudioSilo,
        "example/reader",
    )
    .unwrap();
    assert_eq!(
        book(&mut conn).audiobook.unwrap().narrator,
        "Selected reader"
    );
    diesel::update(media_libraries::table)
        .set(media_libraries::metadata_providers_json.eq("[\"audiosilo\",\"libex\"]"))
        .execute(&mut conn)
        .unwrap();
    let detail = book(&mut conn);
    assert_eq!(detail.display_title, "AudioSilo title");
    assert_eq!(detail.audiobook.unwrap().narrator, "Selected reader");
    select_recording_source(
        &mut conn,
        original.id,
        &MetadataProviderId::AudioSilo,
        "example",
    )
    .unwrap();
    let local = book(&mut conn).audiobook.unwrap();
    assert!(local.recording.is_none());
    assert!(local.narrator.is_empty());
    crate::metadata::clear_item_metadata_tree(&mut conn, original.id, true).unwrap();
    let detail = book(&mut conn);
    assert_eq!(detail.display_title, "Book");
    assert_eq!(detail.audiobook.unwrap().author, "Author");
}

#[test]
fn unavailable_libraries_do_not_expose_audiobook_progress_or_chapter_targets() {
    use crate::db::schema::media_libraries;
    let library = Library::new();
    library.write("Author/Book/01.mp3");
    let mut conn = database();
    library.scan(&mut conn);
    let detail = book(&mut conn);
    upsert_playback_progress(
        &mut conn,
        1,
        detail.children[0].id,
        35000,
        Some(60000),
        false,
    )
    .unwrap();
    diesel::update(media_libraries::table)
        .set(media_libraries::allowed_user_ids_json.eq("[2]"))
        .execute(&mut conn)
        .unwrap();
    let hidden = get_media_home(&mut conn, Some(1), None).unwrap();
    assert!(hidden.shelves.iter().all(|shelf| shelf.items.is_empty()));
    assert!(!user_can_access_library(&mut conn, detail.library_id, Some(1)).unwrap());
    assert!(user_can_access_library(&mut conn, detail.library_id, Some(2)).unwrap());
}

#[test]
fn written_work_and_secondary_recording_facts_supplement_the_primary_source() {
    use crate::metadata::{
        StoredMetadataSnapshot,
        provider_metadata_details,
        upsert_item_metadata_link,
        upsert_item_metadata_snapshot,
    };
    let library = Library::new();
    library.write("Author/Book/01.m4b");
    let mut conn = database();
    library.scan(&mut conn);
    let id = book(&mut conn).id;
    for (provider, external, title, payload, relation) in [
        (
            MetadataProviderId::Libex,
            "us/B002V1CDCO/recording",
            "Primary title",
            serde_json::json!({"asin": "B002V1CDCO", "region": "us", "title": "Primary title",
                "authors": [{"name": "Primary author"}],
                "narrators": [{"name": "Reader"}], "koko_recording_selected": true}),
            "primary",
        ),
        (
            MetadataProviderId::OpenLibrary,
            "works/OL6036787W",
            "Written work",
            serde_json::json!({"key": "/works/OL6036787W", "title": "Written work",
                "koko_author_names": ["Primary author"]}),
            "primary",
        ),
        (
            MetadataProviderId::AudioSilo,
            "work/reader",
            "Supplement",
            serde_json::json!({"id": "work", "title": "Supplement",
                "authors": [{"name": "Primary author"}],
                "koko_recording": {"id": "reader", "narrators": [{"name": "Reader"}],
                    "asin": [{"asin": "B002V1CDCO"}]},
                "series": [{"id": "series", "name": "Series", "position": "2"}],
                "koko_chapters": [{"title": "Opening", "start_ms": 0, "end_ms": 60000}]}),
            "secondary",
        ),
    ] {
        let snapshot = StoredMetadataSnapshot {
            provider_id: provider,
            external_id: external.into(),
            media_type: Some("audiobook".into()),
            title: Some(title.into()),
            overview: None,
            artwork_url: None,
            backdrop_url: None,
            release_year: None,
            locale_key: String::new(),
            provider_locale_key: None,
            provider_payload_json: Some(payload.to_string()),
        };
        if relation == "secondary" {
            upsert_item_metadata_link(
                &mut conn,
                id,
                &snapshot,
                &provider_metadata_details(&snapshot),
                relation,
                None,
            )
            .unwrap();
        } else {
            upsert_item_metadata_snapshot(&mut conn, id, &snapshot).unwrap();
        }
    }
    assert!(book(&mut conn).audiobook.unwrap().chapters.is_empty());
    select_recording_source(
        &mut conn,
        id,
        &MetadataProviderId::Libex,
        "us/B002V1CDCO/recording",
    )
    .unwrap();
    let detail = book(&mut conn);
    let metadata = detail.audiobook.unwrap();
    assert_eq!(metadata.title, "Written work");
    assert_eq!(metadata.author, "Primary author");
    assert_eq!(metadata.series.as_deref(), Some("Series"));
    assert_eq!(metadata.chapters.len(), 1);
    assert_eq!(
        metadata.openlibrary_work_id.as_deref(),
        Some("works/OL6036787W")
    );
    assert_eq!(
        list_media_items(&mut conn, None)
            .unwrap()
            .iter()
            .find(|item| item.id == id)
            .unwrap()
            .display_title,
        "Written work"
    );
    // A different recording from the secondary must not fill this narration's chapters.
    use crate::db::schema::audiobook_provider_metadata as providers;
    let raw = providers::table
        .filter(providers::media_item_id.eq(id))
        .filter(providers::provider_id.eq("audiosilo"))
        .select(providers::metadata_json)
        .first::<String>(&mut conn)
        .unwrap();
    diesel::update(
        providers::table
            .filter(providers::media_item_id.eq(id))
            .filter(providers::provider_id.eq("audiosilo")),
    )
    .set(providers::metadata_json.eq(raw.replace("B002V1CDCO", "B005HF3EJC")))
    .execute(&mut conn)
    .unwrap();
    assert!(book(&mut conn).audiobook.unwrap().chapters.is_empty());
}

#[test]
fn author_work_and_narrations_share_a_book_but_keep_separate_progress() {
    let library = Library::new();
    library.write("Audiobooks/Author/Book/Reader One/CD 1/01.mp3");
    library.write("Audiobooks/Author/Book/Reader Two/01.m4b");
    library.write("Books/Author/Book/Book.epub");
    let mut conn = database();
    library.scan(&mut conn);
    let items = list_media_items(&mut conn, None).unwrap();
    let author = items
        .iter()
        .find(|item| item.item_type == "book_author")
        .unwrap();
    assert_eq!(author.display_title, "Author");
    assert_eq!(author.child_count, 1);
    let work = get_media_item(
        &mut conn,
        items
            .iter()
            .find(|item| item.item_type == "book_work")
            .unwrap()
            .id,
        "",
    )
    .unwrap()
    .unwrap();
    assert_eq!(work.display_title, "Book");
    assert_eq!(work.children.len(), 3);
    let versions = work
        .children
        .iter()
        .filter(|item| item.item_type == "audiobook")
        .collect::<Vec<_>>();
    let first = get_media_item(&mut conn, versions[0].id, "")
        .unwrap()
        .unwrap();
    assert_eq!(first.audiobook_versions.len(), 2);
    assert!(
        first
            .audiobook_versions
            .iter()
            .all(|version| version.label.starts_with("Reader "))
    );
    upsert_playback_progress(
        &mut conn,
        1,
        first.children[0].id,
        12345,
        Some(60000),
        false,
    )
    .unwrap();
    library.scan(&mut conn);
    let mut first_again = get_media_item(&mut conn, first.id, "").unwrap().unwrap();
    apply_user_playback_context_to_detail(&mut conn, Some(1), &mut first_again).unwrap();
    assert_eq!(first_again.playback_target.unwrap().start_ms, 12345);
    let mut other = get_media_item(&mut conn, versions[1].id, "")
        .unwrap()
        .unwrap();
    apply_user_playback_context_to_detail(&mut conn, Some(1), &mut other).unwrap();
    assert_eq!(other.playback_target.unwrap().start_ms, 0);
}

#[tokio::test]
async fn automatic_candidate_keeps_folder_author_and_title_when_rip_tags_are_wrong() {
    use crate::metadata::providers::{
        audiosilo::tests::mock,
        libex,
    };
    let library = Library::new();
    for disc in 1..=5 {
        library.write(&format!("GLENN BECK/COMMON SENSE/CD {disc}/Track 1.mp3"));
    }
    let mut conn = database();
    library.scan(&mut conn);
    diesel::update(media_files::table)
        .set(
            media_files::metadata_json
                .eq(r#"{"format":{"tags":{"album":"Audio CD","artist":"Unknown"}}}"#),
        )
        .execute(&mut conn)
        .unwrap();
    library.scan(&mut conn);
    let candidate = list_automatic_metadata_refresh_candidates(&mut conn, None, usize::MAX)
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.item_type == "audiobook")
        .unwrap();
    let local = candidate.audiobook.unwrap();
    assert_eq!(local.folder_title, "COMMON SENSE");
    assert_eq!(local.folder_author, "GLENN BECK");
    let (root, worker) = mock(vec![
        ("title=Audio+CD", serde_json::json!([])),
        (
            "author=GLENN+BECK",
            serde_json::json!([{
                "asin": "B002V1CDCO", "region": "us", "title": "Glenn Beck's Common Sense",
                "authors": [{"name": "Glenn Beck"}], "narrators": [{"name": "Glenn Beck"}],
                "lengthMinutes": 342, "bookFormat": "unabridged"
            }]),
        ),
    ]);
    let matched = libex::TEST_API_ROOT
        .scope(
            root,
            libex::guess(&crate::config::MetadataSettings::default(), &local),
        )
        .await
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    assert_eq!(matched.external_id, "us/B002V1CDCO");
}

#[test]
fn jellyfin_type_folders_optional_authors_and_direct_audio_files_need_no_reorganization() {
    let library = Library::new();
    for path in [
        "Books/Audiobooks/Author/Book1/Book1.flac",
        "Books/Audiobooks/Author/Book2/CD 1/01.mp3",
        "Books/Audiobooks/Book3/Book3.aac",
        "Books/Books/Author/Book1/Book1.epub",
        "Books/Books/Book4/content.epub",
        "Books/Audiobooks/Book5/[Reader One]/01.mp3",
        "Books/Audiobooks/Book5/[Reader Two]/01.mp3",
    ] {
        library.write(path);
    }
    let mut conn = database();
    library.scan(&mut conn);
    let items = list_media_items(&mut conn, None).unwrap();
    let works = items
        .iter()
        .filter(|item| item.item_type == "book_work")
        .collect::<Vec<_>>();
    assert_eq!(works.len(), 5);
    for title in [
        "Book1", "Book2", "Book3", "Book4", "Book5",
    ] {
        assert!(works.iter().any(|item| item.display_title == title));
    }
    assert!(
        items
            .iter()
            .filter(|item| item.item_type == "book_author")
            .all(|item| ["Author", "Unknown author"].contains(&item.display_title.as_str()))
    );
    let shared = get_media_item(
        &mut conn,
        works
            .iter()
            .find(|item| item.display_title == "Book1")
            .unwrap()
            .id,
        "",
    )
    .unwrap()
    .unwrap();
    assert_eq!(shared.children.len(), 2);
    let multi = get_media_item(
        &mut conn,
        works
            .iter()
            .find(|item| item.display_title == "Book5")
            .unwrap()
            .id,
        "",
    )
    .unwrap()
    .unwrap();
    assert_eq!(multi.children.len(), 2);
    assert!(multi.children.iter().all(|item| {
        item.audiobook
            .as_ref()
            .unwrap()
            .version_label
            .starts_with("Reader ")
    }));
}
