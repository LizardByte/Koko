//! Matching, credential isolation, cache eviction, and ambiguous-title regressions.
use super::*;
use crate::config::MetadataProviderSettings;
use crate::metadata::{
    MetadataProviderDescriptor,
    StoredMetadataSnapshot,
    providers::MetadataProviderFuture,
};

struct Provider {
    id: MetadataProviderId,
    results: Vec<MetadataSearchResult>,
    episodes: HashMap<String, String>,
}
impl Provider {
    fn snapshot(
        &self,
        id: &str,
        title: Option<String>,
    ) -> StoredMetadataSnapshot {
        StoredMetadataSnapshot {
            provider_id: self.id.clone(),
            external_id: id.into(),
            media_type: Some("tv".into()),
            title,
            overview: None,
            artwork_url: Some(format!("https://example.com/{id}/poster.jpg")),
            backdrop_url: Some(format!("https://example.com/{id}/backdrop.jpg")),
            release_year: None,
            locale_key: "en-US".into(),
            provider_locale_key: None,
            provider_payload_json: None,
        }
    }
}
impl MetadataProvider for Provider {
    fn descriptor(&self) -> MetadataProviderDescriptor {
        MetadataRegistry::new()
            .provider(&self.id)
            .unwrap()
            .descriptor()
    }

    fn search<'a>(
        &'a self,
        _: &'a MetadataSettings,
        _: &'a str,
        _: Option<&'a str>,
    ) -> MetadataProviderFuture<'a, Vec<MetadataSearchResult>> {
        Box::pin(async move { Ok(self.results.clone()) })
    }

    fn fetch_snapshot<'a>(
        &'a self,
        _: &'a MetadataSettings,
        id: &'a str,
        _: &'a str,
        _: bool,
    ) -> MetadataProviderFuture<'a, StoredMetadataSnapshot> {
        Box::pin(async move { Ok(self.snapshot(id, Some("Last Man Standing".into()))) })
    }

    fn fetch_episode_snapshot<'a>(
        &'a self,
        _: &'a MetadataSettings,
        id: &'a str,
        season: i32,
        episode: i32,
        _: Option<&'a str>,
        _: bool,
    ) -> MetadataProviderFuture<'a, StoredMetadataSnapshot> {
        Box::pin(async move {
            assert_eq!((season, episode), (7, 20));
            Ok(self.snapshot(id, self.episodes.get(id).cloned()))
        })
    }
}

fn program() -> Program {
    Program {
        id: 1,
        source_id: 1,
        guide_key: "station".into(),
        starts_at: super::super::now(),
        ends_at: super::super::now() + 1800,
        title: "Last Man Standing".into(),
        metadata: super::super::formats::ProgramMetadata {
            episode: Some("S7E20".into()),
            subtitle: Some("Yass Queen".into()),
            ..Default::default()
        },
    }
}
fn candidate(
    id: &str,
    kind: &str,
    year: Option<i32>,
) -> MetadataSearchResult {
    MetadataSearchResult {
        provider_id: MetadataProviderId::Tmdb,
        external_id: id.into(),
        media_type: kind.into(),
        title: "Last Man Standing".into(),
        overview: None,
        artwork_url: None,
        backdrop_url: None,
        release_year: year,
        score: None,
        track_count: None,
    }
}

#[tokio::test]
async fn both_providers_disambiguate_same_title_series_by_episode_identity() {
    for (id, kind) in [
        (MetadataProviderId::Tmdb, "tv"),
        (MetadataProviderId::Tvdb, "series"),
    ] {
        let provider = Provider {
            id,
            results: vec![
                candidate("competition", kind, None),
                candidate("sitcom", kind, None),
                candidate("movie", "movie", None),
            ],
            episodes: HashMap::from([
                ("competition".into(), "A different episode".into()),
                ("sitcom".into(), "Yass Queen".into()),
            ]),
        };
        let art = lookup(&provider, &MetadataSettings::default(), &program())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            art.poster.as_deref(),
            Some("https://example.com/sitcom/poster.jpg")
        );
    }
}

#[tokio::test]
async fn ambiguous_titles_and_mismatched_episode_names_keep_guide_artwork() {
    let provider = Provider {
        id: MetadataProviderId::Tmdb,
        results: vec![
            candidate("one", "tv", None),
            candidate("two", "tv", None),
        ],
        episodes: HashMap::new(),
    };
    let mut show = program();
    show.metadata.episode = None;
    assert!(
        lookup(&provider, &MetadataSettings::default(), &show)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        lookup(&provider, &MetadataSettings::default(), &program())
            .await
            .unwrap()
            .is_none()
    );
    show.metadata.categories = vec!["Movie".into()];
    show.metadata.year = Some(1958);
    let results = candidates(
        &show,
        vec![
            candidate("original", "movie", Some(1958)),
            candidate("remake", "movie", Some(1986)),
            candidate("series", "tv", Some(1958)),
        ],
        "movie",
    );
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].external_id, "original");
}

#[test]
fn provider_configuration_reuses_ordered_credentials_and_honors_guide_only() {
    let mut settings = MetadataSettings {
        providers: vec![
            MetadataProviderSettings {
                id: MetadataProviderId::Tvdb,
                enabled: true,
                api_key: Some(format!("fixture-{:x}", rand::random::<u128>())),
                ..Default::default()
            },
            MetadataProviderSettings {
                id: MetadataProviderId::Tmdb,
                enabled: true,
                api_key: Some(format!("fixture-{:x}", rand::random::<u128>())),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    assert_eq!(
        configured(&settings, "automatic")
            .providers
            .iter()
            .map(|p| p.id.clone())
            .collect::<Vec<_>>(),
        vec![
            MetadataProviderId::Tvdb,
            MetadataProviderId::Tmdb
        ]
    );
    assert!(configured(&settings, "guide").providers.is_empty());
    assert_eq!(configured(&settings, "tmdb").providers.len(), 1);
    settings.providers[1].enabled = false;
    assert!(configured(&settings, "tmdb").providers.is_empty());
}

#[test]
fn artwork_decoration_preserves_guide_information_and_honors_per_source_selection() {
    let settings = MetadataSettings {
        providers: vec![MetadataProviderSettings {
            id: MetadataProviderId::Tmdb,
            enabled: true,
            api_key: Some(format!("fixture-{:x}", rand::random::<u128>())),
            ..Default::default()
        }],
        ..Default::default()
    };
    let sources = ["automatic", "guide", "tvdb"]
        .into_iter()
        .enumerate()
        .map(|(index, mode)| Source {
            id: index as i32 + 1,
            name: "NextPVR".into(),
            kind: "nextpvr".into(),
            config_json: serde_json::json!({
                "guide_kind":"zap2it", "artwork_provider":mode
            })
            .to_string(),
            enabled: true,
            capacity: 1,
            next_refresh: i64::MAX,
            last_error: None,
        })
        .collect::<Vec<_>>();
    let original = program();
    let mut programs = sources
        .iter()
        .map(|source| Program {
            source_id: source.id,
            ..original.clone()
        })
        .collect::<Vec<_>>();
    decorate(&mut programs, &sources, &settings);
    for program in &programs {
        assert_eq!(program.title, original.title);
        assert_eq!(program.starts_at, original.starts_at);
        assert_eq!(program.ends_at, original.ends_at);
        assert_eq!(program.metadata.episode, original.metadata.episode);
        assert_eq!(program.metadata.subtitle, original.metadata.subtitle);
    }
    assert_eq!(programs[0].metadata.artwork_sources, ["tmdb"]);
    assert!(
        programs[0]
            .metadata
            .poster
            .as_deref()
            .unwrap()
            .ends_with("?kind=poster")
    );
    assert!(programs[1].metadata.poster.is_none());
    assert!(programs[2].metadata.poster.is_none());
    assert!(programs[1].metadata.artwork_sources.is_empty());
    assert!(programs[2].metadata.artwork_sources.is_empty());
}

#[test]
fn cache_reuses_series_artwork_but_separates_unrelated_episode_matches() {
    let settings = MetadataSettings::default();
    let mut show = program();
    let first = key(&show, &settings);
    show.metadata.subtitle = Some("Another episode".into());
    assert_ne!(first, key(&show, &settings));
    show.metadata.series_id = Some("SH01419581".into());
    let series = key(&show, &settings);
    show.metadata.subtitle = Some("Yass Queen".into());
    assert_eq!(series, key(&show, &settings));
    assert!(!series.contains("Last Man Standing"));
    assert!(!eligible(&Program {
        metadata: super::super::formats::ProgramMetadata {
            categories: vec!["Sports event".into()],
            ..Default::default()
        },
        ..show
    }));
    assert!(valid_image("javascript:bad()".into()).is_none());
    assert!(valid_image("https://user:secret@example.com/image.jpg".into()).is_none());
}

#[test]
fn persistent_artwork_cache_expires_negative_results_and_has_a_hard_entry_limit() {
    let mut conn = super::super::tests::database();
    let now = super::super::now();
    save(&mut conn, &format!("{PREFIX}miss"), None, 300, now).unwrap();
    assert!(matches!(
        cached(&mut conn, &format!("{PREFIX}miss"), now).unwrap(),
        Some(None)
    ));
    assert!(
        cached(&mut conn, &format!("{PREFIX}miss"), now + 301)
            .unwrap()
            .is_none()
    );
    let value = serde_json::to_string(&CacheEntry {
        expires_at: now + 600,
        artwork: None,
    })
    .unwrap();
    let rows = (0..CACHE_LIMIT + 20)
        .map(|index| {
            (
                a::key.eq(format!("{PREFIX}{index:05}")),
                a::value.eq(&value),
                a::updated_at.eq(now),
            )
        })
        .collect::<Vec<_>>();
    diesel::insert_into(a::table)
        .values(rows)
        .execute(&mut conn)
        .unwrap();
    diesel::insert_into(a::table)
        .values((
            a::key.eq("liveXtvYartwork:unrelated-setting"),
            a::value.eq("keep"),
            a::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .unwrap();
    prune(&mut conn, now + 301).unwrap();
    assert_eq!(
        a::table
            .filter(a::key.like(PREFIX_PATTERN).escape('\\'))
            .count()
            .get_result::<i64>(&mut conn)
            .unwrap(),
        CACHE_LIMIT
    );
    prune(&mut conn, now + 601).unwrap();
    assert_eq!(a::table.count().get_result::<i64>(&mut conn).unwrap(), 1);
}
