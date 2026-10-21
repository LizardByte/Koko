//! Comic Vine contracts use local responses and never require or expose a real API key.

use super::super::audiosilo::tests::mock;
use super::*;
use serde_json::json;

fn settings() -> MetadataSettings {
    let mut settings = MetadataSettings::default();
    let provider = settings
        .providers
        .iter_mut()
        .find(|provider| provider.id == MetadataProviderId::ComicVine)
        .unwrap();
    provider.enabled = true;
    provider.api_key = Some("fixture-key".into());
    settings
}

#[tokio::test]
async fn issue_search_and_fetch_preserve_series_number_and_cover_identity() {
    let issue = json!({"id": 123, "name": "A Beginning",
        "volume": {"id": 456, "name": "Example Series"},
        "issue_number": "2", "cover_date": "2020-03-01", "description": "<p>A &amp; B</p>",
        "image": {"super_url": "https://example.com/cover.jpg"}});
    let (root, worker) = mock(vec![
        (
            "/search/?query=Example&resources=issue",
            json!({"status_code": 1, "results": [issue.clone()]}),
        ),
        (
            "/issue/4000-123/",
            json!({"status_code": 1, "results": issue}),
        ),
    ]);
    let settings = settings();
    TEST_API_ROOT
        .scope(root, async {
            let results = ComicVineMetadataProvider
                .search(&settings, "Example", Some("comic_issue"))
                .await
                .unwrap();
            assert_eq!(results[0].external_id, "4000-123");
            assert_eq!(results[0].title, "Example Series #2 — A Beginning");
            assert_eq!(results[0].release_year, Some(2020));
            assert_eq!(results[0].overview.as_deref(), Some("A & B"));
            let snapshot = ComicVineMetadataProvider
                .fetch_snapshot(&settings, "4000-123", "comic_issue", false)
                .await
                .unwrap();
            assert_eq!(
                snapshot.artwork_url.as_deref(),
                Some("https://example.com/cover.jpg")
            );
            assert_eq!(
                ComicVineMetadataProvider.metadata_details(&snapshot).genres,
                vec!["Comics"]
            );
        })
        .await;
    worker.join().unwrap();
}

#[tokio::test]
async fn series_results_are_distinct_and_wrong_record_types_are_rejected_before_network() {
    let (root, worker) = mock(vec![(
        "/search/?query=Example&resources=volume",
        json!({"status_code": 1,
            "results": [{"id": 456, "name": "Example Series", "start_year": "1998"}]}),
    )]);
    let settings = settings();
    let results = TEST_API_ROOT
        .scope(
            root,
            ComicVineMetadataProvider.search(&settings, "Example", Some("comic_series")),
        )
        .await
        .unwrap();
    worker.join().unwrap();
    assert_eq!(results[0].external_id, "4050-456");
    assert_eq!(results[0].release_year, Some(1998));
    for (id, kind) in [
        ("4000-123", "comic_series"),
        ("4050-456", "comic_issue"),
        ("4000-../secret", "comic_issue"),
        ("4000-1", "book"),
    ] {
        assert!(
            ComicVineMetadataProvider
                .fetch_snapshot(&settings, id, kind, false)
                .await
                .unwrap_err()
                .contains("identifier")
        );
    }
    let mut disabled = MetadataSettings::default();
    disabled
        .providers
        .iter_mut()
        .find(|provider| provider.id == MetadataProviderId::ComicVine)
        .unwrap()
        .enabled = false;
    let error = ComicVineMetadataProvider
        .search(&disabled, "Example", Some("comic_issue"))
        .await
        .unwrap_err();
    assert!(error.contains("Enable Comic Vine"));
    let error = ComicVineMetadataProvider
        .search(&MetadataSettings::default(), "Example", Some("comic_issue"))
        .await
        .unwrap_err();
    assert!(error.contains("requires an API key"));
    assert!(!error.contains("fixture-key"));
}

#[tokio::test]
async fn rejected_api_keys_and_mismatched_responses_never_link_the_wrong_issue() {
    let (root, worker) = mock(vec![
        (
            "/search/",
            json!({"status_code": 100, "error": "Invalid API Key", "results": []}),
        ),
        (
            "/issue/4000-123/",
            json!({"status_code": 1, "results": {"id": 999, "issue_number": "9"}}),
        ),
    ]);
    TEST_API_ROOT
        .scope(root, async {
            let settings = settings();
            let error = ComicVineMetadataProvider
                .search(&settings, "Example", Some("comic_issue"))
                .await
                .unwrap_err();
            assert!(!error.contains("fixture-key"));
            assert!(
                ComicVineMetadataProvider
                    .fetch_snapshot(&settings, "4000-123", "comic_issue", false)
                    .await
                    .unwrap_err()
                    .contains("different record")
            );
        })
        .await;
    worker.join().unwrap();
}
