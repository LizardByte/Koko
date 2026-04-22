// local imports
use koko::config::{
    MediaLibraryKind, MediaLibrarySettings, MetadataProviderId, MetadataProviderSettings,
    MetadataSettings, Settings, settings_for_persistence,
};
use koko::metadata::list_provider_statuses;

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
    assert_eq!(serde_json::to_string(&MetadataProviderId::MusicBrainz).unwrap(), "\"musicbrainz\"");
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

