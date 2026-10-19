use std::fs;
use std::path::PathBuf;

use diesel::{
    Connection,
    RunQueryDsl,
    SqliteConnection,
    sql_types,
};
use koko::config::{
    FfmpegSettings,
    MediaLibraryKind,
    MediaLibrarySettings,
    MetadataProviderId,
};
use koko::media::{
    get_library_files,
    get_media_item,
    get_playback_decision,
    list_automatic_metadata_candidates,
    list_automatic_metadata_refresh_candidates,
    list_media_items,
    mark_metadata_match_attempted,
    search_media_items,
    sync_library_catalog,
};
use koko::metadata::{
    StoredMetadataSnapshot,
    clear_item_metadata_tree,
    upsert_item_metadata_snapshot,
};

struct MusicLibrary(PathBuf);

#[test]
fn continue_listening_groups_albums_and_resumes_the_latest_track_for_each_user() {
    let fixture = MusicLibrary::new();
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    koko::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    diesel::sql_query(
        "INSERT INTO users (id,username,password,admin) VALUES \
         (1,'alice','hash',1),(2,'bob','hash',1)",
    )
    .execute(&mut conn)
    .unwrap();
    let library = MediaLibrarySettings {
        name: "Music".into(),
        path: fixture.0.to_string_lossy().into(),
        kind: MediaLibraryKind::Music,
        recursive: true,
        ..Default::default()
    };
    sync_library_catalog(
        &mut conn,
        &[library],
        &FfmpegSettings {
            ffprobe_path: "missing-test-ffprobe".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let items = list_media_items(&mut conn, None).unwrap();
    let album = items.iter().find(|item| item.item_type == "album").unwrap();
    let detail = get_media_item(&mut conn, album.id, "").unwrap().unwrap();
    let first = detail.children[0].id;
    let second = detail.children[1].id;
    koko::media::upsert_playback_progress(&mut conn, 1, first, 5000, Some(120000), false).unwrap();
    let home = koko::media::get_media_home(&mut conn, Some(1), Some(album.library_id)).unwrap();
    assert_eq!(home.shelves[0].title, "Continue listening");
    assert_eq!(home.shelves[0].items.len(), 1);
    assert_eq!(home.shelves[0].items[0].id, album.id);
    assert_eq!(home.shelves[0].items[0].item_type, "album");
    assert!(
        !koko::media::get_media_home(&mut conn, Some(2), None)
            .unwrap()
            .shelves[0]
            .items
            .iter()
            .any(|item| item.id == album.id)
    );
    let mut detail = detail;
    koko::media::apply_user_playback_context_to_detail(&mut conn, Some(1), &mut detail).unwrap();
    let target = detail.playback_target.unwrap();
    assert_eq!((target.item_id, target.start_ms), (first, 5000));
    assert_eq!(target.label, "Continue album");
    koko::media::upsert_playback_progress(&mut conn, 1, first, 120000, Some(120000), true).unwrap();
    let mut detail = get_media_item(&mut conn, album.id, "").unwrap().unwrap();
    koko::media::apply_user_playback_context_to_detail(&mut conn, Some(1), &mut detail).unwrap();
    let target = detail.playback_target.unwrap();
    assert_eq!((target.item_id, target.start_ms), (second, 0));
    koko::media::upsert_playback_progress(&mut conn, 1, second, 18000, Some(120000), false)
        .unwrap();
    // The most recent track wins even if an older track still has unfinished progress.
    diesel::sql_query("UPDATE playback_progress SET updated_at=1 WHERE media_item_id=?")
        .bind::<sql_types::Integer, _>(first)
        .execute(&mut conn)
        .unwrap();
    let mut detail = get_media_item(&mut conn, album.id, "").unwrap().unwrap();
    koko::media::apply_user_playback_context_to_detail(&mut conn, Some(1), &mut detail).unwrap();
    let target = detail.playback_target.unwrap();
    assert_eq!((target.item_id, target.start_ms), (second, 18000));
    assert_eq!(
        koko::media::get_media_home(&mut conn, Some(1), None)
            .unwrap()
            .shelves[0]
            .items
            .len(),
        1
    );
    koko::media::upsert_playback_progress(&mut conn, 1, second, 120000, Some(120000), true)
        .unwrap();
    assert!(
        koko::media::get_media_home(&mut conn, Some(1), None)
            .unwrap()
            .shelves[0]
            .items
            .is_empty()
    );
    // Missing tracks never become resume targets.
    diesel::sql_query("UPDATE media_items SET missing_since=1 WHERE id=?")
        .bind::<sql_types::Integer, _>(second)
        .execute(&mut conn)
        .unwrap();
    let mut detail = get_media_item(&mut conn, album.id, "").unwrap().unwrap();
    koko::media::apply_user_playback_context_to_detail(&mut conn, Some(1), &mut detail).unwrap();
    assert!(detail.playback_target.is_none());
}

impl MusicLibrary {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "koko-music-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        fs::create_dir_all(root.join("Folder Artist/Folder Album/Disc 1")).unwrap();
        fs::create_dir_all(root.join("Folder Artist/Folder Album/Disc 2")).unwrap();
        fs::write(
            root.join("Folder Artist/Folder Album/Disc 1/02 Second.flac"),
            b"audio one",
        )
        .unwrap();
        fs::write(
            root.join("Folder Artist/Folder Album/Disc 2/01 First.flac"),
            b"audio two",
        )
        .unwrap();
        fs::write(root.join("ignore.mp4"), b"video").unwrap();
        Self(root)
    }
}

impl Drop for MusicLibrary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn album_named_disc_folders_form_one_album_and_keep_track_ids_on_rescan() {
    let fixture = MusicLibrary::new();
    for disc in 1..=2 {
        let directory = fixture.0.join(format!(
            "The Beach Boys/Pet Sounds/Pet Sounds - CD {disc:02}"
        ));
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("01 Song.flac"), b"audio").unwrap();
    }
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    koko::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    let library = MediaLibrarySettings {
        name: "Music".into(),
        path: fixture.0.to_string_lossy().into(),
        kind: MediaLibraryKind::Music,
        recursive: true,
        ..Default::default()
    };
    let ffmpeg = FfmpegSettings {
        ffprobe_path: "missing-music-test-ffprobe".into(),
        ..Default::default()
    };
    let mut previous = None;
    for _ in 0..2 {
        sync_library_catalog(&mut conn, std::slice::from_ref(&library), &ffmpeg).unwrap();
        let items = list_media_items(&mut conn, None).unwrap();
        let artist = items
            .iter()
            .find(|item| item.item_type == "artist" && item.display_title == "The Beach Boys")
            .unwrap();
        let albums = items
            .iter()
            .filter(|item| item.parent_id == Some(artist.id))
            .collect::<Vec<_>>();
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].display_title, "Pet Sounds");
        assert_eq!(albums[0].child_count, 2);
        let mut ids = items
            .iter()
            .filter(|item| item.parent_id == Some(albums[0].id))
            .map(|item| item.id)
            .collect::<Vec<_>>();
        ids.sort();
        if let Some(previous) = previous {
            assert_eq!(ids, previous);
        }
        previous = Some(ids);
    }
}

#[test]
fn music_library_scans_tags_matches_and_preserves_track_identity() {
    let fixture = MusicLibrary::new();
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    koko::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    let library = MediaLibrarySettings {
        use_embedded_music_tags: true,
        name: "Music".into(),
        path: fixture.0.to_string_lossy().into(),
        paths: vec![],
        recursive: true,
        kind: MediaLibraryKind::Music,
        scanner: Default::default(),
        metadata_providers: vec![],
        metadata_language_mode: Default::default(),
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    };
    let ffmpeg = FfmpegSettings {
        ffprobe_path: "missing-music-test-ffprobe".into(),
        ..Default::default()
    };
    let libraries =
        sync_library_catalog(&mut conn, std::slice::from_ref(&library), &ffmpeg).unwrap();
    #[derive(diesel::QueryableByName)]
    struct Count {
        #[diesel(sql_type = sql_types::BigInt)]
        count: i64,
    }
    for table in [
        "music_file_metadata",
        "music_item_metadata",
        "music_file_genres",
        "music_item_genres",
    ] {
        let count = diesel::sql_query(format!(
            "SELECT COUNT(*) AS count FROM pragma_table_info('{table}') WHERE name LIKE '%json%'"
        ))
        .get_result::<Count>(&mut conn)
        .unwrap();
        assert_eq!(count.count, 0);
    }
    let raw = diesel::sql_query(
        "SELECT COUNT(*) AS count FROM media_files WHERE media_kind = 'audio' AND metadata_json \
         IS NOT NULL",
    )
    .get_result::<Count>(&mut conn)
    .unwrap();
    assert_eq!(raw.count, 0);

    let persisted_libraries = koko::media::get_persisted_library_summaries(&mut conn).unwrap();
    let library_id = libraries[0].id;
    assert_eq!(
        persisted_libraries[0].metadata_providers,
        vec![MetadataProviderId::MusicBrainz]
    );
    let before = list_media_items(&mut conn, Some(library_id)).unwrap();
    assert_eq!(before.len(), 4);
    let tracks_before = before
        .iter()
        .filter(|item| item.item_type == "track")
        .map(|item| (item.relative_path.clone(), item.id))
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(
        before
            .iter()
            .find(|item| item.item_type == "artist")
            .unwrap()
            .display_title,
        "Folder Artist"
    );
    let release_id = "f5093c06-23e3-404f-aeaa-40f72885ee3a";
    for file in get_library_files(&mut conn, library_id).unwrap() {
        let disc = if file.relative_path.contains("Disc 1") { 1 } else { 2 };
        let number = if disc == 1 { 2 } else { 1 };
        diesel::sql_query(
            "UPDATE music_file_metadata SET title = ?, artist = ?, album_artist = 'Album Artist', \
             album = 'Tagged Album', track_number = ?, disc_number = ?, release_year = 2020, \
             musicbrainz_release_id = ? WHERE media_file_id = ?",
        )
        .bind::<sql_types::Text, _>(format!("Tagged track {disc}"))
        .bind::<sql_types::Text, _>(format!("Guest {disc}"))
        .bind::<sql_types::Integer, _>(number)
        .bind::<sql_types::Integer, _>(disc)
        .bind::<sql_types::Text, _>(release_id)
        .bind::<sql_types::Integer, _>(file.id)
        .execute(&mut conn)
        .unwrap();
        diesel::sql_query(
            "UPDATE media_files SET duration_ms = 120000, container = 'flac', audio_codec = \
             'flac' WHERE id = ?",
        )
        .bind::<sql_types::Integer, _>(file.id)
        .execute(&mut conn)
        .unwrap();
        for genre in ["Rock", "Pop"] {
            diesel::sql_query("INSERT INTO music_file_genres (media_file_id,genre) VALUES (?,?)")
                .bind::<sql_types::Integer, _>(file.id)
                .bind::<sql_types::Text, _>(genre)
                .execute(&mut conn)
                .unwrap();
        }
    }
    sync_library_catalog(&mut conn, std::slice::from_ref(&library), &ffmpeg).unwrap();
    let items = list_media_items(&mut conn, Some(library_id)).unwrap();
    assert_eq!(items.len(), 4);
    let artist = items
        .iter()
        .find(|item| item.item_type == "artist")
        .unwrap();
    assert_eq!(artist.display_title, "Album Artist");
    let album = items.iter().find(|item| item.item_type == "album").unwrap();
    assert_eq!(album.parent_id, Some(artist.id));
    assert_eq!(album.display_title, "Tagged Album");
    let album_id = album.id;
    for track in items.iter().filter(|item| item.item_type == "track") {
        assert_eq!(track.id, tracks_before[&track.relative_path]);
        assert_eq!(track.parent_id, Some(album_id));
        assert_eq!(
            track
                .music
                .as_ref()
                .unwrap()
                .musicbrainz_release_id
                .as_deref(),
            Some(release_id)
        );
        assert_eq!(
            get_playback_decision(&mut conn, track.id, None)
                .unwrap()
                .unwrap()
                .source_audio_codec
                .as_deref(),
            Some("flac")
        );
    }
    let detail = get_media_item(&mut conn, album_id, "").unwrap().unwrap();
    assert_eq!(
        detail
            .children
            .iter()
            .map(|track| track.music.as_ref().unwrap().disc_number)
            .collect::<Vec<_>>(),
        vec![Some(1), Some(2)]
    );
    assert!(!detail.playable);
    assert_eq!(detail.duration_ms, Some(240000));
    assert_eq!(
        search_media_items(&mut conn, "Tagged", Some(library_id))
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        list_automatic_metadata_candidates(&mut conn, Some(library_id), 100)
            .unwrap()
            .len(),
        4
    );
    diesel::sql_query(
        "UPDATE media_items SET modified_at = CASE item_type WHEN 'artist' THEN 1 WHEN 'album' \
         THEN 2 ELSE 100 + id END WHERE library_id = ?",
    )
    .bind::<sql_types::Integer, _>(library_id)
    .execute(&mut conn)
    .unwrap();
    let queue = list_automatic_metadata_candidates(&mut conn, Some(library_id), 100).unwrap();
    assert_eq!(
        queue
            .iter()
            .map(|item| item.item_type.as_str())
            .collect::<Vec<_>>(),
        vec![
            "artist", "album", "track", "track"
        ]
    );
    assert_eq!(queue[2].music.as_ref().unwrap().disc_number, Some(1));
    assert_eq!(queue[3].music.as_ref().unwrap().disc_number, Some(2));
    mark_metadata_match_attempted(&mut conn, artist.id, 1).unwrap();
    assert_eq!(
        list_automatic_metadata_candidates(&mut conn, Some(library_id), 100)
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        list_automatic_metadata_refresh_candidates(&mut conn, Some(library_id), 100)
            .unwrap()
            .len(),
        4
    );
    let snapshot = StoredMetadataSnapshot {
        provider_id: MetadataProviderId::MusicBrainz,
        external_id: release_id.into(),
        media_type: Some("release".into()),
        title: Some("MusicBrainz Album".into()),
        overview: Some("Release disambiguation".into()),
        artwork_url: Some(format!(
            "https://coverartarchive.org/release/{release_id}/front-500"
        )),
        backdrop_url: None,
        release_year: Some(2020),
        locale_key: "en-US".into(),
        provider_locale_key: None,
        provider_payload_json: Some(serde_json::json!({"genres": [{"name": "rock"}]}).to_string()),
    };
    upsert_item_metadata_snapshot(&mut conn, album_id, &snapshot).unwrap();
    let detail = get_media_item(&mut conn, album_id, "").unwrap().unwrap();
    assert_eq!(detail.display_title, "MusicBrainz Album");
    assert_eq!(detail.genres, vec!["rock"]);
    assert!(
        detail
            .poster_url
            .unwrap()
            .contains(&format!("/items/{album_id}/artwork"))
    );
    clear_item_metadata_tree(&mut conn, artist.id, true).unwrap();
    assert!(
        list_automatic_metadata_refresh_candidates(&mut conn, Some(library_id), 100)
            .unwrap()
            .is_empty()
    );
    sync_library_catalog(&mut conn, std::slice::from_ref(&library), &ffmpeg).unwrap();
    assert!(
        list_automatic_metadata_candidates(&mut conn, Some(library_id), 100)
            .unwrap()
            .is_empty()
    );
    let mut excluded = library.clone();
    excluded.name = "Ignore tags".into();
    excluded.use_embedded_music_tags = false;
    let libraries = sync_library_catalog(&mut conn, &[library, excluded], &ffmpeg).unwrap();
    let excluded_items = list_media_items(&mut conn, Some(libraries[1].id)).unwrap();
    assert_eq!(
        excluded_items
            .iter()
            .find(|item| item.item_type == "artist")
            .unwrap()
            .display_title,
        "Folder Artist"
    );
    assert!(excluded_items.iter().all(|item| {
        item.music
            .as_ref()
            .unwrap()
            .musicbrainz_release_id
            .is_none()
    }));
    assert!(
        !serde_json::from_str::<MediaLibrarySettings>("{}")
            .unwrap()
            .use_embedded_music_tags
    );
    clear_item_metadata_tree(&mut conn, artist.id, false).unwrap();
    assert_eq!(
        list_automatic_metadata_candidates(&mut conn, Some(library_id), 100)
            .unwrap()
            .len(),
        4
    );
}

#[test]
fn music_transcode_excludes_embedded_video_and_keeps_selected_audio() {
    let spec = koko::transcode::TranscodeSpec {
        source_path: "music.flac".into(),
        audio_only: true,
        output_path: "output.mp4".into(),
        container: "mp4".into(),
        video_codec: None,
        audio_codec: Some("aac".into()),
        max_width: None,
        max_height: None,
        max_bitrate_kbps: None,
        start_time_ms: Some(30000),
        audio_stream_index: Some(1),
    };
    let args = spec.to_ffmpeg_stdout_args();
    assert!(args.iter().any(|arg| arg == "-vn"));
    assert!(!args.iter().any(|arg| arg.starts_with("0:v:")));
    assert!(args.windows(2).any(|pair| pair == ["-map", "0:a:1?"]));
    assert!(args.windows(2).any(|pair| pair == ["-ss", "30.000"]));
    assert_eq!(args.last().unwrap(), "pipe:1");
}
