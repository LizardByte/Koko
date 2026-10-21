//! Recording identity and public API contract regressions for the primary audiobook provider.

use super::super::audiosilo::tests::mock;
use super::*;
use serde_json::json;

fn book() -> serde_json::Value {
    json!({
        "asin": "B005HF3EJC", "region": "us", "title": "How I Filmed the War",
        "authors": [{"name": "Geoffrey H Malins"}],
        "narrators": [{"name": "Felbrigg Napoleon Herriot"}],
        "lengthMinutes": 392, "language": "english", "bookFormat": "unabridged",
        "releaseDate": "2011-08-16", "description": "Book description",
        "series": [{"asin": "B000000001", "name": "A Series", "position": "2.5"}]
    })
}

#[test]
fn recording_choices_keep_the_specific_editions_cover_and_read_old_saved_identities() {
    let mut value = book();
    value["imageUrl"] = json!("https://covers.example/libex-edition.jpg");
    let source: Book = serde_json::from_value(value).unwrap();
    let identity = recording(&source).unwrap();
    assert_eq!(
        identity.artwork_url.as_deref(),
        Some("https://covers.example/libex-edition.jpg")
    );
    let mut legacy = serde_json::to_value(&identity).unwrap();
    legacy.as_object_mut().unwrap().remove("artwork_url");
    let legacy: AudiobookRecording = serde_json::from_value(legacy).unwrap();
    assert_eq!(legacy.artwork_url, None);
    assert_eq!(legacy.external_id, identity.external_id);
}

#[tokio::test]
async fn search_distinguishes_book_facts_from_the_narrators_recording() {
    let (root, worker) = mock(vec![("/search?title=How", json!([book()]))]);
    let results = TEST_API_ROOT
        .scope(
            root,
            LibexMetadataProvider.search(
                &MetadataSettings::default(),
                "How I Filmed the War",
                Some("audiobook"),
            ),
        )
        .await
        .unwrap();
    worker.join().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].external_id, "us/B005HF3EJC");
    assert!(results[0].title.contains("Book details only"));
    assert_eq!(results[1].external_id, "us/B005HF3EJC/recording");
    assert!(results[1].title.contains("Felbrigg Napoleon Herriot"));
}

#[tokio::test]
async fn librivox_title_match_does_not_assign_a_different_narrator_or_chapters() {
    let (root, worker) = mock(vec![("/search", json!([book()]))]);
    let metadata = AudiobookMetadata {
        title: "How I Filmed the War".into(),
        author: "Geoffrey H. Malins".into(),
        narrator: "R. S. Steinberg".into(),
        ..Default::default()
    };
    let result = TEST_API_ROOT
        .scope(root, guess(&MetadataSettings::default(), &metadata))
        .await
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    assert_eq!(result.external_id, "us/B005HF3EJC");
    let (root, worker) = mock(vec![("/book/B005HF3EJC?region=us", book())]);
    let snapshot = TEST_API_ROOT
        .scope(
            root,
            LibexMetadataProvider.fetch_snapshot(
                &MetadataSettings::default(),
                &result.external_id,
                "audiobook",
                true,
            ),
        )
        .await
        .unwrap();
    worker.join().unwrap();
    let normalized =
        recording_metadata(snapshot.provider_payload_json.as_deref().unwrap()).unwrap();
    assert!(normalized.narrator.is_empty());
    assert!(normalized.chapters.is_empty());
    assert_eq!(normalized.author, "Geoffrey H Malins");
    assert!(snapshot.release_year.is_none());
}

#[tokio::test]
async fn explicit_recording_flattens_accurate_chapters_and_keeps_original_offsets() {
    let (root, worker) = mock(vec![
        ("/book/B005HF3EJC?region=us", book()),
        (
            "/book/B005HF3EJC/chapters?region=us",
            json!({"isAccurate": true, "chapters": [
                {"title": "Part", "startOffsetMs": 0, "lengthMs": 180000, "chapters": [
                    {"title": "Chapter One", "startOffsetMs": 0, "lengthMs": 35250},
                    {"title": "Chapter Two", "startOffsetMs": 35250, "lengthMs": 144750}
                ]},
                {"title": "Invalid", "startOffsetMs": -1, "lengthMs": 2}
            ]}),
        ),
    ]);
    let snapshot = TEST_API_ROOT
        .scope(
            root,
            LibexMetadataProvider.fetch_snapshot(
                &MetadataSettings::default(),
                "us/B005HF3EJC/recording",
                "audiobook",
                true,
            ),
        )
        .await
        .unwrap();
    worker.join().unwrap();
    let metadata = recording_metadata(snapshot.provider_payload_json.as_deref().unwrap()).unwrap();
    assert_eq!(metadata.narrator, "Felbrigg Napoleon Herriot");
    assert_eq!(metadata.chapters.len(), 2);
    assert_eq!(metadata.chapters[1].start_ms, 35_250);
    assert_eq!(metadata.series_position.as_deref(), Some("2.5"));
}

#[tokio::test]
async fn inaccurate_chapter_data_does_not_prevent_recording_metadata() {
    let (root, worker) = mock(vec![
        ("/book/B005HF3EJC", book()),
        (
            "/book/B005HF3EJC/chapters",
            json!({"isAccurate": false,
            "chapters": [{"title": "Estimated",
            "startOffsetMs": 0,
            "lengthMs": 1000}]}),
        ),
    ]);
    let snapshot = TEST_API_ROOT
        .scope(
            root,
            LibexMetadataProvider.fetch_snapshot(
                &MetadataSettings::default(),
                "us/B005HF3EJC/recording",
                "audiobook",
                true,
            ),
        )
        .await
        .unwrap();
    worker.join().unwrap();
    assert!(
        recording_metadata(snapshot.provider_payload_json.as_deref().unwrap())
            .unwrap()
            .chapters
            .is_empty()
    );
}

#[tokio::test]
async fn invalid_regions_identifiers_and_recording_suffixes_do_not_make_requests() {
    for id in [
        "us/../book",
        "xx/B005HF3EJC",
        "us/short",
        "us/B005HF3EJC/other",
        "us/B005HF3EJC/recording/more",
    ] {
        assert!(
            LibexMetadataProvider
                .fetch_snapshot(&MetadataSettings::default(), id, "audiobook", true)
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn folder_title_can_match_an_author_possessive_without_selecting_a_recording() {
    let mut source = book();
    source["asin"] = json!("B002V1CDCO");
    source["title"] =
        json!("Glenn Beck's Common Sense: The Case Against an Out-of-Control Government");
    source["authors"] = json!([{"name": "Glenn Beck"}]);
    let (root, worker) = mock(vec![("author=GLENN+BECK", json!([source]))]);
    let metadata = AudiobookMetadata {
        title: "COMMON SENSE".into(),
        author: "GLENN BECK".into(),
        ..Default::default()
    };
    let result = TEST_API_ROOT
        .scope(root, guess(&MetadataSettings::default(), &metadata))
        .await
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    assert_eq!(result.external_id, "us/B002V1CDCO");
    assert!(!work_title_matches(
        "Common Sense",
        "Another Book",
        "Glenn Beck"
    ));
}
