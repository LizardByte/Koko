//! Provider rating scales, persistence, refreshes, and public API presentation.

// Compile the shared schema without exposing database internals to library consumers.
#[path = "../src/db/schema.rs"]
mod schema;

#[path = "ratings/migration.rs"]
mod migration;

use diesel::prelude::*;
use koko::config::MetadataProviderId;
use koko::metadata::{
    ProviderMetadataDetails,
    StoredMetadataSnapshot,
    get_item_metadata_summaries,
    get_primary_item_metadata_link,
    upsert_item_metadata_link,
    upsert_item_metadata_snapshot,
};
use schema::{
    item_metadata_links,
    media_items,
    media_libraries,
};
use serde_json::json;

fn connection() -> SqliteConnection {
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    koko::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    diesel::insert_into(media_libraries::table)
        .values((
            media_libraries::id.eq(1),
            media_libraries::name.eq("Books"),
            media_libraries::path.eq("books"),
            media_libraries::kind.eq("books"),
        ))
        .execute(&mut conn)
        .unwrap();
    diesel::insert_into(media_items::table)
        .values((
            media_items::id.eq(1),
            media_items::library_id.eq(1),
            media_items::identity_key.eq("book"),
            media_items::item_type.eq("book_work"),
            media_items::display_title.eq("Book"),
        ))
        .execute(&mut conn)
        .unwrap();
    conn
}

fn snapshot(
    provider_id: MetadataProviderId,
    payload: serde_json::Value,
) -> StoredMetadataSnapshot {
    StoredMetadataSnapshot {
        provider_id,
        external_id: "works/OL1W".into(),
        media_type: Some("book".into()),
        title: Some("Book".into()),
        overview: None,
        artwork_url: None,
        backdrop_url: None,
        release_year: None,
        locale_key: String::new(),
        provider_locale_key: None,
        provider_payload_json: Some(payload.to_string()),
    }
}

#[test]
fn open_library_averages_use_shared_rating_storage_and_api_attribution() {
    let mut conn = connection();
    for (average, stored, expected) in [
        (json!(4.125), Some(83), Some(8.3)),
        (json!(5), Some(100), Some(10.0)),
        (json!(null), None, None),
    ] {
        let source = snapshot(
            MetadataProviderId::OpenLibrary,
            json!({
                "key":"/works/OL1W", "title":"Book", "koko_rating_average":average,
            }),
        );
        let saved = upsert_item_metadata_snapshot(&mut conn, 1, &source).unwrap();
        let row = get_primary_item_metadata_link(&mut conn, 1)
            .unwrap()
            .unwrap();
        assert_eq!(row.rating, stored);
        assert_eq!(saved.rating, expected);
        assert_eq!(
            get_item_metadata_summaries(&mut conn, 1).unwrap()[0].rating,
            expected
        );
        let summary = koko::media::get_media_item_summary(&mut conn, 1)
            .unwrap()
            .unwrap();
        let detail = koko::media::get_media_item(&mut conn, 1, ".")
            .unwrap()
            .unwrap();
        assert_eq!(summary.rating, expected);
        assert_eq!(detail.rating, expected);
        let provider = expected.map(|_| "open_library");
        assert_eq!(summary.rating_provider_id.as_deref(), provider);
        assert_eq!(detail.rating_provider_id.as_deref(), provider);
        assert_eq!(
            detail.rating_source_item_type.as_deref(),
            expected.map(|_| "book_work")
        );
    }
}

#[test]
fn providers_default_to_ten_and_round_to_display_precision() {
    let mut conn = connection();
    let source = snapshot(MetadataProviderId::Tmdb, json!({"vote_average":8.25}));
    let saved = upsert_item_metadata_snapshot(&mut conn, 1, &source).unwrap();
    assert_eq!(saved.rating, Some(8.3));
    assert_eq!(
        get_primary_item_metadata_link(&mut conn, 1)
            .unwrap()
            .unwrap()
            .rating,
        Some(83)
    );
}

#[test]
fn every_metadata_link_validates_against_its_provider_scale_before_saving() {
    let mut conn = connection();
    for (provider, maximum) in [
        (MetadataProviderId::Tmdb, 10.0),
        (MetadataProviderId::OpenLibrary, 5.0),
    ] {
        let source = snapshot(provider.clone(), json!({}));
        for relation in ["primary", "secondary"] {
            for value in [
                0.0,
                maximum,
                -1.0,
                maximum + 0.1,
                f64::NAN,
                f64::INFINITY,
                f64::NEG_INFINITY,
            ] {
                let expected = if value == 0.0 {
                    Some(0)
                } else if value == maximum {
                    Some(100)
                } else {
                    None
                };
                let details = ProviderMetadataDetails {
                    rating: Some(value),
                    ..Default::default()
                };
                let saved =
                    upsert_item_metadata_link(&mut conn, 1, &source, &details, relation, None)
                        .unwrap();
                let stored: Option<i16> = item_metadata_links::table
                    .find(saved.id)
                    .select(item_metadata_links::rating)
                    .first(&mut conn)
                    .unwrap();
                assert_eq!(stored, expected, "{provider:?} {relation} {value}");
                assert_eq!(saved.rating, expected.map(|value| f32::from(value) / 10.0));
            }
        }
    }
}

#[test]
fn database_rejects_out_of_range_scores_without_changing_saved_ratings() {
    let mut conn = connection();
    let source = snapshot(
        MetadataProviderId::OpenLibrary,
        json!({
            "key":"/works/OL1W", "title":"Book", "koko_rating_average":4.125,
        }),
    );
    let saved = upsert_item_metadata_snapshot(&mut conn, 1, &source).unwrap();
    for value in [-1_i16, 101] {
        assert!(
            diesel::update(item_metadata_links::table.find(saved.id))
                .set(item_metadata_links::rating.eq(value))
                .execute(&mut conn)
                .is_err()
        );
    }
    assert_eq!(
        get_item_metadata_summaries(&mut conn, 1).unwrap()[0].rating,
        Some(8.3)
    );
    let detail = koko::media::get_media_item(&mut conn, 1, ".")
        .unwrap()
        .unwrap();
    assert_eq!(detail.rating, Some(8.3));
    assert_eq!(detail.rating_provider_id.as_deref(), Some("open_library"));
    let summary = koko::media::get_media_item_summary(&mut conn, 1)
        .unwrap()
        .unwrap();
    assert_eq!(summary.rating, Some(8.3));
    assert_eq!(summary.rating_provider_id.as_deref(), Some("open_library"));
}

#[test]
fn rating_outages_preserve_only_the_same_works_previous_normalized_score() {
    let mut conn = connection();
    let mut source = snapshot(
        MetadataProviderId::OpenLibrary,
        json!({
            "key":"/works/OL1W", "title":"Book", "koko_rating_average":4.25,
        }),
    );
    upsert_item_metadata_snapshot(&mut conn, 1, &source).unwrap();
    source.provider_payload_json = Some(
        json!({
            "key":"/works/OL1W", "title":"Book", "koko_rating_incomplete":true,
        })
        .to_string(),
    );
    assert_eq!(
        upsert_item_metadata_snapshot(&mut conn, 1, &source)
            .unwrap()
            .rating,
        Some(8.5)
    );
    source.external_id = "works/OL2W".into();
    source.provider_payload_json = Some(
        json!({
            "key":"/works/OL2W", "title":"Other Book", "koko_rating_incomplete":true,
        })
        .to_string(),
    );
    assert_eq!(
        upsert_item_metadata_snapshot(&mut conn, 1, &source)
            .unwrap()
            .rating,
        None
    );
}
