// local imports
use diesel::Connection;
use diesel::connection::SimpleConnection;
use koko::config::{
    FfmpegSettings, MediaLibraryKind, MediaLibrarySettings, MetadataProviderId,
    MetadataProviderSettings, MetadataSettings, Settings, load_database_settings,
    save_database_settings, seed_database_settings, settings_for_persistence,
    settings_yaml_for_persistence,
};
use koko::metadata::{
    StoredMetadataSnapshot, expected_artwork_cache_path, list_provider_statuses,
    managed_metadata_asset_dir, metadata_asset_uuid, persist_item_metadata_assets,
};
use std::fs;

#[test]
fn test_metadata_provider_statuses_include_tmdb() {
    let statuses = list_provider_statuses(&MetadataSettings::default());
    let tmdb = statuses
        .iter()
        .find(|provider| provider.id == MetadataProviderId::Tmdb)
        .expect("Expected TMDB provider to be registered");

    assert_eq!(tmdb.display_name, "TheMovieDB");
    assert!(tmdb.enabled);
    assert!(tmdb.requires_api_key);
    assert!(!tmdb.configured);
    assert!(tmdb.implemented);
}

#[test]
fn test_metadata_provider_statuses_include_tvdb() {
    let statuses = list_provider_statuses(&MetadataSettings::default());
    let tvdb = statuses
        .iter()
        .find(|provider| provider.id == MetadataProviderId::Tvdb)
        .expect("Expected TheTVDB provider to be registered");

    assert_eq!(tvdb.display_name, "TheTVDB");
    assert!(!tvdb.enabled);
    assert!(tvdb.requires_api_key);
    assert!(!tvdb.configured);
    assert!(tvdb.implemented);
}

#[test]
fn test_metadata_settings_default_includes_tvdb_provider_entry() {
    let settings = MetadataSettings::default();
    let tvdb = settings
        .providers
        .iter()
        .find(|provider| provider.id == MetadataProviderId::Tvdb)
        .expect("Expected MetadataSettings::default() to include a TheTVDB provider entry");

    assert!(!tvdb.enabled);
    assert_eq!(tvdb.language, "en-US");
}

#[test]
fn test_metadata_provider_statuses_include_trailerdb() {
    let statuses = list_provider_statuses(&MetadataSettings::default());
    let trailerdb = statuses
        .iter()
        .find(|provider| provider.id == MetadataProviderId::TrailerDb)
        .expect("Expected TrailerDB provider to be registered");

    assert_eq!(trailerdb.display_name, "TrailerDB");
    assert!(trailerdb.enabled);
    assert!(!trailerdb.requires_api_key);
    assert!(trailerdb.configured);
    assert!(trailerdb.implemented);
    assert_eq!(
        trailerdb.extends_provider_ids,
        vec![MetadataProviderId::Tmdb]
    );
}

#[test]
fn test_metadata_provider_statuses_respect_api_key_configuration() {
    let settings = MetadataSettings {
        providers: vec![MetadataProviderSettings {
            id: MetadataProviderId::Tmdb,
            enabled: true,
            api_key: Some("test-key".into()),
            language: "en-US".into(),
            rate_limit_per_second: 4,
            retry_attempts: 3,
            retry_backoff_ms: 1_000,
        }],
        refresh_interval_days: Some(30),
    };

    let statuses = list_provider_statuses(&settings);
    let tmdb = statuses
        .iter()
        .find(|provider| provider.id == MetadataProviderId::Tmdb)
        .expect("Expected TMDB provider to be registered");

    assert!(tmdb.configured);
}

#[test]
fn test_metadata_provider_id_rejects_legacy_musicbrainz_alias() {
    let canonical: MetadataProviderId = serde_json::from_str("\"musicbrainz\"")
        .expect("Expected canonical musicbrainz identifier to deserialize");
    let legacy = serde_json::from_str::<MetadataProviderId>("\"music_brainz\"");

    assert_eq!(canonical, MetadataProviderId::MusicBrainz);
    assert!(legacy.is_err());
    assert_eq!(
        serde_json::to_string(&MetadataProviderId::MusicBrainz).unwrap(),
        "\"musicbrainz\""
    );
}

#[test]
fn test_metadata_provider_id_supports_tvdb() {
    let provider: MetadataProviderId =
        serde_json::from_str("\"tvdb\"").expect("Expected tvdb identifier to deserialize");

    assert_eq!(provider, MetadataProviderId::Tvdb);
    assert_eq!(serde_json::to_string(&provider).unwrap(), "\"tvdb\"");
}

#[test]
fn test_metadata_provider_id_supports_trailerdb() {
    let provider: MetadataProviderId = serde_json::from_str("\"trailerdb\"")
        .expect("Expected trailerdb identifier to deserialize");

    assert_eq!(provider, MetadataProviderId::TrailerDb);
    assert_eq!(provider.as_storage_value(), "trailerdb");
    assert_eq!(serde_json::to_string(&provider).unwrap(), "\"trailerdb\"");
}

#[test]
fn test_settings_persistence_clears_library_definitions() {
    let mut settings = Settings::default();
    settings.media.libraries.push(MediaLibrarySettings {
        name: "Movies".into(),
        path: "C:/Media/Movies".into(),
        paths: vec!["C:/Media/Movies".into()],
        recursive: true,
        kind: MediaLibraryKind::Movies,
        metadata_providers: vec![MetadataProviderId::Tmdb],
        metadata_language_mode: koko::config::MediaLibraryMetadataLanguageMode::Auto,
        metadata_languages: vec![],
        allowed_user_ids: vec![],
    });

    let persisted = settings_for_persistence(&settings);
    assert!(persisted.media.libraries.is_empty());

    let persisted_yaml = settings_yaml_for_persistence(&settings).unwrap();
    for omitted in [
        "media:",
        "libraries:",
        "metadata:",
        "server:",
        "ffmpeg:",
    ] {
        assert!(
            !persisted_yaml.contains(omitted),
            "Expected persisted YAML to omit {omitted}, got:\n{persisted_yaml}"
        );
    }
    assert!(persisted_yaml.contains("general:"));
}

#[test]
fn test_settings_persistence_includes_tvdb_provider_and_preserves_api_key() {
    let mut settings = Settings::default();
    settings.metadata.providers = vec![MetadataProviderSettings {
        id: MetadataProviderId::Tmdb,
        enabled: true,
        api_key: Some("tmdb-key".into()),
        language: "en-US".into(),
        rate_limit_per_second: 4,
        retry_attempts: 3,
        retry_backoff_ms: 1_000,
    }];

    let persisted = settings_for_persistence(&settings);
    let tvdb = persisted
        .metadata
        .providers
        .iter()
        .find(|provider| provider.id == MetadataProviderId::Tvdb)
        .expect("Expected settings persistence to keep a TheTVDB provider entry");
    assert!(!tvdb.enabled);
    assert_eq!(tvdb.api_key, None);

    let tmdb = persisted
        .metadata
        .providers
        .iter()
        .find(|provider| provider.id == MetadataProviderId::Tmdb)
        .expect("Expected settings persistence to keep the TMDB provider entry");
    assert_eq!(tmdb.api_key.as_deref(), Some("tmdb-key"));
}

#[test]
fn test_database_settings_round_trip_runtime_sections() {
    let mut conn =
        diesel::SqliteConnection::establish(":memory:").expect("Expected in-memory SQLite");
    conn.batch_execute(
        "CREATE TABLE app_settings (\
            key TEXT PRIMARY KEY NOT NULL,\
            value TEXT NOT NULL,\
            updated_at BIGINT DEFAULT NULL\
        );",
    )
    .unwrap();

    let mut settings = Settings::default();
    settings.server.port = 8181;
    settings.ffmpeg = FfmpegSettings {
        ffmpeg_path: "C:/Tools/ffmpeg.exe".into(),
        ffprobe_path: "C:/Tools/ffprobe.exe".into(),
    };
    settings.media.missing_item_auto_delete_days = Some(14);
    settings.metadata.providers = vec![MetadataProviderSettings {
        id: MetadataProviderId::Tmdb,
        enabled: true,
        api_key: Some("tmdb-key".into()),
        language: "en-US".into(),
        rate_limit_per_second: 4,
        retry_attempts: 3,
        retry_backoff_ms: 1_000,
    }];

    seed_database_settings(&mut conn, &settings).unwrap();
    let loaded = load_database_settings(&mut conn, &Settings::default()).unwrap();
    assert_eq!(loaded.server.port, 8181);
    assert_eq!(loaded.ffmpeg.ffmpeg_path, "C:/Tools/ffmpeg.exe");
    assert_eq!(loaded.media.missing_item_auto_delete_days, Some(14));
    assert_eq!(
        loaded.metadata.providers[0].api_key.as_deref(),
        Some("tmdb-key")
    );

    let mut updated = loaded.clone();
    updated.server.port = 8282;
    updated.ffmpeg.ffmpeg_path = "D:/ffmpeg.exe".into();
    updated.media.missing_item_auto_delete_days = None;
    save_database_settings(&mut conn, &updated).unwrap();
    let reloaded = load_database_settings(&mut conn, &Settings::default()).unwrap();
    assert_eq!(reloaded.server.port, 8282);
    assert_eq!(reloaded.ffmpeg.ffmpeg_path, "D:/ffmpeg.exe");
    assert_eq!(reloaded.media.missing_item_auto_delete_days, None);
}

#[test]
fn test_expected_artwork_cache_path_changes_when_url_changes() {
    let cache_dir = std::path::Path::new("C:/tmp");
    let first = expected_artwork_cache_path(
        "https://image.tmdb.org/t/p/w500/alpha.jpg",
        cache_dir,
        "tmdb_poster",
    );
    let second = expected_artwork_cache_path(
        "https://image.tmdb.org/t/p/w500/beta.jpg",
        cache_dir,
        "tmdb_poster",
    );

    assert_ne!(first, second);
}

#[test]
fn test_persist_item_metadata_assets_clears_stale_provider_poster_when_url_missing() {
    let temp_dir = std::env::temp_dir().join(format!(
        "koko_metadata_asset_cleanup_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let data_dir = temp_dir.to_string_lossy().to_string();
    let snapshot = StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tmdb,
        external_id: "tv:1412:season:1:episode:1".into(),
        media_type: Some("episode".into()),
        title: Some("Pilot".into()),
        overview: None,
        artwork_url: None,
        backdrop_url: None,
        release_year: Some(2012),
        locale_key: "en-US".into(),
        provider_locale_key: Some("en-US".into()),
        provider_payload_json: None,
    };
    let item_dir = managed_metadata_asset_dir(
        &data_dir,
        snapshot.provider_id.clone(),
        &snapshot.external_id,
        snapshot.media_type.as_deref(),
        &snapshot.locale_key,
    );
    fs::create_dir_all(&item_dir).unwrap();
    fs::write(item_dir.join("tmdb_poster-aaaaaaaaaaaaaaaa.jpg"), b"stale").unwrap();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime
        .block_on(persist_item_metadata_assets(&snapshot, 198, &data_dir))
        .unwrap();

    assert!(
        !item_dir.join("tmdb_poster-aaaaaaaaaaaaaaaa.jpg").exists(),
        "Expected stale provider poster to be removed when no artwork URL is present"
    );

    fs::remove_dir_all(temp_dir).unwrap();
}

#[test]
fn test_managed_metadata_asset_dir_uses_locale_aware_sha256_uuid_path() {
    let data_dir = "C:/koko-data";
    let english = managed_metadata_asset_dir(
        data_dir,
        MetadataProviderId::Tmdb,
        "603",
        Some("movie"),
        "en-US",
    );
    let spanish = managed_metadata_asset_dir(
        data_dir,
        MetadataProviderId::Tmdb,
        "603",
        Some("movie"),
        "es-ES",
    );

    assert_ne!(english, spanish);
    assert_eq!(
        metadata_asset_uuid(MetadataProviderId::Tmdb, "603", "en-US"),
        "tmdb:603:en-US"
    );
    assert!(
        english.to_string_lossy().contains("/metadata/movies/")
            || english.to_string_lossy().contains("\\metadata\\movies\\")
    );
    assert!(!english.to_string_lossy().contains(".bundle"));
}
