//! Relocation regressions exercise the real filesystem scanner and typed catalog state.

use super::*;
use crate::config::{
    FfmpegSettings,
    MediaLibraryKind,
    MediaLibrarySettings,
    MetadataProviderId,
};
use crate::db::schema::users;
use crate::media::{
    CatalogSyncScannerSink,
    PersistedLibrarySummary,
    ProbeContext,
    apply_user_playback_context_to_detail,
    audiobooks,
    get_library_files,
    get_media_item,
    get_user_playback_progress,
    list_media_items,
    load_catalog_file_for_item,
    replace_library_settings,
    sync_library_catalog,
    sync_logical_media_items_for_library,
    upsert_playback_progress,
};
use crate::scanner::{
    LibraryScanStatus,
    inspect_library_streaming,
};
use diesel::Connection;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "koko-relocation-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(
        &self,
        root: &str,
        path: &str,
        contents: &[u8],
    ) {
        let path = self.0.join(root).join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn settings(
        &self,
        root: &str,
        kind: MediaLibraryKind,
    ) -> MediaLibrarySettings {
        MediaLibrarySettings {
            name: "Relocation".into(),
            path: self.0.join(root).to_string_lossy().into(),
            kind,
            recursive: true,
            use_embedded_music_tags: true,
            ..Default::default()
        }
    }

    fn scan(
        &self,
        conn: &mut SqliteConnection,
        settings: &[MediaLibrarySettings],
    ) -> Vec<PersistedLibrarySummary> {
        sync_library_catalog(
            conn,
            settings,
            &FfmpegSettings {
                ffprobe_path: "missing-relocation-ffprobe".into(),
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn move_root(
        &self,
        from: &str,
        to: &str,
    ) {
        let from = self.0.join(from);
        let to = self.0.join(to);
        assert!(from.starts_with(&self.0) && to.starts_with(&self.0));
        std::fs::rename(from, to).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn database() -> SqliteConnection {
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    crate::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    crate::db::configure_sqlite_connection(&mut conn).unwrap();
    for id in 1..=2 {
        diesel::insert_into(users::table)
            .values((
                users::id.eq(id),
                users::username.eq(format!("user{id}")),
                users::password.eq("hash"),
                users::admin.eq(false),
            ))
            .execute(&mut conn)
            .unwrap();
    }
    conn
}
fn sample(kind: &MediaLibraryKind) -> &'static str {
    match kind {
        MediaLibraryKind::Shows => "Show/Season 1/Show S01E01.mkv",
        MediaLibraryKind::Music => "Artist/Album/01 Song.mp3",
        MediaLibraryKind::Books => "Author/Book/Reader/CD 1/Track 01.mp3",
        MediaLibraryKind::Photos => "Album/photo.jpg",
        _ => "Title/movie.mkv",
    }
}
fn seed_metadata(
    conn: &mut SqliteConnection,
    item: i32,
) -> i32 {
    let snapshot = crate::metadata::StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tmdb,
        external_id: "123".into(),
        media_type: Some("movie".into()),
        title: Some("Saved metadata".into()),
        overview: Some("Saved description".into()),
        artwork_url: None,
        backdrop_url: None,
        release_year: None,
        locale_key: String::new(),
        provider_locale_key: None,
        provider_payload_json: None,
    };
    crate::metadata::upsert_item_metadata_snapshot(conn, item, &snapshot)
        .unwrap()
        .id
}

#[test]
fn moving_each_library_kind_preserves_all_catalog_ids_metadata_and_user_progress() {
    for kind in [
        MediaLibraryKind::Movies,
        MediaLibraryKind::Shows,
        MediaLibraryKind::Music,
        MediaLibraryKind::Books,
        MediaLibraryKind::Photos,
        MediaLibraryKind::Mixed,
    ] {
        let fixture = Fixture::new();
        fixture.write("old", sample(&kind), b"unique original media");
        let mut conn = database();
        let before = fixture.scan(&mut conn, &[fixture.settings("old", kind.clone())]);
        let library_id = before[0].id;
        let items = list_media_items(&mut conn, None).unwrap();
        let ids = items.iter().map(|item| item.id).collect::<HashSet<_>>();
        let leaf = items.iter().find(|item| item.child_count == 0).unwrap().id;
        let owner = items
            .iter()
            .find(|item| item.item_type == "audiobook")
            .or_else(|| items.iter().find(|item| item.parent_id.is_none()))
            .unwrap()
            .id;
        let link = seed_metadata(&mut conn, owner);
        upsert_playback_progress(&mut conn, 1, leaf, 12345, Some(60000), false).unwrap();
        let membership = load_catalog_files_for_library(&mut conn, library_id, false).unwrap()[0]
            .library_file_id;
        diesel::update(media_items::table)
            .set(media_items::created_at.eq(Some(42_i64)))
            .execute(&mut conn)
            .unwrap();
        diesel::update(media_files::table)
            .set(media_files::duration_ms.eq(Some(60000_i64)))
            .execute(&mut conn)
            .unwrap();
        fixture.move_root("old", "new");
        // Saving settings before scanning must not discard the only record of the old root.
        replace_library_settings(&mut conn, &[fixture.settings("new", kind.clone())]).unwrap();
        fixture.scan(&mut conn, &[fixture.settings("new", kind.clone())]);
        let after = list_media_items(&mut conn, None).unwrap();
        assert_eq!(
            after.iter().map(|item| item.id).collect::<HashSet<_>>(),
            ids,
            "{kind:?}"
        );
        assert!(
            media_items::table
                .filter(media_items::deleted_at.is_null())
                .select(media_items::created_at)
                .load::<Option<i64>>(&mut conn)
                .unwrap()
                .iter()
                .all(|created| *created == Some(42))
        );
        assert_eq!(
            item_metadata_links::table
                .find(link)
                .select(item_metadata_links::media_item_id)
                .first::<i32>(&mut conn)
                .unwrap(),
            owner
        );
        assert_eq!(
            get_user_playback_progress(&mut conn, Some(1), leaf)
                .unwrap()
                .unwrap()
                .position_ms,
            12345
        );
        assert!(
            get_user_playback_progress(&mut conn, Some(2), leaf)
                .unwrap()
                .is_none()
        );
        let files = load_catalog_files_for_library(&mut conn, library_id, false).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].library_file_id, membership);
        assert!(
            files[0]
                .path
                .starts_with(fixture.0.join("new").to_str().unwrap())
        );
        assert_eq!(files[0].duration_ms, Some(60000));
        fixture.scan(&mut conn, &[fixture.settings("new", kind)]);
        assert_eq!(
            list_media_items(&mut conn, None)
                .unwrap()
                .iter()
                .map(|item| item.id)
                .collect::<HashSet<_>>(),
            ids
        );
    }
}

/// Reproduce an already-persisted duplicate tree produced by the previous scanner.
fn legacy_rescan(
    conn: &mut SqliteConnection,
    settings: &MediaLibrarySettings,
) {
    replace_library_settings(conn, std::slice::from_ref(settings)).unwrap();
    let row = crate::db::schema::media_libraries::table
        .select(MediaLibrary::as_select())
        .first::<MediaLibrary>(conn)
        .unwrap();
    let mut sink = CatalogSyncScannerSink {
        conn,
        library_row: &row,
        probe_context: ProbeContext {
            ffprobe_path: "missing-relocation-ffprobe",
            enabled: false,
        },
        scan_started_at: current_timestamp(),
    };
    inspect_library_streaming(settings, &mut sink).unwrap();
    sync_logical_media_items_for_library(conn, &row, &settings.kind).unwrap();
}

#[test]
fn a_rescan_repairs_existing_duplicates_and_merges_recent_progress_and_playlist_references() {
    for kind in [
        MediaLibraryKind::Movies,
        MediaLibraryKind::Shows,
        MediaLibraryKind::Music,
        MediaLibraryKind::Books,
    ] {
        let fixture = Fixture::new();
        fixture.write("old", sample(&kind), b"original edition");
        let mut conn = database();
        fixture.scan(&mut conn, &[fixture.settings("old", kind.clone())]);
        let before = list_media_items(&mut conn, None).unwrap();
        let original_ids = before.iter().map(|item| item.id).collect::<HashSet<_>>();
        let original = before.iter().find(|item| item.playable).unwrap().id;
        let root = before
            .iter()
            .find(|item| item.item_type == "audiobook")
            .or_else(|| before.iter().find(|item| item.parent_id.is_none()))
            .unwrap()
            .id;
        let link = seed_metadata(&mut conn, root);
        upsert_playback_progress(&mut conn, 1, original, 10000, Some(60000), false).unwrap();
        diesel::update(playback_progress::table)
            .set(playback_progress::updated_at.eq(Some(10_i64)))
            .execute(&mut conn)
            .unwrap();
        fixture.move_root("old", "new");
        legacy_rescan(&mut conn, &fixture.settings("new", kind.clone()));
        let duplicate = list_media_items(&mut conn, None)
            .unwrap()
            .into_iter()
            .find(|item| item.playable && !original_ids.contains(&item.id))
            .map_or(original, |item| item.id);
        for user in [1, 2] {
            upsert_playback_progress(&mut conn, user, duplicate, 35000, Some(60000), false)
                .unwrap();
        }
        diesel::update(
            playback_progress::table.filter(playback_progress::media_item_id.eq(duplicate)),
        )
        .set(playback_progress::updated_at.eq(Some(20_i64)))
        .execute(&mut conn)
        .unwrap();
        diesel::insert_into(music_playlists::table)
            .values((
                music_playlists::id.eq(1),
                music_playlists::user_id.eq(1),
                music_playlists::name.eq("Saved queue"),
                music_playlists::created_at.eq(1_i64),
                music_playlists::updated_at.eq(1_i64),
                music_playlists::artwork_item_id.eq(Some(duplicate)),
            ))
            .execute(&mut conn)
            .unwrap();
        diesel::insert_into(music_playlist_entries::table)
            .values((
                music_playlist_entries::playlist_id.eq(1),
                music_playlist_entries::position.eq(0),
                music_playlist_entries::media_item_id.eq(duplicate),
            ))
            .execute(&mut conn)
            .unwrap();
        fixture.scan(&mut conn, &[fixture.settings("new", kind.clone())]);
        let after = list_media_items(&mut conn, None).unwrap();
        assert_eq!(
            after.iter().map(|item| item.id).collect::<HashSet<_>>(),
            original_ids,
            "{kind:?}"
        );
        for user in [1, 2] {
            assert_eq!(
                get_user_playback_progress(&mut conn, Some(user), original)
                    .unwrap()
                    .unwrap()
                    .position_ms,
                35000
            );
        }
        assert_eq!(
            item_metadata_links::table
                .find(link)
                .select(item_metadata_links::media_item_id)
                .first::<i32>(&mut conn)
                .unwrap(),
            root
        );
        assert_eq!(
            music_playlist_entries::table
                .select(music_playlist_entries::media_item_id)
                .first::<i32>(&mut conn)
                .unwrap(),
            original
        );
        assert_eq!(
            music_playlists::table
                .select(music_playlists::artwork_item_id)
                .first::<Option<i32>>(&mut conn)
                .unwrap(),
            Some(original)
        );
        assert_eq!(get_library_files(&mut conn, 1).unwrap().len(), 1);
        fixture.scan(&mut conn, &[fixture.settings("new", kind)]);
        assert_eq!(
            list_media_items(&mut conn, None).unwrap().len(),
            original_ids.len()
        );
    }
}

#[test]
fn configured_copies_and_ambiguous_replacements_are_not_silently_collapsed() {
    let fixture = Fixture::new();
    for root in [
        "first",
        "second",
        "replacement",
    ] {
        fixture.write(
            root,
            "Author/Book/01.mp3",
            b"identical file in distinct roots",
        );
    }
    let mut conn = database();
    let mut settings = fixture.settings("first", MediaLibraryKind::Books);
    settings.paths = vec![
        settings.path.clone(),
        fixture.0.join("second").to_string_lossy().into(),
    ];
    fixture.scan(&mut conn, &[settings.clone()]);
    let versions = list_media_items(&mut conn, None)
        .unwrap()
        .into_iter()
        .filter(|item| item.item_type == "audiobook")
        .collect::<Vec<_>>();
    assert_eq!(versions.len(), 2);
    fixture.scan(&mut conn, &[settings]);
    assert_eq!(
        list_media_items(&mut conn, None)
            .unwrap()
            .iter()
            .filter(|item| item.item_type == "audiobook")
            .count(),
        2
    );
    // Two possible originals for one replacement must not arbitrarily inherit either one's history.
    fixture.scan(
        &mut conn,
        &[fixture.settings("replacement", MediaLibraryKind::Books)],
    );
    let current = list_media_items(&mut conn, None)
        .unwrap()
        .into_iter()
        .find(|item| item.item_type == "audiobook")
        .unwrap();
    assert!(!versions.iter().any(|old| old.id == current.id));
    assert_eq!(get_library_files(&mut conn, 1).unwrap().len(), 1);
}

#[test]
fn a_missing_replacement_retains_history_and_a_later_successful_scan_recovers_it() {
    let fixture = Fixture::new();
    fixture.write("old", "Title/movie.mkv", b"original bytes");
    let mut conn = database();
    fixture.scan(
        &mut conn,
        &[fixture.settings("old", MediaLibraryKind::Movies)],
    );
    let original = list_media_items(&mut conn, None).unwrap()[0].id;
    seed_metadata(&mut conn, original);
    upsert_playback_progress(&mut conn, 1, original, 35000, Some(60000), false).unwrap();
    fixture.move_root("old", "new");
    let failed = fixture.scan(
        &mut conn,
        &[fixture.settings("typo", MediaLibraryKind::Movies)],
    );
    assert_eq!(failed[0].status, LibraryScanStatus::MissingPath);
    let missing = list_media_items(&mut conn, None).unwrap();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].id, original);
    assert!(missing[0].missing_since.is_some());
    fixture.scan(
        &mut conn,
        &[fixture.settings("new", MediaLibraryKind::Movies)],
    );
    let restored = list_media_items(&mut conn, None).unwrap();
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].id, original);
    assert!(restored[0].missing_since.is_none());
    assert_eq!(
        get_user_playback_progress(&mut conn, Some(1), original)
            .unwrap()
            .unwrap()
            .position_ms,
        35000
    );
}

#[test]
fn different_content_at_the_same_relative_path_does_not_inherit_the_old_item() {
    let fixture = Fixture::new();
    fixture.write("old", "Title/movie.mkv", b"first film");
    fixture.write("new", "Title/movie.mkv", b"other film");
    let mut conn = database();
    fixture.scan(
        &mut conn,
        &[fixture.settings("old", MediaLibraryKind::Movies)],
    );
    let old = list_media_items(&mut conn, None).unwrap()[0].id;
    upsert_playback_progress(&mut conn, 1, old, 10000, Some(60000), false).unwrap();
    fixture.scan(
        &mut conn,
        &[fixture.settings("new", MediaLibraryKind::Movies)],
    );
    let current = list_media_items(&mut conn, None).unwrap();
    assert_eq!(current.len(), 1);
    assert_ne!(current[0].id, old);
    assert!(
        get_user_playback_progress(&mut conn, Some(1), current[0].id)
            .unwrap()
            .is_none()
    );
    assert!(
        get_user_playback_progress(&mut conn, Some(1), old)
            .unwrap()
            .is_some()
    );
}

#[test]
fn moving_one_library_does_not_rewrite_a_physical_path_shared_with_another_library() {
    let fixture = Fixture::new();
    for root in ["old", "new"] {
        fixture.write(root, "Title/movie.mkv", b"same film");
    }
    let mut conn = database();
    let second = fixture.settings("old", MediaLibraryKind::Movies);
    fixture.scan(&mut conn, &[second.clone(), second.clone()]);
    let before = list_media_items(&mut conn, None).unwrap();
    let first_id = before.iter().find(|item| item.library_id == 1).unwrap().id;
    let second_id = before.iter().find(|item| item.library_id == 2).unwrap().id;
    fixture.scan(
        &mut conn,
        &[
            fixture.settings("new", MediaLibraryKind::Movies),
            second,
        ],
    );
    assert_eq!(list_media_items(&mut conn, None).unwrap().len(), 2);
    let first = load_catalog_file_for_item(&mut conn, first_id)
        .unwrap()
        .unwrap();
    let other = load_catalog_file_for_item(&mut conn, second_id)
        .unwrap()
        .unwrap();
    assert!(
        first
            .path
            .starts_with(fixture.0.join("new").to_str().unwrap())
    );
    assert!(
        other
            .path
            .starts_with(fixture.0.join("old").to_str().unwrap())
    );
    assert_eq!(
        media_files::table
            .count()
            .get_result::<i64>(&mut conn)
            .unwrap(),
        2
    );
}

#[test]
fn multiple_cd_recordings_keep_their_selected_narration_chapters_and_parts_after_a_move() {
    let fixture = Fixture::new();
    for (path, contents) in [
        ("Author/Book/Reader A/CD 1/Track 1.mp3", "part one"),
        ("Author/Book/Reader A/CD 2/Track 1.mp3", "part two"),
        ("Author/Book/Reader B/Book.m4b", "another narration"),
        ("Author/Book/Book.epub", "written work"),
    ] {
        fixture.write("old", path, contents.as_bytes());
    }
    let mut conn = database();
    fixture.scan(
        &mut conn,
        &[fixture.settings("old", MediaLibraryKind::Books)],
    );
    let before = list_media_items(&mut conn, None).unwrap();
    let version = before
        .iter()
        .find(|item| {
            item.item_type == "audiobook"
                && item.audiobook.as_ref().unwrap().version_label == "Reader A"
        })
        .unwrap()
        .id;
    let parts = get_media_item(&mut conn, version, "")
        .unwrap()
        .unwrap()
        .children
        .iter()
        .map(|part| part.id)
        .collect::<Vec<_>>();
    let snapshot = crate::metadata::StoredMetadataSnapshot {
        provider_id: MetadataProviderId::AudioSilo,
        external_id: "book/reader-a".into(),
        media_type: Some("audiobook".into()),
        title: Some("Book".into()),
        overview: None,
        artwork_url: None,
        backdrop_url: None,
        release_year: None,
        locale_key: String::new(),
        provider_locale_key: None,
        provider_payload_json: Some(
            serde_json::json!({"id": "book", "title": "Book",
                "authors": [{"name": "Author"}],
                "koko_recording": {"id": "reader-a", "narrators": [{"name": "Reader A"}]},
                "koko_chapters": [{"title": "Chapter", "start_ms": 35000, "end_ms": 120000}]
            })
            .to_string(),
        ),
    };
    crate::metadata::upsert_item_metadata_snapshot(&mut conn, version, &snapshot).unwrap();
    audiobooks::select_recording_source(
        &mut conn,
        version,
        &MetadataProviderId::AudioSilo,
        "book/reader-a",
    )
    .unwrap();
    upsert_playback_progress(&mut conn, 1, parts[1], 12345, Some(60000), false).unwrap();
    fixture.move_root("old", "new");
    fixture.scan(
        &mut conn,
        &[fixture.settings("new", MediaLibraryKind::Books)],
    );
    let mut after = get_media_item(&mut conn, version, "").unwrap().unwrap();
    assert_eq!(
        after
            .children
            .iter()
            .map(|part| part.id)
            .collect::<Vec<_>>(),
        parts
    );
    assert_eq!(after.audiobook_versions.len(), 2);
    let metadata = after.audiobook.as_ref().unwrap();
    assert_eq!(
        metadata.recording.as_ref().unwrap().external_id,
        "book/reader-a"
    );
    assert_eq!(metadata.narrator, "Reader A");
    assert_eq!(metadata.chapters[0].start_ms, 35000);
    apply_user_playback_context_to_detail(&mut conn, Some(1), &mut after).unwrap();
    assert_eq!(after.playback_target.unwrap().item_id, parts[1]);
    assert_eq!(
        list_media_items(&mut conn, None)
            .unwrap()
            .iter()
            .map(|item| item.id)
            .collect::<HashSet<_>>(),
        before.iter().map(|item| item.id).collect::<HashSet<_>>()
    );
}

#[test]
fn repair_preserves_a_recording_selected_on_the_new_duplicate() {
    let fixture = Fixture::new();
    fixture.write("old", "Author/Book/01.mp3", b"recording");
    let mut conn = database();
    fixture.scan(
        &mut conn,
        &[fixture.settings("old", MediaLibraryKind::Books)],
    );
    let original = list_media_items(&mut conn, None)
        .unwrap()
        .into_iter()
        .find(|item| item.item_type == "audiobook")
        .unwrap()
        .id;
    let mut snapshot = crate::metadata::StoredMetadataSnapshot {
        provider_id: MetadataProviderId::AudioSilo,
        external_id: "book".into(),
        media_type: Some("audiobook".into()),
        title: Some("Book".into()),
        overview: None,
        artwork_url: None,
        backdrop_url: None,
        release_year: None,
        locale_key: String::new(),
        provider_locale_key: None,
        provider_payload_json: Some(
            serde_json::json!({
                "id": "book", "title": "Book", "authors": [{"name": "Author"}]
            })
            .to_string(),
        ),
    };
    crate::metadata::upsert_item_metadata_snapshot(&mut conn, original, &snapshot).unwrap();
    fixture.move_root("old", "new");
    legacy_rescan(&mut conn, &fixture.settings("new", MediaLibraryKind::Books));
    let duplicate = list_media_items(&mut conn, None)
        .unwrap()
        .into_iter()
        .find(|item| item.item_type == "audiobook" && item.id != original)
        .unwrap()
        .id;
    snapshot.external_id = "book/reader-a".into();
    snapshot.provider_payload_json = Some(
        serde_json::json!({
            "id": "book", "title": "Book", "authors": [{"name": "Author"}],
            "koko_recording": {"id": "reader-a", "narrators": [{"name": "Reader A"}]},
            "koko_chapters": [{"title": "Opening", "start_ms": 0, "end_ms": 60000}]
        })
        .to_string(),
    );
    crate::metadata::upsert_item_metadata_snapshot(&mut conn, duplicate, &snapshot).unwrap();
    audiobooks::select_recording_source(
        &mut conn,
        duplicate,
        &MetadataProviderId::AudioSilo,
        "book/reader-a",
    )
    .unwrap();
    fixture.scan(
        &mut conn,
        &[fixture.settings("new", MediaLibraryKind::Books)],
    );
    let repaired = get_media_item(&mut conn, original, "").unwrap().unwrap();
    let metadata = repaired.audiobook.unwrap();
    assert_eq!(metadata.recording.unwrap().external_id, "book/reader-a");
    assert_eq!(metadata.narrator, "Reader A");
    assert_eq!(metadata.chapters.len(), 1);
    assert_eq!(
        list_media_items(&mut conn, None)
            .unwrap()
            .iter()
            .filter(|item| item.item_type == "audiobook")
            .count(),
        1
    );
}

#[test]
fn overlapping_roots_count_one_physical_file_and_preserve_its_existing_item() {
    let fixture = Fixture::new();
    fixture.write("media", "Author/Book/01.mp3", b"one recording");
    let mut conn = database();
    let mut settings = fixture.settings("media", MediaLibraryKind::Books);
    fixture.scan(&mut conn, &[settings.clone()]);
    let before = list_media_items(&mut conn, None)
        .unwrap()
        .iter()
        .map(|item| item.id)
        .collect::<HashSet<_>>();
    settings.paths = vec![
        settings.path.clone(),
        fixture.0.join("media/Author").to_string_lossy().into(),
    ];
    let summary = fixture.scan(&mut conn, &[settings.clone()]);
    assert_eq!(summary[0].total_files, 1);
    assert_eq!(summary[0].audio_files, 1);
    assert_eq!(get_library_files(&mut conn, 1).unwrap().len(), 1);
    assert_eq!(
        list_media_items(&mut conn, None)
            .unwrap()
            .iter()
            .map(|item| item.id)
            .collect::<HashSet<_>>(),
        before
    );
    // A move into overlapping replacement roots must also have one destination, not two.
    fixture.move_root("media", "moved");
    settings.path = fixture.0.join("moved").to_string_lossy().into();
    settings.paths = vec![
        settings.path.clone(),
        fixture.0.join("moved/Author").to_string_lossy().into(),
    ];
    let summary = fixture.scan(&mut conn, &[settings]);
    assert_eq!(summary[0].total_files, 1);
    assert_eq!(
        list_media_items(&mut conn, None)
            .unwrap()
            .iter()
            .map(|item| item.id)
            .collect::<HashSet<_>>(),
        before
    );
}

#[test]
fn unique_content_can_reconnect_a_renamed_file_without_resetting_its_metadata_or_progress() {
    let fixture = Fixture::new();
    fixture.write("old", "Title/movie.mkv", b"unchanged unique content");
    fixture.write(
        "new",
        "Renamed Title/renamed.mkv",
        b"unchanged unique content",
    );
    let mut conn = database();
    fixture.scan(
        &mut conn,
        &[fixture.settings("old", MediaLibraryKind::Movies)],
    );
    let original = list_media_items(&mut conn, None).unwrap()[0].id;
    let link = seed_metadata(&mut conn, original);
    upsert_playback_progress(&mut conn, 1, original, 35000, Some(60000), false).unwrap();
    fixture.scan(
        &mut conn,
        &[fixture.settings("new", MediaLibraryKind::Movies)],
    );
    let items = list_media_items(&mut conn, None).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, original);
    assert_eq!(items[0].relative_path, "Renamed Title/renamed.mkv");
    assert_eq!(items[0].display_title, "Saved metadata");
    assert_eq!(
        item_metadata_links::table
            .find(link)
            .select(item_metadata_links::media_item_id)
            .first::<i32>(&mut conn)
            .unwrap(),
        original
    );
    assert_eq!(
        get_user_playback_progress(&mut conn, Some(1), original)
            .unwrap()
            .unwrap()
            .position_ms,
        35000
    );
}
