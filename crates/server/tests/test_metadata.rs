// local imports
use koko::config::{
    MediaLibraryKind, MediaLibrarySettings, MetadataProviderId, MetadataProviderSettings,
    MetadataSettings, Settings, settings_for_persistence,
};
use koko::metadata::{
    StoredMetadataSnapshot, expected_artwork_cache_path, list_provider_statuses,
    managed_metadata_asset_dir, persist_item_metadata_assets,
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
fn test_settings_persistence_clears_library_definitions() {
    let mut settings = Settings::default();
    settings.media.libraries.push(MediaLibrarySettings {
        name: "Movies".into(),
        path: "C:/Media/Movies".into(),
        paths: vec!["C:/Media/Movies".into()],
        recursive: true,
        kind: MediaLibraryKind::Movies,
        metadata_providers: vec![MetadataProviderId::Tmdb],
    });

    let persisted = settings_for_persistence(&settings);
    assert!(persisted.media.libraries.is_empty());
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
