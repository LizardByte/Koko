// standard imports
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

// lib imports
use diesel::Connection;
use diesel::RunQueryDsl;
use diesel::SqliteConnection;
use diesel::connection::SimpleConnection;
use diesel_migrations::MigrationHarness;

// local imports
use koko::config::{FfmpegSettings, MediaLibraryKind, MediaLibrarySettings, MetadataProviderId};
use koko::db::{MIGRATIONS, reconcile_legacy_sqlite_schema};
use koko::media::{
    LibraryScanStatus, get_item_youtube_theme_collection_references,
    get_item_youtube_theme_provider_references, get_library_files, get_media_home,
    get_media_home_with_preferred_languages, get_media_item, get_persisted_library_summaries,
    get_preferred_item_metadata_link, infer_episode_number, infer_season_number, inspect_libraries,
    inspect_transcoding_capability, list_automatic_metadata_candidates, list_library_settings,
    list_media_items, remove_library_setting, replace_library_settings,
    resolve_local_item_artwork_path, resolve_media_item_source_path, search_media_items,
    sync_library_catalog, upsert_playback_progress,
};
use koko::metadata::{
    ArtworkKind, StoredMetadataSnapshot, get_preferred_item_metadata_link_for_languages,
    get_primary_item_metadata_link, set_item_metadata_refresh_state, upsert_item_metadata_snapshot,
    upsert_secondary_collection_theme_song_url, upsert_secondary_youtube_theme_metadata_link,
};

static MEDIA_TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_temp_dir(name: &str) -> PathBuf {
    let test_id = MEDIA_TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    std::env::temp_dir().join(format!("koko_{}_{}_{}", name, test_id, timestamp))
}

fn create_test_connection(name: &str) -> (SqliteConnection, PathBuf) {
    let db_path = unique_temp_dir(name).with_extension("db");
    let mut connection = SqliteConnection::establish(&db_path.to_string_lossy())
        .expect("Failed to establish SQLite test connection");

    connection
        .run_pending_migrations(MIGRATIONS)
        .expect("Failed to run test migrations");

    (connection, db_path)
}

#[test]
fn test_inspect_libraries_counts_media_types() {
    let root = unique_temp_dir("library_scan");
    let nested = root.join("nested");
    fs::create_dir_all(&nested).unwrap();

    fs::write(root.join("movie.mkv"), b"video").unwrap();
    fs::write(root.join("song.mp3"), b"audio").unwrap();
    fs::write(root.join("cover.jpg"), b"image").unwrap();
    fs::write(root.join("book.epub"), b"book").unwrap();
    fs::write(nested.join("notes.txt"), b"other").unwrap();
    fs::write(nested.join("episode.mp4"), b"video").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Primary library".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Mixed,
        metadata_providers: vec![],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let summaries = inspect_libraries(&libraries);
    assert_eq!(summaries.len(), 1);

    let summary = &summaries[0];
    assert_eq!(summary.status, LibraryScanStatus::Available);
    assert_eq!(summary.total_files, 5);
    assert_eq!(summary.video_files, 2);
    assert_eq!(summary.audio_files, 1);
    assert_eq!(summary.image_files, 1);
    assert_eq!(summary.book_files, 1);
    assert_eq!(summary.other_files, 0);
    assert!(summary.error.is_none());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn test_inspect_libraries_detects_missing_and_empty_paths() {
    let missing_path = unique_temp_dir("missing_library")
        .to_string_lossy()
        .to_string();
    let libraries = vec![
        MediaLibrarySettings {
            name: "Empty".into(),
            path: String::new(),
            paths: vec![],
            recursive: true,
            kind: MediaLibraryKind::Mixed,
            metadata_providers: vec![],
            metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
            metadata_languages: vec![],
            allowed_user_ids: vec![],
        },
        MediaLibrarySettings {
            name: "Missing".into(),
            path: missing_path.clone(),
            paths: vec![missing_path],
            recursive: true,
            kind: MediaLibraryKind::Movies,
            metadata_providers: vec![],
            metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
            metadata_languages: vec![],
            allowed_user_ids: vec![],
        },
    ];

    let summaries = inspect_libraries(&libraries);
    assert_eq!(summaries[0].status, LibraryScanStatus::EmptyPath);
    assert_eq!(summaries[1].status, LibraryScanStatus::MissingPath);
}

#[test]
fn test_movie_library_ignores_sidecar_audio_and_json_files() {
    let root = unique_temp_dir("movie_library_filtering");
    fs::create_dir_all(&root).unwrap();

    fs::write(root.join("movie.mkv"), b"video").unwrap();
    fs::write(root.join("theme.mp3"), b"audio").unwrap();
    fs::write(root.join("movie.json"), b"metadata").unwrap();
    fs::write(root.join("poster.jpg"), b"image").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Movies".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Movies,
        metadata_providers: vec![],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let summaries = inspect_libraries(&libraries);
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].total_files, 1);
    assert_eq!(summaries[0].video_files, 1);
    assert_eq!(summaries[0].audio_files, 0);
    assert_eq!(summaries[0].image_files, 0);
    assert_eq!(summaries[0].other_files, 0);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn test_inspect_transcoding_capability_reports_missing_binary() {
    let settings = FfmpegSettings {
        ffmpeg_path: "koko-ffmpeg-missing-binary".into(),
        ffprobe_path: "koko-ffprobe-missing-binary".into(),
        ..FfmpegSettings::default()
    };

    let capability = inspect_transcoding_capability(&settings);
    assert!(!capability.ffmpeg.available);
    assert!(!capability.ffprobe.available);
    assert!(capability.ffmpeg.error.is_some());
    assert!(capability.ffprobe.error.is_some());
}

#[test]
fn test_sync_library_catalog_persists_library_and_inventory() {
    let root = unique_temp_dir("persist_library_scan");
    let nested = root.join("nested");
    fs::create_dir_all(&nested).unwrap();

    fs::write(root.join("movie.mkv"), b"video").unwrap();
    fs::write(root.join("song.mp3"), b"audio").unwrap();
    fs::write(nested.join("episode.mp4"), b"video").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Persistent library".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Mixed,
        metadata_providers: vec![],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) = create_test_connection("media_catalog");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();

    assert_eq!(persisted.len(), 1);
    let library = &persisted[0];
    assert!(library.id > 0);
    assert_eq!(library.status, LibraryScanStatus::Available);
    assert_eq!(library.total_files, 3);
    assert_eq!(library.video_files, 2);
    assert_eq!(library.audio_files, 1);

    let files = get_library_files(&mut connection, library.id).unwrap();
    assert_eq!(files.len(), 3);
    assert_eq!(files[0].library_id, library.id);
    assert!(files.iter().any(|file| file.relative_path == "movie.mkv"));
    assert!(files.iter().any(|file| file.relative_path == "song.mp3"));
    assert!(
        files
            .iter()
            .any(|file| file.relative_path == "nested/episode.mp4")
    );

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_sync_library_catalog_updates_incrementally() {
    let root = unique_temp_dir("incremental_library_scan");
    fs::create_dir_all(&root).unwrap();

    fs::write(root.join("movie.mkv"), b"original-video").unwrap();
    fs::write(root.join("song.mp3"), b"audio").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Incremental library".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Mixed,
        metadata_providers: vec![],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) = create_test_connection("media_catalog_incremental");
    let first_sync =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let first_library = &first_sync[0];
    let first_files = get_library_files(&mut connection, first_library.id).unwrap();
    let first_movie = first_files
        .iter()
        .find(|file| file.relative_path == "movie.mkv")
        .unwrap();

    fs::remove_file(root.join("song.mp3")).unwrap();
    fs::write(root.join("movie.mkv"), b"updated-video-content").unwrap();
    fs::write(root.join("cover.jpg"), b"image").unwrap();

    let second_sync =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let second_library = &second_sync[0];
    let second_files = get_library_files(&mut connection, second_library.id).unwrap();
    let second_movie = second_files
        .iter()
        .find(|file| file.relative_path == "movie.mkv")
        .unwrap();

    assert_eq!(
        second_library.scan_revision,
        first_library.scan_revision + 1
    );
    assert_eq!(second_library.total_files, 2);
    assert_eq!(second_library.video_files, 1);
    assert_eq!(second_library.image_files, 1);
    assert_eq!(second_files.len(), 2);
    assert_eq!(first_movie.id, second_movie.id);
    assert!(
        second_files
            .iter()
            .any(|file| file.relative_path == "cover.jpg")
    );
    assert!(
        !second_files
            .iter()
            .any(|file| file.relative_path == "song.mp3")
    );

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_item_queries_and_search_work_on_persisted_catalog() {
    let root = unique_temp_dir("item_query_library_scan");
    let nested = root.join("nested");
    fs::create_dir_all(&nested).unwrap();

    fs::write(root.join("movie.mkv"), b"video").unwrap();
    fs::write(root.join("song.mp3"), b"audio").unwrap();
    fs::write(nested.join("episode.mp4"), b"video").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Query library".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Mixed,
        metadata_providers: vec![],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) = create_test_connection("media_catalog_item_queries");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let library = &persisted[0];

    let items = list_media_items(&mut connection, Some(library.id)).unwrap();
    assert_eq!(items.len(), 3);
    assert!(items.iter().any(|item| item.display_title == "movie"));

    let movie = items
        .iter()
        .find(|item| item.relative_path == "movie.mkv")
        .unwrap();
    let detail = get_media_item(&mut connection, movie.id, &root.to_string_lossy())
        .unwrap()
        .expect("Expected movie detail to exist");
    assert_eq!(detail.display_title, "movie");
    assert_eq!(detail.relative_path, "movie.mkv");

    let search_results = search_media_items(&mut connection, "episode", Some(library.id)).unwrap();
    assert_eq!(search_results.len(), 1);
    assert_eq!(search_results[0].relative_path, "nested/episode.mp4");

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_shows_library_builds_show_season_episode_hierarchy() {
    let root = unique_temp_dir("shows_library_hierarchy");
    let season = root.join("Mock Show").join("Season 1");
    fs::create_dir_all(&season).unwrap();
    fs::write(season.join("Mock Show - S01E01 - Pilot.mkv"), b"video").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Shows".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Shows,
        metadata_providers: vec![MetadataProviderId::Tmdb],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) = create_test_connection("shows_library_hierarchy_db");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let library = &persisted[0];
    let items = list_media_items(&mut connection, Some(library.id)).unwrap();

    assert_eq!(items.len(), 3);

    let show = items.iter().find(|item| item.item_type == "show").unwrap();
    let season = items
        .iter()
        .find(|item| item.item_type == "season")
        .unwrap();
    let episode = items
        .iter()
        .find(|item| item.item_type == "episode")
        .unwrap();

    assert_eq!(show.parent_id, None);
    assert_eq!(show.child_count, 1);
    assert_eq!(season.parent_id, Some(show.id));
    assert_eq!(season.child_count, 1);
    assert_eq!(episode.parent_id, Some(season.id));
    assert!(episode.playable);
    assert_eq!(episode.season_number, Some(1));
    assert_eq!(episode.episode_number, Some(1));

    let show_detail = get_media_item(&mut connection, show.id, &root.to_string_lossy())
        .unwrap()
        .expect("Expected show detail to exist");
    assert_eq!(show_detail.children.len(), 1);
    assert_eq!(show_detail.children[0].id, season.id);

    let season_detail = get_media_item(&mut connection, season.id, &root.to_string_lossy())
        .unwrap()
        .expect("Expected season detail to exist");
    assert_eq!(season_detail.hierarchy.len(), 1);
    assert_eq!(season_detail.hierarchy[0].id, show.id);
    assert_eq!(season_detail.children.len(), 1);
    assert_eq!(season_detail.children[0].id, episode.id);

    let episode_detail = get_media_item(&mut connection, episode.id, &root.to_string_lossy())
        .unwrap()
        .expect("Expected episode detail to exist");
    assert_eq!(episode_detail.hierarchy.len(), 2);
    assert_eq!(episode_detail.hierarchy[0].id, show.id);
    assert_eq!(episode_detail.hierarchy[1].id, season.id);

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_episode_number_parser_handles_show_titles_before_sxxexx() {
    assert_eq!(
        infer_episode_number("Marvel's Agents of S H I E L D (2013) - S03E01 - Laws of Nature.mkv"),
        Some(1)
    );
    assert_eq!(
        infer_episode_number("Marvel's Agents of S H I E L D (2013) - 3x22 - Ascension.mkv"),
        Some(22)
    );
    assert_eq!(
        infer_season_number("Marvel's Agents of S H I E L D (2013) - S03E01 - Laws of Nature.mkv"),
        Some(3)
    );
}

#[test]
fn test_shows_are_included_in_automatic_metadata_candidates() {
    let root = unique_temp_dir("automatic_show_metadata_candidates");
    let season = root.join("Mock Show").join("Season 1");
    fs::create_dir_all(&season).unwrap();
    fs::write(season.join("Mock Show - S01E01 - Pilot.mkv"), b"video").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Shows".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Shows,
        metadata_providers: vec![MetadataProviderId::Tmdb],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) = create_test_connection("automatic_show_metadata_candidates_db");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let library = &persisted[0];
    let show = list_media_items(&mut connection, Some(library.id))
        .unwrap()
        .into_iter()
        .find(|item| item.item_type == "show")
        .expect("Expected show item to exist");

    let candidates = list_automatic_metadata_candidates(&mut connection, None, 8).unwrap();
    assert!(candidates.iter().any(|candidate| {
        candidate.item_id == show.id
            && candidate.display_title == show.display_title
            && candidate.library_kind == MediaLibraryKind::Shows
    }));

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_show_recently_added_collapses_to_episode_season_or_show() {
    let root = unique_temp_dir("show_recently_added_collapsed");
    let alpha = root.join("Alpha Show").join("Season 1");
    let beta = root.join("Beta Show").join("Season 1");
    let gamma_season_1 = root.join("Gamma Show").join("Season 1");
    let gamma_season_2 = root.join("Gamma Show").join("Season 2");
    fs::create_dir_all(&alpha).unwrap();
    fs::create_dir_all(&beta).unwrap();
    fs::create_dir_all(&gamma_season_1).unwrap();
    fs::create_dir_all(&gamma_season_2).unwrap();

    fs::write(alpha.join("Alpha Show - S01E01 - Pilot.mkv"), b"video").unwrap();
    fs::write(beta.join("Beta Show - S01E01 - Pilot.mkv"), b"video").unwrap();
    fs::write(beta.join("Beta Show - S01E02 - Second.mkv"), b"video").unwrap();
    fs::write(
        gamma_season_1.join("Gamma Show - S01E01 - Pilot.mkv"),
        b"video",
    )
    .unwrap();
    fs::write(
        gamma_season_2.join("Gamma Show - S02E01 - Return.mkv"),
        b"video",
    )
    .unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Shows".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Shows,
        metadata_providers: vec![MetadataProviderId::Tmdb],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) = create_test_connection("show_recently_added_collapsed_db");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let library = &persisted[0];
    let home = get_media_home(&mut connection, None, Some(library.id)).unwrap();
    let recently_added = home
        .shelves
        .iter()
        .find(|shelf| shelf.id == "recently_added")
        .expect("Expected recently added shelf");

    assert_eq!(recently_added.items.len(), 3);
    assert_eq!(
        recently_added
            .items
            .iter()
            .filter(|item| item.item_type == "episode")
            .count(),
        1
    );
    assert_eq!(
        recently_added
            .items
            .iter()
            .filter(|item| item.item_type == "season")
            .count(),
        1
    );
    assert_eq!(
        recently_added
            .items
            .iter()
            .filter(|item| item.item_type == "show")
            .count(),
        1
    );

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_home_includes_real_collection_summaries() {
    let root = unique_temp_dir("home_collection_summaries");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("collection-one.mkv"), b"video").unwrap();
    fs::write(root.join("collection-two.mkv"), b"video").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Movies".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Movies,
        metadata_providers: vec![MetadataProviderId::Tmdb],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) = create_test_connection("home_collection_summaries_db");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let library = &persisted[0];
    let items = list_media_items(&mut connection, Some(library.id)).unwrap();

    for item in items.iter().take(2) {
        let snapshot = StoredMetadataSnapshot {
            provider_id: MetadataProviderId::Tmdb,
            external_id: format!("movie-{}", item.id),
            media_type: Some("movie".into()),
            title: Some(item.display_title.clone()),
            overview: Some("Part of a test collection.".into()),
            artwork_url: None,
            backdrop_url: None,
            release_year: Some(1999),
            locale_key: "en-US".into(),
            provider_locale_key: Some("en-US".into()),
            provider_payload_json: Some(
                serde_json::json!({
                    "title": item.display_title,
                    "overview": "Part of a test collection.",
                    "release_date": "1999-03-31",
                    "belongs_to_collection": {
                        "id": 4242,
                        "name": "Test Saga",
                        "overview": "A linked movie collection for home browsing.",
                        "poster_path": "/poster.jpg",
                        "backdrop_path": "/backdrop.jpg"
                    }
                })
                .to_string(),
            ),
        };
        upsert_item_metadata_snapshot(&mut connection, item.id, &snapshot).unwrap();
    }
    upsert_item_metadata_snapshot(
        &mut connection,
        items[0].id,
        &StoredMetadataSnapshot {
            provider_id: MetadataProviderId::Tmdb,
            external_id: format!("movie-{}", items[0].id),
            media_type: Some("movie".into()),
            title: Some(items[0].display_title.clone()),
            overview: Some("Parte de una colección de prueba.".into()),
            artwork_url: None,
            backdrop_url: None,
            release_year: Some(1999),
            locale_key: "es-ES".into(),
            provider_locale_key: Some("es-ES".into()),
            provider_payload_json: Some(
                serde_json::json!({
                    "title": items[0].display_title,
                    "overview": "Parte de una colección de prueba.",
                    "release_date": "1999-03-31",
                    "belongs_to_collection": {
                        "id": 4242,
                        "name": "Saga de Prueba",
                        "overview": "Una colección de películas para navegar.",
                        "poster_path": "/poster-es.jpg",
                        "backdrop_path": "/backdrop-es.jpg"
                    }
                })
                .to_string(),
            ),
        },
    )
    .unwrap();

    let home = get_media_home(&mut connection, None, Some(library.id)).unwrap();
    assert_eq!(home.collections.len(), 1);
    assert_eq!(home.collections[0].name, "Test Saga");
    assert_eq!(home.collections[0].item_count, 2);
    assert_eq!(home.collections[0].item_ids.len(), 2);
    let spanish_home = get_media_home_with_preferred_languages(
        &mut connection,
        None,
        Some(library.id),
        &["es-ES".into()],
    )
    .unwrap();
    assert_eq!(spanish_home.collections.len(), 1);
    assert_eq!(spanish_home.collections[0].name, "Saga de Prueba");
    assert_eq!(spanish_home.collections[0].item_count, 2);

    #[derive(diesel::QueryableByName)]
    struct CountRow {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        count: i64,
    }
    #[derive(diesel::QueryableByName)]
    struct TextRow {
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        value: Option<String>,
    }
    let collection_count = diesel::sql_query("SELECT COUNT(*) AS count FROM metadata_collections")
        .get_result::<CountRow>(&mut connection)
        .unwrap()
        .count;
    let collection_item_count =
        diesel::sql_query("SELECT COUNT(*) AS count FROM metadata_collection_items")
            .get_result::<CountRow>(&mut connection)
            .unwrap()
            .count;
    assert_eq!(collection_count, 2);
    assert_eq!(collection_item_count, 3);
    let collection_references = get_item_youtube_theme_collection_references(
        &mut connection,
        items[0].id,
        MetadataProviderId::Themerr,
    )
    .unwrap();
    assert_eq!(collection_references.len(), 1);
    assert_eq!(
        (
            collection_references[0].1.as_str(),
            collection_references[0].2.as_str(),
            collection_references[0].3.as_str(),
        ),
        ("collection", "tmdb", "4242")
    );
    upsert_secondary_collection_theme_song_url(
        &mut connection,
        collection_references[0].0,
        MetadataProviderId::Themerr,
        collection_references[0].1.as_str(),
        collection_references[0].2.as_str(),
        collection_references[0].3.as_str(),
        "https://youtu.be/SLBACEP6LsI",
    )
    .unwrap();
    let themerr_collection_name = diesel::sql_query(
        "SELECT name AS value FROM metadata_collections \
         WHERE provider_id = 'themerr' AND relation_kind = 'secondary'",
    )
    .get_result::<TextRow>(&mut connection)
    .unwrap()
    .value;
    assert_eq!(themerr_collection_name, None);

    diesel::sql_query(
        "UPDATE metadata_collections SET name = 'Test Saga' \
         WHERE provider_id = 'themerr' AND relation_kind = 'secondary'",
    )
    .execute(&mut connection)
    .unwrap();
    upsert_secondary_collection_theme_song_url(
        &mut connection,
        collection_references[0].0,
        MetadataProviderId::Themerr,
        collection_references[0].1.as_str(),
        collection_references[0].2.as_str(),
        collection_references[0].3.as_str(),
        "https://youtu.be/SLBACEP6LsI",
    )
    .unwrap();
    let repaired_themerr_collection_name = diesel::sql_query(
        "SELECT name AS value FROM metadata_collections \
         WHERE provider_id = 'themerr' AND relation_kind = 'secondary'",
    )
    .get_result::<TextRow>(&mut connection)
    .unwrap()
    .value;
    assert_eq!(repaired_themerr_collection_name, None);
    let merged_home_after_theme = get_media_home(&mut connection, None, Some(library.id)).unwrap();
    assert_eq!(merged_home_after_theme.collections[0].name, "Test Saga");

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_sync_restores_file_name_as_display_title() {
    let root = unique_temp_dir("title_policy_refresh");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("Movie Name.mkv"), b"video").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Movies".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Movies,
        metadata_providers: vec![],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) = create_test_connection("media_catalog_title_policy_refresh");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let library = &persisted[0];
    let items = list_media_items(&mut connection, Some(library.id)).unwrap();
    let movie = items.first().unwrap();

    diesel::sql_query("UPDATE media_files SET display_title = ? WHERE id = ?")
        .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(Some(
            "Embedded Metadata Title".to_string(),
        ))
        .bind::<diesel::sql_types::Integer, _>(movie.id)
        .execute(&mut connection)
        .unwrap();

    sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let refreshed = get_media_item(&mut connection, movie.id, &root.to_string_lossy())
        .unwrap()
        .expect("Expected item detail after refresh");
    assert_eq!(refreshed.display_title, "Movie Name");

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_movie_scan_strips_year_provider_tags_and_format_from_display_title() {
    let root = unique_temp_dir("movie_title_parser");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("Top Gun- Maverick (2022) - 1080p [tmdb-361743] [tvdb-12345].mkv"),
        b"video",
    )
    .unwrap();
    fs::write(
        root.join("Beyond The Sky (2018) - Bluray-1080p.mkv"),
        b"video",
    )
    .unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Movies".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Movies,
        metadata_providers: vec![],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) = create_test_connection("movie_title_parser_db");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let library = &persisted[0];
    let items = list_media_items(&mut connection, Some(library.id)).unwrap();
    let titles = items
        .into_iter()
        .map(|item| item.display_title)
        .collect::<Vec<_>>();

    assert!(
        titles.iter().any(|title| title == "Top Gun: Maverick"),
        "Expected cleaned Top Gun title in {titles:?}"
    );
    assert!(
        titles.iter().any(|title| title == "Beyond The Sky"),
        "Expected cleaned Beyond The Sky title in {titles:?}"
    );

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_item_detail_includes_linked_metadata_presentation() {
    let root = unique_temp_dir("item_detail_linked_metadata");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("movie.mkv"), b"video").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Movies".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Movies,
        metadata_providers: vec![],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) =
        create_test_connection("media_catalog_item_detail_linked_metadata");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let library = &persisted[0];
    let items = list_media_items(&mut connection, Some(library.id)).unwrap();
    let movie = items.first().unwrap();

    let snapshot = StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tmdb,
        external_id: "603".into(),
        media_type: Some("movie".into()),
        title: Some("The Matrix".into()),
        overview: Some("A hacker discovers reality is a simulation.".into()),
        artwork_url: Some("https://image.tmdb.org/t/p/w500/poster.jpg".into()),
        backdrop_url: Some("https://image.tmdb.org/t/p/w1280/backdrop.jpg".into()),
        release_year: Some(1999),
        locale_key: "en-US".into(),
        provider_locale_key: Some("en-US".into()),
        provider_payload_json: Some(
            serde_json::json!({
                "tagline": "Welcome to the real world.",
                "overview": "A hacker discovers reality is a simulation.",
                "genres": [
                    { "id": 28, "name": "Action" },
                    { "id": 878, "name": "Science Fiction" }
                ],
                "release_date": "1999-03-31",
                "vote_average": 8.2,
                "images": {
                    "logos": [
                        { "file_path": "/matrix-logo.png" }
                    ]
                },
                "release_dates": {
                    "results": [
                        {
                            "iso_3166_1": "US",
                            "release_dates": [
                                { "certification": "R" }
                            ]
                        }
                    ]
                },
                "videos": {
                    "results": [
                        {
                            "site": "YouTube",
                            "type": "Trailer",
                            "official": true,
                            "name": "Official Trailer",
                            "key": "vKQi3bBA1y8"
                        }
                    ]
                }
            })
            .to_string(),
        ),
    };
    upsert_item_metadata_snapshot(&mut connection, movie.id, &snapshot).unwrap();
    let stored_link = get_primary_item_metadata_link(&mut connection, movie.id)
        .unwrap()
        .expect("Expected stored metadata link");
    assert_eq!(
        stored_link.logo_url.as_deref(),
        Some("https://image.tmdb.org/t/p/w500/matrix-logo.png")
    );
    assert_eq!(stored_link.rating, Some(8.2));
    assert_eq!(stored_link.content_rating.as_deref(), Some("R"));
    assert_eq!(
        stored_link.trailer_title.as_deref(),
        Some("Official Trailer")
    );

    let detail = get_media_item(&mut connection, movie.id, &root.to_string_lossy())
        .unwrap()
        .expect("Expected linked movie detail to exist");
    let expected_poster_url = format!("/api/v1/items/{}/artwork?kind=poster", movie.id);
    let expected_backdrop_url = format!("/api/v1/items/{}/artwork?kind=backdrop", movie.id);
    assert_eq!(
        detail.tagline.as_deref(),
        Some("Welcome to the real world.")
    );
    assert_eq!(detail.release_year, Some(1999));
    assert_eq!(detail.genres, vec!["Action", "Science Fiction"]);
    assert_eq!(
        detail.logo_url.as_deref(),
        Some("/api/v1/items/1/artwork?kind=logo")
    );
    assert_eq!(detail.rating, Some(8.2));
    assert_eq!(detail.content_rating.as_deref(), Some("R"));
    assert!(detail.artwork_updated_at.is_some());
    assert_eq!(detail.trailer_title.as_deref(), Some("Official Trailer"));
    assert_eq!(
        detail.trailer_url.as_deref(),
        Some("https://www.youtube.com/watch?v=vKQi3bBA1y8")
    );
    assert_eq!(
        detail.poster_url.as_deref(),
        Some(expected_poster_url.as_str())
    );
    assert_eq!(
        detail.backdrop_url.as_deref(),
        Some(expected_backdrop_url.as_str())
    );

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_metadata_links_can_store_multiple_locales_for_same_provider() {
    let root = unique_temp_dir("metadata_link_locales");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("movie.mkv"), b"video").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Movies".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Movies,
        metadata_providers: vec![],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) = create_test_connection("metadata_link_locales_db");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let library = &persisted[0];
    let items = list_media_items(&mut connection, Some(library.id)).unwrap();
    let movie = items.first().unwrap();

    upsert_item_metadata_snapshot(
        &mut connection,
        movie.id,
        &StoredMetadataSnapshot {
            provider_id: MetadataProviderId::Tmdb,
            external_id: "603".into(),
            media_type: Some("movie".into()),
            title: Some("The Matrix".into()),
            overview: Some("English overview.".into()),
            artwork_url: None,
            backdrop_url: None,
            release_year: Some(1999),
            locale_key: "en-US".into(),
            provider_locale_key: Some("en-US".into()),
            provider_payload_json: None,
        },
    )
    .unwrap();
    upsert_item_metadata_snapshot(
        &mut connection,
        movie.id,
        &StoredMetadataSnapshot {
            provider_id: MetadataProviderId::Tmdb,
            external_id: "603".into(),
            media_type: Some("movie".into()),
            title: Some("Matrix".into()),
            overview: Some("Resumen en espanol.".into()),
            artwork_url: None,
            backdrop_url: None,
            release_year: Some(1999),
            locale_key: "es-ES".into(),
            provider_locale_key: Some("es-ES".into()),
            provider_payload_json: None,
        },
    )
    .unwrap();

    let preferred = get_preferred_item_metadata_link_for_languages(
        &mut connection,
        movie.id,
        &[
            "es-ES".to_string(),
            "en-US".to_string(),
        ],
    )
    .unwrap()
    .expect("Expected localized metadata link");
    assert_eq!(preferred.locale_key, "es-ES");
    assert_eq!(preferred.overview.as_deref(), Some("Resumen en espanol."));

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_item_detail_uses_primary_metadata_link_only() {
    let root = unique_temp_dir("item_detail_primary_metadata_only");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("movie.mkv"), b"video").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Movies".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Movies,
        metadata_providers: vec![],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) =
        create_test_connection("media_catalog_item_detail_primary_metadata_only");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let library = &persisted[0];
    let items = list_media_items(&mut connection, Some(library.id)).unwrap();
    let movie = items.first().unwrap();

    upsert_item_metadata_snapshot(
        &mut connection,
        movie.id,
        &StoredMetadataSnapshot {
            provider_id: MetadataProviderId::Tmdb,
            external_id: "603".into(),
            media_type: Some("movie".into()),
            title: Some("The Matrix".into()),
            overview: Some("Primary metadata overview.".into()),
            artwork_url: Some("https://image.tmdb.org/t/p/w500/poster.jpg".into()),
            backdrop_url: None,
            release_year: Some(1999),
            locale_key: "en-US".into(),
            provider_locale_key: Some("en-US".into()),
            provider_payload_json: None,
        },
    )
    .unwrap();

    diesel::sql_query(
        "INSERT INTO item_metadata_links (\
            media_item_id, provider_id, external_id, title, overview, tagline, artwork_url, backdrop_url, \
            release_year, media_type, relation_kind, match_state, cached_artwork_path, \
            cached_backdrop_path, refresh_state, refresh_interval_seconds, last_refreshed_at, next_refresh_at, \
            refresh_error, updated_at\
        ) VALUES (?, ?, ?, ?, ?, NULL, ?, NULL, NULL, ?, ?, ?, NULL, NULL, ?, 0, NULL, NULL, NULL, ?)",
    )
    .bind::<diesel::sql_types::Integer, _>(movie.id)
    .bind::<diesel::sql_types::Text, _>("musicbrainz")
    .bind::<diesel::sql_types::Text, _>("musicbrainz:collection:999")
    .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(Some(
        "Wrong Collection Title".to_string(),
    ))
    .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(Some(
        "Wrong collection overview.".to_string(),
    ))
    .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(Some(
        "https://example.invalid/wrong.jpg".to_string(),
    ))
    .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(Some(
        "collection".to_string(),
    ))
    .bind::<diesel::sql_types::Text, _>("collection")
    .bind::<diesel::sql_types::Text, _>("linked")
    .bind::<diesel::sql_types::Text, _>("fresh")
    .bind::<diesel::sql_types::Nullable<diesel::sql_types::BigInt>, _>(Some(i64::MAX - 1))
    .execute(&mut connection)
    .unwrap();

    let detail = get_media_item(&mut connection, movie.id, &root.to_string_lossy())
        .unwrap()
        .expect("Expected item detail to exist");
    assert_eq!(detail.display_title, "The Matrix");
    assert_eq!(
        detail.overview.as_deref(),
        Some("Primary metadata overview.")
    );

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_item_detail_merges_metadata_links_by_library_provider_order() {
    let root = unique_temp_dir("item_detail_metadata_provider_merge");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("movie.mkv"), b"video").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Movies".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Movies,
        metadata_providers: vec![
            MetadataProviderId::Tmdb,
            MetadataProviderId::Tvdb,
        ],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) =
        create_test_connection("item_detail_metadata_provider_merge_db");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let movie = list_media_items(&mut connection, Some(persisted[0].id))
        .unwrap()
        .into_iter()
        .find(|item| item.item_type == "movie")
        .unwrap();

    upsert_item_metadata_snapshot(
        &mut connection,
        movie.id,
        &StoredMetadataSnapshot {
            provider_id: MetadataProviderId::Tmdb,
            external_id: "603".into(),
            media_type: Some("movie".into()),
            title: Some("Priority Title".into()),
            overview: None,
            artwork_url: None,
            backdrop_url: None,
            release_year: Some(1999),
            locale_key: "en-US".into(),
            provider_locale_key: Some("en-US".into()),
            provider_payload_json: None,
        },
    )
    .unwrap();
    upsert_item_metadata_snapshot(
        &mut connection,
        movie.id,
        &StoredMetadataSnapshot {
            provider_id: MetadataProviderId::Tvdb,
            external_id: "movie-603".into(),
            media_type: Some("movie".into()),
            title: Some("Fallback Title".into()),
            overview: Some("Fallback overview from lower priority provider.".into()),
            artwork_url: None,
            backdrop_url: None,
            release_year: Some(2003),
            locale_key: "en-US".into(),
            provider_locale_key: Some("eng".into()),
            provider_payload_json: None,
        },
    )
    .unwrap();

    let detail = get_media_item(&mut connection, movie.id, &root.to_string_lossy())
        .unwrap()
        .expect("Expected item detail");
    assert_eq!(detail.display_title, "Priority Title");
    assert_eq!(
        detail.overview.as_deref(),
        Some("Fallback overview from lower priority provider.")
    );
    assert_eq!(detail.release_year, Some(1999));

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_secondary_theme_song_metadata_is_stored_and_presented() {
    let root = unique_temp_dir("secondary_theme_song_metadata");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("movie.mkv"), b"video").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Movies".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Movies,
        metadata_providers: vec![
            MetadataProviderId::Tmdb,
            MetadataProviderId::Themerr,
        ],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) = create_test_connection("secondary_theme_song_metadata_db");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let movie = list_media_items(&mut connection, Some(persisted[0].id))
        .unwrap()
        .into_iter()
        .find(|item| item.item_type == "movie")
        .unwrap();

    upsert_item_metadata_snapshot(
        &mut connection,
        movie.id,
        &StoredMetadataSnapshot {
            provider_id: MetadataProviderId::Tmdb,
            external_id: "603".into(),
            media_type: Some("movie".into()),
            title: Some("The Matrix".into()),
            overview: None,
            artwork_url: None,
            backdrop_url: None,
            release_year: Some(1999),
            locale_key: "en-US".into(),
            provider_locale_key: Some("en-US".into()),
            provider_payload_json: None,
        },
    )
    .unwrap();

    let secondary = upsert_secondary_youtube_theme_metadata_link(
        &mut connection,
        movie.id,
        MetadataProviderId::Themerr,
        "movie",
        "tmdb",
        "603",
        "https://youtu.be/SLBACEP6LsI",
        None,
    )
    .unwrap()
    .expect("Expected secondary metadata link");
    assert_eq!(
        secondary.theme_song_url.as_deref(),
        Some("https://www.youtube.com/watch?v=SLBACEP6LsI")
    );

    let detail = get_media_item(&mut connection, movie.id, &root.to_string_lossy())
        .unwrap()
        .expect("Expected item detail");
    assert_eq!(
        detail.theme_song_url.as_deref(),
        Some("https://www.youtube.com/watch?v=SLBACEP6LsI")
    );

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_metadata_refresh_target_change_clears_cached_artwork_paths() {
    let root = unique_temp_dir("metadata_refresh_target_change_clears_cache");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("movie.mkv"), b"video").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Movies".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Movies,
        metadata_providers: vec![MetadataProviderId::Tmdb],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) =
        create_test_connection("metadata_refresh_target_change_clears_cache_db");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let library = &persisted[0];
    let items = list_media_items(&mut connection, Some(library.id)).unwrap();
    let movie = items.first().unwrap();

    upsert_item_metadata_snapshot(
        &mut connection,
        movie.id,
        &StoredMetadataSnapshot {
            provider_id: MetadataProviderId::Tmdb,
            external_id: "603".into(),
            media_type: Some("movie".into()),
            title: Some("The Matrix".into()),
            overview: Some("A hacker discovers reality is a simulation.".into()),
            artwork_url: Some("https://image.tmdb.org/t/p/w500/poster.jpg".into()),
            backdrop_url: Some("https://image.tmdb.org/t/p/w1280/backdrop.jpg".into()),
            release_year: Some(1999),
            locale_key: "en-US".into(),
            provider_locale_key: Some("en-US".into()),
            provider_payload_json: None,
        },
    )
    .unwrap();
    diesel::sql_query(
        "UPDATE item_metadata_links SET cached_artwork_path = ?, cached_backdrop_path = ? WHERE media_item_id = ?",
    )
    .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(Some(
        "C:/tmp/old-poster.jpg".to_string(),
    ))
    .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(Some(
        "C:/tmp/old-backdrop.jpg".to_string(),
    ))
    .bind::<diesel::sql_types::Integer, _>(movie.id)
    .execute(&mut connection)
    .unwrap();

    set_item_metadata_refresh_state(
        &mut connection,
        movie.id,
        MetadataProviderId::Tmdb,
        "999",
        Some("movie"),
        "pending",
        None,
    )
    .unwrap();

    let link = get_primary_item_metadata_link(&mut connection, movie.id)
        .unwrap()
        .expect("Expected metadata link to exist");
    assert!(
        link.cached_artwork_path.is_none(),
        "Expected cached artwork path to clear when metadata target changes"
    );
    assert!(
        link.cached_backdrop_path.is_none(),
        "Expected cached backdrop path to clear when metadata target changes"
    );

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_preferred_item_metadata_link_rejects_episode_tmdb_link_with_wrong_show_external_id() {
    let root = unique_temp_dir("preferred_episode_metadata_link");
    let alpha = root.join("Alpha Show").join("Season 1");
    let beta = root.join("Beta Show").join("Season 1");
    fs::create_dir_all(&alpha).unwrap();
    fs::create_dir_all(&beta).unwrap();
    fs::write(alpha.join("Alpha Show - S01E01.mkv"), b"video").unwrap();
    fs::write(beta.join("Beta Show - S01E01.mkv"), b"video").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Shows".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Shows,
        metadata_providers: vec![MetadataProviderId::Tmdb],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) = create_test_connection("preferred_episode_metadata_link_db");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let library = &persisted[0];
    let items = list_media_items(&mut connection, Some(library.id)).unwrap();
    let alpha_show = items
        .iter()
        .find(|item| item.item_type == "show" && item.display_title.contains("Alpha"))
        .unwrap();
    let alpha_episode = items
        .iter()
        .find(|item| item.item_type == "episode" && item.relative_path.contains("Alpha Show"))
        .unwrap();

    upsert_item_metadata_snapshot(
        &mut connection,
        alpha_show.id,
        &StoredMetadataSnapshot {
            provider_id: MetadataProviderId::Tmdb,
            external_id: "111".into(),
            media_type: Some("tv".into()),
            title: Some("Alpha Show".into()),
            overview: None,
            artwork_url: None,
            backdrop_url: None,
            release_year: None,
            locale_key: "en-US".into(),
            provider_locale_key: Some("en-US".into()),
            provider_payload_json: None,
        },
    )
    .unwrap();

    upsert_item_metadata_snapshot(
        &mut connection,
        alpha_episode.id,
        &StoredMetadataSnapshot {
            provider_id: MetadataProviderId::Tmdb,
            external_id: "tv:222:season:1:episode:1".into(),
            media_type: Some("tv_episode".into()),
            title: Some("Wrong Episode".into()),
            overview: None,
            artwork_url: None,
            backdrop_url: None,
            release_year: None,
            locale_key: "en-US".into(),
            provider_locale_key: Some("en-US".into()),
            provider_payload_json: None,
        },
    )
    .unwrap();

    let preferred = get_preferred_item_metadata_link(&mut connection, alpha_episode.id).unwrap();
    assert!(
        preferred.is_none(),
        "Expected mismatched TMDB episode link to be rejected"
    );

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_resolve_media_item_source_path_rejects_mismatched_backing_file() {
    let root = unique_temp_dir("reject_mismatched_episode_backing_file");
    let alpha = root.join("Alpha Show").join("Season 1");
    let beta = root.join("Beta Show").join("Season 1");
    fs::create_dir_all(&alpha).unwrap();
    fs::create_dir_all(&beta).unwrap();
    fs::write(alpha.join("Alpha Show - S01E01.mkv"), b"video").unwrap();
    fs::write(beta.join("Beta Show - S01E01.mkv"), b"video").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Shows".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Shows,
        metadata_providers: vec![],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) =
        create_test_connection("reject_mismatched_episode_backing_file_db");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let library = &persisted[0];
    let items = list_media_items(&mut connection, Some(library.id)).unwrap();
    let alpha_episode = items
        .iter()
        .find(|item| item.item_type == "episode" && item.relative_path.contains("Alpha Show"))
        .unwrap();
    let beta_episode = items
        .iter()
        .find(|item| item.item_type == "episode" && item.relative_path.contains("Beta Show"))
        .unwrap();

    diesel::sql_query("UPDATE media_files SET media_item_id = NULL WHERE media_item_id = ?")
        .bind::<diesel::sql_types::Integer, _>(alpha_episode.id)
        .execute(&mut connection)
        .unwrap();
    diesel::sql_query("UPDATE media_files SET media_item_id = ? WHERE media_item_id = ?")
        .bind::<diesel::sql_types::Integer, _>(alpha_episode.id)
        .bind::<diesel::sql_types::Integer, _>(beta_episode.id)
        .execute(&mut connection)
        .unwrap();

    let resolved = resolve_media_item_source_path(&mut connection, alpha_episode.id).unwrap();
    assert!(
        resolved.is_none(),
        "Expected no source path when the linked media file path does not match the item path"
    );

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_persisted_library_summaries_include_metadata_refresh_progress() {
    let root = unique_temp_dir("library_refresh_progress");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("The Matrix (1999).mkv"), b"video").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Movies".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Movies,
        metadata_providers: vec![MetadataProviderId::Tmdb],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) = create_test_connection("library_refresh_progress_db");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let library = &persisted[0];
    let items = list_media_items(&mut connection, Some(library.id)).unwrap();
    let movie = items.iter().find(|item| item.item_type == "movie").unwrap();

    upsert_item_metadata_snapshot(
        &mut connection,
        movie.id,
        &StoredMetadataSnapshot {
            provider_id: MetadataProviderId::Tmdb,
            external_id: "603".into(),
            media_type: Some("movie".into()),
            title: Some("The Matrix".into()),
            overview: Some("A hacker discovers reality is a simulation.".into()),
            artwork_url: Some(
                "https://image.tmdb.org/t/p/w500/f89U3ADr1oiB1s9GkdPOEpXUk5H.jpg".into(),
            ),
            backdrop_url: None,
            release_year: Some(1999),
            locale_key: "en-US".into(),
            provider_locale_key: Some("en-US".into()),
            provider_payload_json: None,
        },
    )
    .unwrap();

    let fresh_summary = get_persisted_library_summaries(&mut connection, &libraries).unwrap();
    assert_eq!(fresh_summary[0].metadata_refresh_total, 1);
    assert_eq!(fresh_summary[0].metadata_refresh_pending, 0);
    assert_eq!(fresh_summary[0].metadata_refresh_completed, 1);
    assert_eq!(fresh_summary[0].metadata_refresh_failed, 0);

    set_item_metadata_refresh_state(
        &mut connection,
        movie.id,
        MetadataProviderId::Tmdb,
        "603",
        Some("movie"),
        "pending",
        None,
    )
    .unwrap();
    let pending_summary = get_persisted_library_summaries(&mut connection, &libraries).unwrap();
    assert_eq!(pending_summary[0].metadata_refresh_total, 1);
    assert_eq!(pending_summary[0].metadata_refresh_pending, 1);
    assert_eq!(pending_summary[0].metadata_refresh_completed, 0);
    assert_eq!(pending_summary[0].metadata_refresh_failed, 0);

    set_item_metadata_refresh_state(
        &mut connection,
        movie.id,
        MetadataProviderId::Tmdb,
        "603",
        Some("movie"),
        "error",
        Some("boom"),
    )
    .unwrap();
    let error_summary = get_persisted_library_summaries(&mut connection, &libraries).unwrap();
    assert_eq!(error_summary[0].metadata_refresh_total, 1);
    assert_eq!(error_summary[0].metadata_refresh_pending, 0);
    assert_eq!(error_summary[0].metadata_refresh_completed, 1);
    assert_eq!(error_summary[0].metadata_refresh_failed, 1);

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_secondary_theme_song_reference_inherits_from_linked_show() {
    let root = unique_temp_dir("secondary_theme_song_reference_show");
    let season_dir = root.join("Mock Show").join("Season 1");
    fs::create_dir_all(&season_dir).unwrap();
    fs::write(
        season_dir.join("Mock Show - S01E01 - Winter Is Coming.mkv"),
        b"video",
    )
    .unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Shows".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Shows,
        metadata_providers: vec![MetadataProviderId::Tmdb],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) =
        create_test_connection("secondary_theme_song_reference_show_db");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let library = &persisted[0];
    let items = list_media_items(&mut connection, Some(library.id)).unwrap();
    let show = items.iter().find(|item| item.item_type == "show").unwrap();
    let season = items
        .iter()
        .find(|item| item.item_type == "season")
        .unwrap();
    let episode = items
        .iter()
        .find(|item| item.item_type == "episode")
        .unwrap();

    upsert_item_metadata_snapshot(
        &mut connection,
        show.id,
        &StoredMetadataSnapshot {
            provider_id: MetadataProviderId::Tmdb,
            external_id: "1399".into(),
            media_type: Some("tv".into()),
            title: Some("Mock Show".into()),
            overview: None,
            artwork_url: None,
            backdrop_url: None,
            release_year: Some(2011),
            locale_key: "en-US".into(),
            provider_locale_key: Some("en-US".into()),
            provider_payload_json: Some(serde_json::json!({ "name": "Mock Show" }).to_string()),
        },
    )
    .unwrap();

    assert_eq!(
        get_item_youtube_theme_provider_references(
            &mut connection,
            show.id,
            MetadataProviderId::Themerr
        )
        .unwrap(),
        vec![("tv".into(), "tmdb".into(), "1399".into())]
    );
    assert_eq!(
        get_item_youtube_theme_provider_references(
            &mut connection,
            season.id,
            MetadataProviderId::Themerr
        )
        .unwrap(),
        vec![("tv".into(), "tmdb".into(), "1399".into())]
    );
    assert_eq!(
        get_item_youtube_theme_provider_references(
            &mut connection,
            episode.id,
            MetadataProviderId::Themerr
        )
        .unwrap(),
        vec![("tv".into(), "tmdb".into(), "1399".into())]
    );

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_secondary_theme_song_reference_includes_external_id_fallbacks() {
    let root = unique_temp_dir("secondary_theme_song_reference_movie");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("The Matrix (1999).mkv"), b"video").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Movies".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Movies,
        metadata_providers: vec![MetadataProviderId::Tmdb],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) =
        create_test_connection("secondary_theme_song_reference_movie_db");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let library = &persisted[0];
    let items = list_media_items(&mut connection, Some(library.id)).unwrap();
    let movie = items.iter().find(|item| item.item_type == "movie").unwrap();

    upsert_item_metadata_snapshot(
        &mut connection,
        movie.id,
        &StoredMetadataSnapshot {
            provider_id: MetadataProviderId::Tmdb,
            external_id: "603".into(),
            media_type: Some("movie".into()),
            title: Some("The Matrix".into()),
            overview: None,
            artwork_url: None,
            backdrop_url: None,
            release_year: Some(1999),
            locale_key: "en-US".into(),
            provider_locale_key: Some("en-US".into()),
            provider_payload_json: Some(
                serde_json::json!({
                    "title": "The Matrix",
                    "imdb_id": "tt0133093"
                })
                .to_string(),
            ),
        },
    )
    .unwrap();

    assert_eq!(
        get_item_youtube_theme_provider_references(
            &mut connection,
            movie.id,
            MetadataProviderId::Themerr
        )
        .unwrap(),
        vec![
            ("movie".into(), "tmdb".into(), "603".into()),
            ("movie".into(), "imdb".into(), "tt0133093".into()),
        ]
    );

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_library_settings_are_persisted_in_database() {
    let root = unique_temp_dir("persisted_library_settings_movies");
    let updated_root = unique_temp_dir("persisted_library_settings_shows");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&updated_root).unwrap();

    let legacy_libraries = vec![MediaLibrarySettings {
        name: "Movies".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Movies,
        metadata_providers: vec![MetadataProviderId::Tmdb],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) = create_test_connection("persisted_library_settings_db");
    let bootstrapped = list_library_settings(&mut connection, &legacy_libraries).unwrap();
    assert_eq!(bootstrapped.len(), 1);
    assert_eq!(bootstrapped[0].name, "Movies");

    let updated = replace_library_settings(
        &mut connection,
        &[MediaLibrarySettings {
            name: "Shows".into(),
            path: updated_root.to_string_lossy().to_string(),
            paths: vec![updated_root.to_string_lossy().to_string()],
            recursive: false,
            kind: MediaLibraryKind::Shows,
            metadata_providers: vec![MetadataProviderId::Tmdb],
            metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
            metadata_languages: vec![],
            allowed_user_ids: vec![],
        }],
    )
    .unwrap();
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].name, "Shows");
    assert_eq!(updated[0].kind, MediaLibraryKind::Shows);

    assert!(remove_library_setting(&mut connection, 0).unwrap());
    assert!(
        list_library_settings(&mut connection, &[])
            .unwrap()
            .is_empty()
    );

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(updated_root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_replace_library_settings_allows_duplicate_paths() {
    let root = unique_temp_dir("persisted_library_settings_duplicate_paths");
    let movies = root.join("Movies");
    fs::create_dir_all(&movies).unwrap();

    let (mut connection, db_path) =
        create_test_connection("persisted_library_settings_duplicate_paths_db");
    let result = replace_library_settings(
        &mut connection,
        &[
            MediaLibrarySettings {
                name: "Movies".into(),
                path: movies.to_string_lossy().to_string(),
                paths: vec![movies.to_string_lossy().to_string()],
                recursive: true,
                kind: MediaLibraryKind::Movies,
                metadata_providers: vec![MetadataProviderId::Tmdb],
                metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
                metadata_languages: vec![],
                allowed_user_ids: vec![],
            },
            MediaLibrarySettings {
                name: "Shows".into(),
                path: movies.to_string_lossy().to_string(),
                paths: vec![movies.to_string_lossy().to_string()],
                recursive: true,
                kind: MediaLibraryKind::Shows,
                metadata_providers: vec![
                    MetadataProviderId::Tmdb,
                    MetadataProviderId::Tvdb,
                ],
                metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
                metadata_languages: vec![],
                allowed_user_ids: vec![],
            },
        ],
    );

    let libraries = result.unwrap();
    assert_eq!(libraries.len(), 2);
    assert_eq!(libraries[0].path, movies.to_string_lossy().to_string());
    assert_eq!(libraries[1].path, movies.to_string_lossy().to_string());

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_sync_library_catalog_initializes_scan_state_for_duplicate_paths() {
    let root = unique_temp_dir("sync_library_catalog_duplicate_paths");
    let media = root.join("Media");
    fs::create_dir_all(&media).unwrap();
    fs::write(media.join("feature.mkv"), b"video").unwrap();

    let duplicate_libraries = vec![
        MediaLibrarySettings {
            name: "Movies".into(),
            path: media.to_string_lossy().to_string(),
            paths: vec![media.to_string_lossy().to_string()],
            recursive: true,
            kind: MediaLibraryKind::Movies,
            metadata_providers: vec![MetadataProviderId::Tmdb],
            metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
            metadata_languages: vec![],
            allowed_user_ids: vec![],
        },
        MediaLibrarySettings {
            name: "Shows".into(),
            path: media.to_string_lossy().to_string(),
            paths: vec![media.to_string_lossy().to_string()],
            recursive: true,
            kind: MediaLibraryKind::Shows,
            metadata_providers: vec![MetadataProviderId::Tvdb],
            metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
            metadata_languages: vec![],
            allowed_user_ids: vec![],
        },
    ];

    let (mut connection, db_path) =
        create_test_connection("sync_library_catalog_duplicate_paths_db");
    replace_library_settings(&mut connection, &duplicate_libraries)
        .expect("Expected duplicate-path settings to persist");

    sync_library_catalog(
        &mut connection,
        &duplicate_libraries,
        &FfmpegSettings::default(),
    )
    .expect("Expected sync to process both duplicate-path libraries");

    let summaries = get_persisted_library_summaries(&mut connection, &[])
        .expect("Expected persisted library summaries after sync");
    assert_eq!(summaries.len(), 2);
    assert_ne!(summaries[0].id, summaries[1].id);
    assert!(
        summaries.iter().all(|summary| summary.scan_revision > 0),
        "Expected every duplicate-path library to have scan_state initialized"
    );

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_sync_allows_duplicate_show_libraries_with_same_path() {
    let root = unique_temp_dir("sync_duplicate_show_libraries_same_path");
    let season = root.join("Example Show").join("Season 1");
    fs::create_dir_all(&season).unwrap();
    fs::write(season.join("Example Show - S01E01.mkv"), b"video").unwrap();

    let duplicate_libraries = vec![
        MediaLibrarySettings {
            name: "TV Shows - TMDB".into(),
            path: root.to_string_lossy().to_string(),
            paths: vec![root.to_string_lossy().to_string()],
            recursive: true,
            kind: MediaLibraryKind::Shows,
            metadata_providers: vec![MetadataProviderId::Tmdb],
            metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
            metadata_languages: vec![],
            allowed_user_ids: vec![],
        },
        MediaLibrarySettings {
            name: "TV Shows - TVDB".into(),
            path: root.to_string_lossy().to_string(),
            paths: vec![root.to_string_lossy().to_string()],
            recursive: true,
            kind: MediaLibraryKind::Shows,
            metadata_providers: vec![MetadataProviderId::Tvdb],
            metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
            metadata_languages: vec![],
            allowed_user_ids: vec![],
        },
    ];

    let (mut connection, db_path) =
        create_test_connection("sync_duplicate_show_libraries_same_path_db");
    replace_library_settings(&mut connection, &duplicate_libraries)
        .expect("Expected duplicate show-library settings to persist");

    sync_library_catalog(
        &mut connection,
        &duplicate_libraries,
        &FfmpegSettings::default(),
    )
    .expect("Expected duplicate show libraries with the same path to sync");

    let summaries = get_persisted_library_summaries(&mut connection, &[])
        .expect("Expected persisted library summaries after sync");
    assert_eq!(summaries.len(), 2);

    for summary in summaries {
        let items = list_media_items(&mut connection, Some(summary.id))
            .expect("Expected scoped media items for duplicate show library");
        assert_eq!(items.len(), 3);
        assert_eq!(
            items.iter().filter(|item| item.item_type == "show").count(),
            1
        );
        assert_eq!(
            items
                .iter()
                .filter(|item| item.item_type == "season")
                .count(),
            1
        );
        assert_eq!(
            items
                .iter()
                .filter(|item| item.item_type == "episode")
                .count(),
            1
        );

        let files = get_library_files(&mut connection, summary.id)
            .expect("Expected scoped media files for duplicate show library");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].library_id, summary.id);
    }

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_latest_metadata_migration_preserves_existing_library_catalog_rows() {
    let root = unique_temp_dir("latest_metadata_migration_preserves_catalog");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("movie-one.mkv"), b"video").unwrap();
    fs::write(root.join("movie-two.mkv"), b"video").unwrap();

    let library = MediaLibrarySettings {
        name: "Movies".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Movies,
        metadata_providers: vec![MetadataProviderId::Tmdb],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    };

    let (mut connection, db_path) = create_test_connection("migration_13_preserves_catalog_db");
    let persisted = sync_library_catalog(&mut connection, &[library], &FfmpegSettings::default())
        .expect("Expected populated catalog before re-running latest migration");
    let library_id = persisted[0].id;
    let item_count_before = list_media_items(&mut connection, Some(library_id))
        .unwrap()
        .len();
    let file_count_before = get_library_files(&mut connection, library_id)
        .unwrap()
        .len();
    let library_count_before = list_library_settings(&mut connection, &[]).unwrap().len();

    connection
        .revert_last_migration(MIGRATIONS)
        .expect("Expected to revert latest metadata migration");
    connection
        .run_pending_migrations(MIGRATIONS)
        .expect("Expected latest metadata migration to re-run successfully");

    let libraries_after = list_library_settings(&mut connection, &[]).unwrap();
    let library_id_after = get_persisted_library_summaries(&mut connection, &[])
        .unwrap()
        .into_iter()
        .find(|entry| entry.path == root.to_string_lossy())
        .expect("Expected migrated library summary to exist")
        .id;
    let item_count_after = list_media_items(&mut connection, Some(library_id_after))
        .unwrap()
        .len();
    let file_count_after = get_library_files(&mut connection, library_id_after)
        .unwrap()
        .len();

    assert_eq!(libraries_after.len(), library_count_before);
    assert_eq!(item_count_after, item_count_before);
    assert_eq!(file_count_after, file_count_before);

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_legacy_collection_schema_repair_preserves_locale_dimensions() {
    let db_path = unique_temp_dir("legacy_collection_schema_repair").with_extension("db");
    let mut connection = SqliteConnection::establish(&db_path.to_string_lossy())
        .expect("Failed to establish SQLite test connection");

    connection
        .batch_execute(
            "CREATE TABLE __diesel_schema_migrations (\
                version VARCHAR(50) PRIMARY KEY NOT NULL,\
                run_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP\
             );\
             INSERT INTO __diesel_schema_migrations(version) VALUES ('0000023');\
             CREATE TABLE media_items (\
                id INTEGER PRIMARY KEY AUTOINCREMENT\
             );\
             CREATE TABLE item_metadata_links (\
                id INTEGER PRIMARY KEY AUTOINCREMENT,\
                media_item_id INTEGER NOT NULL,\
                provider_id TEXT NOT NULL,\
                locale_key TEXT NOT NULL,\
                provider_locale_key TEXT DEFAULT NULL\
             );\
             CREATE TABLE metadata_collections (\
                id INTEGER PRIMARY KEY AUTOINCREMENT,\
                provider_id TEXT NOT NULL,\
                external_id TEXT NOT NULL,\
                name TEXT NOT NULL,\
                overview TEXT DEFAULT NULL,\
                artwork_url TEXT DEFAULT NULL,\
                backdrop_url TEXT DEFAULT NULL,\
                theme_song_url TEXT DEFAULT NULL,\
                updated_at BIGINT DEFAULT NULL,\
                UNIQUE (provider_id, external_id)\
             );\
             CREATE TABLE metadata_collection_items (\
                id INTEGER PRIMARY KEY AUTOINCREMENT,\
                collection_id INTEGER NOT NULL,\
                media_item_id INTEGER NOT NULL,\
                updated_at BIGINT DEFAULT NULL,\
                UNIQUE (collection_id, media_item_id)\
             );\
             INSERT INTO media_items(id) VALUES (1);\
             INSERT INTO item_metadata_links(id, media_item_id, provider_id, locale_key, provider_locale_key)\
                VALUES (10, 1, 'tmdb', 'en-US', 'en-US');\
             INSERT INTO item_metadata_links(id, media_item_id, provider_id, locale_key, provider_locale_key)\
                VALUES (11, 1, 'tmdb', 'es-ES', 'es-ES');\
             INSERT INTO metadata_collections(id, provider_id, external_id, name, overview, updated_at)\
                VALUES (20, 'tmdb', '4242', 'Test Saga', 'English overview', 100);\
             INSERT INTO metadata_collection_items(collection_id, media_item_id, updated_at)\
                VALUES (20, 1, 100);",
        )
        .expect("Expected legacy collection schema fixture to be created");

    reconcile_legacy_sqlite_schema(&mut connection)
        .expect("Expected legacy collection schema repair to complete");

    #[derive(diesel::QueryableByName)]
    struct CountRow {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        count: i64,
    }

    #[derive(diesel::QueryableByName)]
    struct TextRow {
        #[diesel(sql_type = diesel::sql_types::Text)]
        value: String,
    }

    let source_column_count = diesel::sql_query(
        "SELECT COUNT(*) AS count FROM pragma_table_info('metadata_collections') \
         WHERE name = 'source_provider_id'",
    )
    .get_result::<CountRow>(&mut connection)
    .unwrap()
    .count;
    assert_eq!(source_column_count, 1);
    let name_not_null = diesel::sql_query(
        "SELECT CAST(COALESCE(MAX(\"notnull\"), 0) AS BIGINT) AS count \
         FROM pragma_table_info('metadata_collections') \
         WHERE name = 'name'",
    )
    .get_result::<CountRow>(&mut connection)
    .unwrap()
    .count;
    assert_eq!(name_not_null, 0);

    let locales = diesel::sql_query(
        "SELECT locale_key AS value FROM metadata_collections ORDER BY locale_key",
    )
    .load::<TextRow>(&mut connection)
    .unwrap()
    .into_iter()
    .map(|row| row.value)
    .collect::<Vec<_>>();
    assert_eq!(
        locales,
        vec![
            "en-US".to_string(),
            "es-ES".to_string()
        ]
    );

    let membership_count = diesel::sql_query(
        "SELECT COUNT(*) AS count FROM metadata_collection_items \
         WHERE metadata_link_id IS NOT NULL",
    )
    .get_result::<CountRow>(&mut connection)
    .unwrap()
    .count;
    assert_eq!(membership_count, 2);

    diesel::sql_query(
        "INSERT INTO metadata_collections (\
            provider_id, external_id, source_provider_id, source_external_id,\
            relation_kind, locale_key, name\
         ) VALUES ('tmdb', '4242', 'tmdb', '4242', 'primary', 'fr-FR', 'Saga FR')",
    )
    .execute(&mut connection)
    .expect("Expected repaired collection uniqueness to include locale");

    drop(connection);
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_resolve_local_item_artwork_ignores_unlinked_media_file_id_collision() {
    let root = unique_temp_dir("episode_artwork_id_collision");
    let alpha = root.join("Alpha Show").join("Season 1");
    let beta = root.join("Beta Show").join("Season 1");
    let gamma = root.join("Gamma Show").join("Season 1");
    fs::create_dir_all(&alpha).unwrap();
    fs::create_dir_all(&beta).unwrap();
    fs::create_dir_all(&gamma).unwrap();
    fs::write(alpha.join("Alpha Show - S01E01.mkv"), b"video").unwrap();
    fs::write(beta.join("Beta Show - S01E01.mkv"), b"video").unwrap();
    fs::write(gamma.join("Gamma Show - S01E01.mkv"), b"video").unwrap();
    fs::write(gamma.join("poster.jpg"), b"image").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Shows".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Shows,
        metadata_providers: vec![],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) = create_test_connection("episode_artwork_id_collision_db");
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let library = &persisted[0];
    let items = list_media_items(&mut connection, Some(library.id)).unwrap();
    let episodes = items
        .into_iter()
        .filter(|item| item.item_type == "episode")
        .collect::<Vec<_>>();
    let target_episode = episodes
        .iter()
        .find(|episode| episode.relative_path.contains("Alpha Show"))
        .expect("Expected Alpha Show episode to exist");
    let fallback_episode = episodes
        .iter()
        .find(|episode| episode.relative_path.contains("Gamma Show"))
        .expect("Expected Gamma Show episode to exist");

    diesel::sql_query("UPDATE media_files SET media_item_id = NULL WHERE media_item_id = ?")
        .bind::<diesel::sql_types::Integer, _>(target_episode.id)
        .execute(&mut connection)
        .unwrap();
    diesel::sql_query("DELETE FROM media_files WHERE id = ?")
        .bind::<diesel::sql_types::Integer, _>(target_episode.id)
        .execute(&mut connection)
        .unwrap();
    diesel::sql_query(
        "INSERT INTO media_files (\
            id, library_id, source_root_path, relative_path, file_size, modified_at, media_kind, \
            fingerprint_seed, display_title, container, duration_ms, bit_rate, width, height, \
            video_codec, audio_codec, metadata_json, metadata_updated_at, metadata_match_attempted_at, media_item_id\
        ) \
        SELECT \
            ?, library_id, source_root_path, relative_path || '.id-collision', file_size, modified_at, media_kind, \
            fingerprint_seed, display_title, container, duration_ms, bit_rate, width, height, \
            video_codec, audio_codec, metadata_json, metadata_updated_at, metadata_match_attempted_at, ? \
        FROM media_files WHERE media_item_id = ? LIMIT 1",
    )
        .bind::<diesel::sql_types::Integer, _>(target_episode.id)
        .bind::<diesel::sql_types::Integer, _>(fallback_episode.id)
        .bind::<diesel::sql_types::Integer, _>(fallback_episode.id)
        .execute(&mut connection)
        .unwrap();

    let resolved = resolve_local_item_artwork_path(
        &mut connection,
        target_episode.id,
        ArtworkKind::Poster,
        &root.to_string_lossy(),
    )
    .unwrap();
    assert!(
        resolved.is_none(),
        "Expected no artwork when an episode has no linked media file, got {:?}",
        resolved
    );

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}

#[test]
fn test_playback_progress_is_scoped_per_user() {
    let root = unique_temp_dir("playback_progress_per_user");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("movie.mkv"), b"video").unwrap();

    let libraries = vec![MediaLibrarySettings {
        name: "Movies".into(),
        path: root.to_string_lossy().to_string(),
        paths: vec![root.to_string_lossy().to_string()],
        recursive: true,
        kind: MediaLibraryKind::Movies,
        metadata_providers: vec![MetadataProviderId::Tmdb],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    }];

    let (mut connection, db_path) = create_test_connection("playback_progress_per_user_db");
    diesel::sql_query("INSERT INTO users (username, password, pin, admin) VALUES ('alice', 'hash', NULL, 1), ('bob', 'hash', NULL, 0)")
        .execute(&mut connection)
        .unwrap();
    let persisted =
        sync_library_catalog(&mut connection, &libraries, &FfmpegSettings::default()).unwrap();
    let library = &persisted[0];
    let item = list_media_items(&mut connection, Some(library.id))
        .unwrap()
        .pop()
        .unwrap();

    upsert_playback_progress(
        &mut connection,
        1,
        item.id,
        120_000,
        item.duration_ms,
        false,
    )
    .unwrap();
    upsert_playback_progress(
        &mut connection,
        2,
        item.id,
        240_000,
        item.duration_ms,
        false,
    )
    .unwrap();

    let alice_home = get_media_home(&mut connection, Some(1), Some(library.id)).unwrap();
    let bob_home = get_media_home(&mut connection, Some(2), Some(library.id)).unwrap();
    let anonymous_home = get_media_home(&mut connection, None, Some(library.id)).unwrap();

    assert_eq!(alice_home.shelves[0].items.len(), 1);
    assert_eq!(bob_home.shelves[0].items.len(), 1);
    assert!(anonymous_home.shelves[0].items.is_empty());
    assert_eq!(alice_home.shelves[0].items[0].id, item.id);
    assert_eq!(bob_home.shelves[0].items[0].id, item.id);

    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(db_path).unwrap();
}
