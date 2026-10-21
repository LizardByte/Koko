//! Public-provider contract checks against bounded local HTTP fixtures.

use super::*;
use std::io::{
    Read,
    Write,
};

pub(crate) fn mock(responses: Vec<(&'static str, Value)>) -> (String, std::thread::JoinHandle<()>) {
    mock_status(
        responses
            .into_iter()
            .map(|(path, value)| (path, 200, value))
            .collect(),
    )
}

pub(crate) fn mock_status(
    responses: Vec<(&'static str, u16, Value)>
) -> (String, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let root = format!("http://{}", listener.local_addr().unwrap());
    let worker = std::thread::spawn(move || {
        for (expected, status, payload) in responses {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            let mut stream = loop {
                if let Ok((stream, _)) = listener.accept() {
                    break stream;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "missing request for {expected}"
                );
                std::thread::sleep(std::time::Duration::from_millis(5));
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut bytes = [0; 8192];
            let size = stream.read(&mut bytes).unwrap();
            let request = String::from_utf8_lossy(&bytes[..size]);
            assert!(request.contains(expected), "unexpected request: {request}");
            let body = payload.to_string();
            write!(
                stream,
                "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: \
                 {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    (root, worker)
}

fn work() -> Value {
    serde_json::json!({
        "id": "the-book",
            "title": "The Book",
            "authors": [{"id": "author",
            "name": "The Author"}],
            "language": "en",
        "series": [{"id": "series", "name": "The Series", "position": "2"}],
        "recordings": [{"id": "reader-2020",
            "narrators": [{"id": "reader",
            "name": "The Reader"}],
            "runtime_min": 3,
            "release_date": "2020-01-01"}],
        "description": "External prose is not imported",
            "community_description": {"text": "Share-alike prose"},
            "recaps": [{"text": "Spoiler"}]
    })
}

#[test]
fn recording_choices_keep_each_narrations_cover_without_substituting_another_version() {
    let source: Work = serde_json::from_value(work()).unwrap();
    let mut first = source.recordings.as_ref().unwrap()[0].clone();
    first.cover_url = Some("https://covers.example/reader-one.jpg".into());
    let mut second = first.clone();
    second.id = "another-reader".into();
    second.cover_url = Some("https://covers.example/reader-two.jpg".into());
    let one = recording_identity(&source, &first);
    let two = recording_identity(&source, &second);
    assert_eq!(
        one.artwork_url.as_deref(),
        Some("https://covers.example/reader-one.jpg")
    );
    assert_eq!(
        two.artwork_url.as_deref(),
        Some("https://covers.example/reader-two.jpg")
    );
    second.cover_url = None;
    assert_eq!(recording_identity(&source, &second).artwork_url, None);
}

#[tokio::test]
async fn search_exposes_distinct_work_and_recording_choices() {
    let (root, worker) = mock(vec![
        (
            "/works/search?q=Book",
            serde_json::json!({"results": [{"id": "the-book",
                "title": "The Book",
                "authors": [{"id": "author",
                "name": "The Author"}]}]}),
        ),
        ("/works/the-book", work()),
    ]);
    let result = TEST_API_ROOT
        .scope(
            root,
            AudioSiloMetadataProvider.search(
                &MetadataSettings::default(),
                "Book",
                Some("audiobook"),
            ),
        )
        .await
        .unwrap();
    worker.join().unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].external_id, "the-book");
    assert_eq!(result[1].external_id, "the-book/reader-2020");
    assert!(result[1].title.contains("The Reader"));
}

#[tokio::test]
async fn selected_recording_fetches_chapters_and_keeps_only_the_cc0_fields() {
    let (root, worker) = mock(vec![
        ("/works/the-book", work()),
        (
            "/works/the-book/recordings/reader-2020/chapters",
            serde_json::json!({"chapters": [{"title": "Mid-file",
                "start_ms": 35_250,
                "length_ms": 144_750},
                {"title": "Bad",
                "start_ms": -1,
                "length_ms": 3}]}),
        ),
    ]);
    let snapshot = TEST_API_ROOT
        .scope(
            root,
            AudioSiloMetadataProvider.fetch_snapshot(
                &MetadataSettings::default(),
                "the-book/reader-2020",
                "audiobook",
                true,
            ),
        )
        .await
        .unwrap();
    worker.join().unwrap();
    let raw = snapshot.provider_payload_json.as_deref().unwrap();
    for discarded in [
        "External prose",
        "Share-alike prose",
        "Spoiler",
    ] {
        assert!(!raw.contains(discarded));
    }
    assert!(snapshot.overview.is_none());
    let metadata = recording_metadata(raw).unwrap();
    assert_eq!(metadata.narrator, "The Reader");
    assert_eq!(metadata.chapters.len(), 1);
    assert_eq!(metadata.chapters[0].start_ms, 35_250);
    assert_eq!(metadata.chapters[0].end_ms, 180_000);
}

#[tokio::test]
async fn work_matches_do_not_guess_the_recording_or_apply_chapters() {
    let (root, worker) = mock(vec![("/works/the-book", work())]);
    let snapshot = TEST_API_ROOT
        .scope(
            root,
            AudioSiloMetadataProvider.fetch_snapshot(
                &MetadataSettings::default(),
                "the-book",
                "audiobook",
                true,
            ),
        )
        .await
        .unwrap();
    worker.join().unwrap();
    let metadata = recording_metadata(snapshot.provider_payload_json.as_deref().unwrap()).unwrap();
    assert!(metadata.narrator.is_empty());
    assert!(metadata.chapters.is_empty());
    assert_eq!(metadata.author, "The Author");
}

#[tokio::test]
async fn automatic_match_requires_a_unique_high_score_and_never_selects_a_recording() {
    let metadata = crate::media::AudiobookMetadata {
        title: "The Book".into(),
        author: "The Author".into(),
        ..Default::default()
    };
    let (root, worker) = mock(vec![(
        "author=The+Author",
        serde_json::json!({"results": [
            {"id": "the-book", "title": "The Book", "score": 100, "recording_id": "reader-2020"},
            {"id": "another", "title": "Another", "score": 80}
        ]}),
    )]);
    let result = TEST_API_ROOT
        .scope(root, guess(&metadata, Some(180_000)))
        .await
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    assert_eq!(result.external_id, "the-book");
    let (root, worker) = mock(vec![(
        "/works/match",
        serde_json::json!({"results": [
            {"id": "one",
                "title": "The Book",
                "score": 100},
                {"id": "two",
                "title": "The Book",
                "score": 99}
        ]}),
    )]);
    assert!(
        TEST_API_ROOT
            .scope(root, guess(&metadata, None))
            .await
            .unwrap()
            .is_none()
    );
    worker.join().unwrap();
}

#[tokio::test]
async fn malformed_ids_are_rejected_before_any_network_request() {
    for id in [
        "../other",
        "book/../../other",
        "https://host/book",
        "book?query=1",
    ] {
        assert!(
            AudioSiloMetadataProvider
                .fetch_snapshot(&MetadataSettings::default(), id, "audiobook", true)
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn libex_work_enrichment_uses_asin_without_assuming_its_recording() {
    let (root, worker) = mock(vec![
        (
            "/lookup?asin=B002V1CDCO",
            serde_json::json!({"work": {"id": "the-book"}, "recording_id": "reader-2020"}),
        ),
        ("/works/the-book", work()),
    ]);
    let details = TEST_API_ROOT
        .scope(
            root,
            AudioSiloMetadataProvider.fetch_secondary_metadata(
                &MetadataSettings::default(),
                "audiobook",
                "libex",
                "us/B002V1CDCO",
                "",
            ),
        )
        .await
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    let metadata = recording_metadata(details.provider_payload_json.as_deref().unwrap()).unwrap();
    assert_eq!(metadata.series.as_deref(), Some("The Series"));
    assert!(metadata.narrator.is_empty());
    assert!(metadata.chapters.is_empty());
    assert_eq!(
        AudioSiloMetadataProvider.descriptor().role,
        MetadataProviderRole::Secondary
    );
}

#[tokio::test]
async fn selected_libex_recording_can_receive_exact_asin_chapters() {
    let (root, worker) = mock(vec![
        (
            "/lookup?asin=B002V1CDCO",
            serde_json::json!({"work": {"id": "the-book"}, "recording_id": "reader-2020"}),
        ),
        ("/works/the-book", work()),
        (
            "/works/the-book/recordings/reader-2020/chapters",
            serde_json::json!({"chapters": [{"title": "Chapter",
                "start_ms": 35250, "length_ms": 144750}]}),
        ),
    ]);
    let details = TEST_API_ROOT
        .scope(
            root,
            AudioSiloMetadataProvider.fetch_secondary_metadata(
                &MetadataSettings::default(),
                "audiobook",
                "libex",
                "us/B002V1CDCO/recording",
                "",
            ),
        )
        .await
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    let metadata = recording_metadata(details.provider_payload_json.as_deref().unwrap()).unwrap();
    assert_eq!(metadata.narrator, "The Reader");
    assert_eq!(metadata.chapters[0].start_ms, 35250);
}
