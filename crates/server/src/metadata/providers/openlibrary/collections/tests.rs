//! Curated membership is authoritative, deterministic, and separate from personal lists.

use super::*;
use crate::metadata::providers::audiosilo::tests::{
    mock,
    mock_status,
};
use serde_json::json;

fn page(
    key: &str,
    title: &str,
    body: &str,
) -> Value {
    json!({"key":key, "title":title, "body":{"value":body}})
}

#[test]
fn literal_carousels_support_named_arguments_and_do_not_evaluate_code() {
    let body = r#"
        {{QueryCarousel(title="A title", query='subject:"collectionID:Animorphs"', limit=150)}}
        {{QueryCarousel('key:(/works/OL1W OR /works/OL2W)', title="Books")}}
        {{ListCarousel("/people/curator/lists/OL42L/Books", title="A curated list")}}
        {{ListCarousel("https://example.com/people/curator/lists/OL42L")}}
        {{QueryCarousel(__import__('os').system('unsafe'))}}
    "#;
    assert_eq!(
        carousels(body),
        vec![
            Carousel::Query("subject:\"collectionID:Animorphs\"".into()),
            Carousel::Query("key:(/works/OL1W OR /works/OL2W)".into()),
            Carousel::List("/people/curator/lists/OL42L".into()),
        ]
    );
    assert_eq!(
        explicit_work_match("key:(/works/OL1W OR /works/OL2W)", "works/OL1W"),
        Some(true)
    );
    assert_eq!(
        explicit_work_match("work:(\"OL2W\")", "works/OL1W"),
        Some(false)
    );
    for query in [
        "key:/works/OL1W AND subject:fiction",
        "key:(/works/OL1W AND /works/OL2W)",
        "NOT key:/works/OL1W",
    ] {
        assert_eq!(explicit_work_match(query, "works/OL1W"), None);
    }
    for key in [
        "/people/patron",
        "/collections/../escape",
        "/collections/test?redirect=other",
        "/collections//empty",
    ] {
        assert!(!valid_page(key));
    }
}

#[tokio::test]
async fn curated_pages_resolve_explicit_queries_and_only_their_referenced_lists() {
    let (root, worker) = mock(vec![
        (
            "/query.json",
            json!([
                page(
                    "/collections/a-explicit",
                    "The Curated Books",
                    "{{QueryCarousel('key:(/works/OL1W OR /works/OL2W)')}}"
                ),
                page(
                    "/collections/a-explicit",
                    "Duplicate",
                    "{{QueryCarousel('key:/works/OL1W')}}"
                ),
                page(
                    "/collections/b-list",
                    "Curator's Collection",
                    "{{ListCarousel('/people/curator/lists/OL42L')}}"
                ),
                page(
                    "/collections/c-query",
                    "History",
                    "{{QueryCarousel('subject:history AND first_publish_year:[1900 TO *]')}}"
                ),
                page(
                    "/collections/d-unrelated",
                    "Dragons",
                    "{{QueryCarousel('subject:dragons')}}"
                ),
                page(
                    "/collections/e-other-work",
                    "Another Work",
                    "{{QueryCarousel('key:/works/OL2W')}}"
                ),
                page(
                    "/people/personal",
                    "Personal",
                    "{{QueryCarousel('key:/works/OL1W')}}"
                )
            ]),
        ),
        (
            "/works/OL1W/lists.json",
            json!({"size":2,"entries":[
                {"url":"/people/curator/lists/OL42L", "name":"Underlying list"},
                {"url":"/people/patron/lists/OL99L", "name":"My to read list"}
            ]}),
        ),
        (
            "/search.json?q=key%3A%2Fworks%2FOL1W",
            json!({"docs":[{"key":"/works/OL1W"}]}),
        ),
        ("/search.json?q=key%3A%2Fworks%2FOL1W", json!({"docs":[]})),
    ]);
    let found = TEST_API_ROOT
        .scope(root, memberships("works/OL1W"))
        .await
        .unwrap();
    worker.join().unwrap();
    assert!(found.unavailable.is_empty());
    let collections = found.collections;
    assert_eq!(
        collections
            .iter()
            .map(|collection| collection.key.as_str())
            .collect::<Vec<_>>(),
        [
            "/collections/a-explicit",
            "/collections/b-list",
            "/collections/c-query"
        ]
    );
    assert_eq!(collections[1].title, "Curator's Collection");
    assert!(
        collections
            .iter()
            .all(|collection| !collection.title.contains("to read"))
    );
}

#[tokio::test]
async fn invalid_membership_data_fails_instead_of_erasing_existing_collections() {
    for payload in [
        json!({"message":"temporarily unavailable"}),
        json!({"docs":[{"key":"/works/OL2W"}]}),
    ] {
        let (root, worker) = mock(vec![
            (
                "/query.json",
                json!([page(
                    "/collections/history",
                    "History",
                    "{{QueryCarousel('subject:history')}}"
                )]),
            ),
            ("/search.json", payload),
        ]);
        let found = TEST_API_ROOT.scope(root, memberships("works/OL1W")).await;
        assert!(found.is_err());
        worker.join().unwrap();
    }
}

#[tokio::test]
async fn a_broken_curator_query_preserves_only_that_collection_and_keeps_resolving_valid_pages() {
    let (root, worker) = mock_status(vec![
        (
            "/query.json",
            200,
            json!([
                page(
                    "/collections/a-broken",
                    "Broken",
                    "{{QueryCarousel('subject:(broken')}}"
                ),
                page(
                    "/collections/b-history",
                    "History",
                    "{{QueryCarousel('subject:history')}}"
                )
            ]),
        ),
        ("/search.json", 400, json!({"message":"Invalid query"})),
        ("/search.json", 400, json!({"message":"Invalid query"})),
        ("/search.json", 200, json!({"docs":[{"key":"/works/OL1W"}]})),
    ]);
    let found = TEST_API_ROOT
        .scope(root, memberships("works/OL1W"))
        .await
        .unwrap();
    worker.join().unwrap();
    assert_eq!(found.unavailable, ["/collections/a-broken"]);
    assert_eq!(found.collections[0].key, "/collections/b-history");
}
