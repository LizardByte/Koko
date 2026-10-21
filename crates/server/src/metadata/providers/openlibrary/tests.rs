//! Book work lookup and recording isolation through local HTTP fixtures.

use super::*;
use crate::metadata::providers::audiosilo::tests::mock;
use serde_json::json;

#[tokio::test]
async fn missing_or_unavailable_ratings_do_not_block_work_metadata_or_trust_injected_values() {
    for (status, payload) in [
        (200, json!({"summary":{"average":null,"count":0}})),
        (200, json!({})),
        (200, json!({"summary":{"average":"bad"}})),
        (404, json!({"error":"notfound"})),
        (503, json!({"error":"temporarily unavailable"})),
    ] {
        let (root, worker) = crate::metadata::providers::audiosilo::tests::mock_status(vec![
            (
                "/works/OL1W.json",
                200,
                json!({"key":"/works/OL1W","title":"A Book","koko_rating_average":5.0}),
            ),
            ("/works/OL1W/ratings.json", status, payload),
            ("/query.json", 200, json!([])),
        ]);
        let snapshot = TEST_API_ROOT
            .scope(
                root,
                OpenLibraryMetadataProvider.fetch_snapshot(
                    &MetadataSettings::default(),
                    "works/OL1W",
                    "book",
                    false,
                ),
            )
            .await
            .unwrap();
        worker.join().unwrap();
        assert_eq!(snapshot.title.as_deref(), Some("A Book"));
        assert_eq!(
            OpenLibraryMetadataProvider
                .metadata_details(&snapshot)
                .rating,
            None
        );
        assert_eq!(
            OpenLibraryMetadataProvider
                .metadata_details(&snapshot)
                .rating_incomplete,
            status != 200
        );
    }
}

#[tokio::test]
async fn folder_title_and_author_match_a_unique_possessive_work_title() {
    let (root, worker) = mock(vec![(
        "/search.json?title=COMMON",
        json!({"docs": [{"key": "/works/OL6036787W", "title": "Glenn Beck's common sense",
            "author_name": ["Glenn Beck"], "cover_i": 5735496, "first_publish_year": 2009}]}),
    )]);
    let result = TEST_API_ROOT
        .scope(root, guess("COMMON SENSE", "GLENN BECK"))
        .await
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    assert_eq!(result.external_id, "works/OL6036787W");
    assert_eq!(result.media_type, "book");
    assert!(result.artwork_url.unwrap().contains("5735496"));
}

#[tokio::test]
async fn work_snapshot_resolves_authors_and_cannot_supply_narration_or_chapters() {
    let (root, worker) = mock(vec![
        (
            "/works/OL6036787W.json",
            json!({"key": "/works/OL6036787W", "title": "Common Sense",
                "authors": [{"author": {"key": "/authors/OL1A"}}],
                "description": {"value": "Book description"}, "covers": [-1,5735496],
                "subjects": ["History"], "koko_author_names": ["Untrusted"],
                "koko_rating_average": 1.0,
                "koko_rating_incomplete": true,
                "koko_collections": [{"key":"/collections/forged", "title":"Untrusted"}]}),
        ),
        (
            "/works/OL6036787W/ratings.json",
            json!({"summary":{"average":4.25,"count":100},"counts":{"4":75,"5":25}}),
        ),
        ("/authors/OL1A.json", json!({"name": "Glenn Beck"})),
        ("/query.json", json!([])),
    ]);
    let snapshot = TEST_API_ROOT
        .scope(
            root,
            OpenLibraryMetadataProvider.fetch_snapshot(
                &MetadataSettings::default(),
                "works/OL6036787W",
                "book",
                false,
            ),
        )
        .await
        .unwrap();
    worker.join().unwrap();
    let metadata = work_metadata(snapshot.provider_payload_json.as_deref().unwrap()).unwrap();
    assert_eq!(metadata.author, "Glenn Beck");
    assert_eq!(
        metadata.openlibrary_work_id.as_deref(),
        Some("works/OL6036787W")
    );
    assert!(metadata.narrator.is_empty());
    assert!(metadata.chapters.is_empty());
    assert_eq!(snapshot.overview.as_deref(), Some("Book description"));
    assert_eq!(
        OpenLibraryMetadataProvider
            .metadata_details(&snapshot)
            .rating,
        Some(4.25)
    );
    assert!(
        !OpenLibraryMetadataProvider
            .metadata_details(&snapshot)
            .rating_incomplete
    );
    assert!(
        !snapshot
            .provider_payload_json
            .as_deref()
            .unwrap()
            .contains("\"count\"")
    );
    assert!(
        OpenLibraryMetadataProvider
            .metadata_details(&snapshot)
            .collections
            .is_empty()
    );
}

#[tokio::test]
async fn ambiguous_works_and_wrong_authors_do_not_auto_match() {
    let (root, worker) = mock(vec![(
        "/search.json",
        json!({"docs": [
            {"key": "/works/OL1W", "title": "Common Sense", "author_name": ["Glenn Beck"]},
            {"key": "/works/OL2W", "title": "Common Sense", "author_name": ["Glenn Beck"]},
            {"key": "/works/OL3W", "title": "Common Sense", "author_name": ["Thomas Paine"]}
        ]}),
    )]);
    assert!(
        TEST_API_ROOT
            .scope(root, guess("Common Sense", "Glenn Beck"))
            .await
            .unwrap()
            .is_none()
    );
    worker.join().unwrap();
    assert!(guess("Common Sense", "").await.unwrap().is_none());
    for id in [
        "../works/OL1W",
        "works/OL1W/other",
        "works/OL1M",
        "https://example.com",
    ] {
        assert!(
            OpenLibraryMetadataProvider
                .fetch_snapshot(&MetadataSettings::default(), id, "book", false)
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn unavailable_collection_catalog_does_not_block_the_book_or_clear_memberships() {
    let (root, worker) = mock(vec![
        (
            "/works/OL1W.json",
            json!({"key":"/works/OL1W", "title":"A Book"}),
        ),
        (
            "/works/OL1W/ratings.json",
            json!({"summary":{"average":null,"count":0}}),
        ),
        ("/query.json", json!({"message":"temporarily unavailable"})),
    ]);
    let snapshot = TEST_API_ROOT
        .scope(
            root,
            OpenLibraryMetadataProvider.fetch_snapshot(
                &MetadataSettings::default(),
                "works/OL1W",
                "book",
                false,
            ),
        )
        .await
        .unwrap();
    worker.join().unwrap();
    assert_eq!(snapshot.title.as_deref(), Some("A Book"));
    assert!(
        OpenLibraryMetadataProvider
            .metadata_details(&snapshot)
            .collections_incomplete
    );
}

#[tokio::test]
async fn author_search_uses_author_identities_and_requires_a_unique_name_or_alias() {
    let (root, worker) = mock(vec![(
        "/search/authors.json?q=GEOFFREY",
        json!({
            "numFound": 3, "docs": [
                {"key": "OL1A", "name": "Geoffrey Malins",
                    "alternate_names": ["Geoffrey H. Malins"],
                    "top_work": "How I Filmed the War", "work_count": 5},
                {"key": "/authors/OL2A", "name": "Another Author"},
                {"key": "../OL3A", "name": "Geoffrey H. Malins"}
            ]
        }),
    )]);
    let result = TEST_API_ROOT
        .scope(root, guess_author("GEOFFREY H MALINS"))
        .await
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    assert_eq!(result.external_id, "authors/OL1A");
    assert_eq!(result.media_type, "author");
    assert_eq!(result.title, "Geoffrey Malins");
    assert!(result.artwork_url.unwrap().contains("/a/olid/OL1A-L.jpg"));
    assert_eq!(
        result.overview.as_deref(),
        Some("Known for: How I Filmed the War · 5 works listed")
    );
    for payload in [
        json!({"numFound": 2, "docs": [
            {"key": "OL1A", "name": "Same Name"},
            {"key": "OL2A", "name": "Same Name"}
        ]}),
        json!({"numFound": 20, "docs": [{"key": "OL1A", "name": "Same Name"}]}),
    ] {
        let (root, worker) = mock(vec![("/search/authors.json", payload)]);
        assert!(
            TEST_API_ROOT
                .scope(root, guess_author("Same Name"))
                .await
                .unwrap()
                .is_none()
        );
        worker.join().unwrap();
    }
    assert!(guess_author("").await.unwrap().is_none());
    assert!(guess_author("Unknown author").await.unwrap().is_none());
}

#[tokio::test]
async fn author_snapshots_normalize_biographies_photos_and_lifespan_without_book_metadata() {
    for bio in [
        json!("Author biography"),
        json!({"value": "Author biography"}),
    ] {
        let (root, worker) = mock(vec![(
            "/authors/OL1A.json",
            json!({
                "key": "/authors/OL1A", "name": "Geoffrey Malins", "bio": bio,
                "birth_date": "18 November 1886", "death_date": "11 February 1940",
                "photos": [-1, 123]
            }),
        )]);
        let snapshot = TEST_API_ROOT
            .scope(
                root,
                OpenLibraryMetadataProvider.fetch_snapshot(
                    &MetadataSettings::default(),
                    "authors/OL1A",
                    "author",
                    false,
                ),
            )
            .await
            .unwrap();
        worker.join().unwrap();
        assert_eq!(snapshot.overview.as_deref(), Some("Author biography"));
        assert_eq!(
            snapshot.artwork_url.as_deref(),
            Some("https://covers.openlibrary.org/a/id/123-L.jpg?default=false")
        );
        let details = OpenLibraryMetadataProvider.metadata_details(&snapshot);
        assert_eq!(
            details.tagline.as_deref(),
            Some("Born 18 November 1886 · Died 11 February 1940")
        );
        assert_eq!(details.external_ids[0].external_id, "authors/OL1A");
        assert!(snapshot.release_year.is_none());
        assert!(work_metadata(snapshot.provider_payload_json.as_deref().unwrap()).is_none());
    }
}

#[tokio::test]
async fn manual_author_search_keeps_namesakes_and_rejects_cross_type_or_untrusted_identities() {
    let (root, worker) = mock(vec![(
        "/search/authors.json",
        json!({"docs": [
            {"key": "OL1A", "name": "Same Name", "birth_date": "1900"},
            {"key": "/authors/OL2A", "name": "Same Name", "birth_date": "1950"}
        ]}),
    )]);
    let results = TEST_API_ROOT
        .scope(
            root,
            OpenLibraryMetadataProvider.search_with_artist(
                &MetadataSettings::default(),
                "Same Name",
                Some("author"),
                None,
            ),
        )
        .await
        .unwrap();
    worker.join().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].overview.as_deref(), Some("Born 1900"));
    for (id, kind) in [
        ("../authors/OL1A", "author"),
        ("authors/OL1A/other", "author"),
        ("https://example.com/OL1A", "author"),
        ("works/OL1W", "author"),
        ("authors/OL1A", "book"),
        ("works/OL1W", "recording"),
    ] {
        assert!(
            OpenLibraryMetadataProvider
                .fetch_snapshot(&MetadataSettings::default(), id, kind, false)
                .await
                .is_err()
        );
    }
    for payload in [
        json!({"key": "/authors/OL2A", "name": "Wrong author"}),
        json!({"key": "/authors/OL1A"}),
    ] {
        let (root, worker) = mock(vec![("/authors/OL1A.json", payload)]);
        assert!(
            TEST_API_ROOT
                .scope(
                    root,
                    OpenLibraryMetadataProvider.fetch_snapshot(
                        &MetadataSettings::default(),
                        "authors/OL1A",
                        "author",
                        false,
                    )
                )
                .await
                .is_err()
        );
        worker.join().unwrap();
    }
}
