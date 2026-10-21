//! Upgrade and downgrade coverage for the rating column with dependent records in place.

use super::{
    connection,
    schema,
};
use diesel::{
    connection::SimpleConnection,
    prelude::*,
};

// This typed fixture describes the previous floating-point column on the same table.
mod legacy {
    diesel::table! {
        item_metadata_links (id) {
            id -> Integer,
            media_item_id -> Integer,
            provider_id -> Text,
            external_id -> Text,
            locale_key -> Text,
            rating -> Nullable<Double>,
        }
    }
}

const UP: &str = include_str!("../../sql/migrations/834267a910bb_integer_ratings/up.sql");
const DOWN: &str = include_str!("../../sql/migrations/834267a910bb_integer_ratings/down.sql");

#[test]
fn migration_rounds_existing_scores_and_preserves_nulls_links_and_foreign_keys() {
    use legacy::item_metadata_links as old;
    use schema::{
        item_metadata_external_ids as ids,
        item_metadata_links as links,
    };
    let mut conn = connection();
    koko::db::configure_sqlite_connection(&mut conn).unwrap();
    conn.batch_execute(DOWN).unwrap();

    for (index, score) in [
        None,
        Some(0.0),
        Some(10.0),
        Some(7.8),
        Some(8.25),
        Some(9.99),
        Some(-1.0),
        Some(10.01),
        Some(f64::INFINITY),
    ]
    .into_iter()
    .enumerate()
    {
        let id = i32::try_from(index + 1).unwrap();
        diesel::insert_into(old::table)
            .values((
                old::id.eq(id),
                old::media_item_id.eq(1),
                old::provider_id.eq("tmdb"),
                old::external_id.eq(id.to_string()),
                old::locale_key.eq(format!("test-{id}")),
                old::rating.eq(score),
            ))
            .execute(&mut conn)
            .unwrap();
    }
    diesel::insert_into(ids::table)
        .values((
            ids::metadata_link_id.eq(1),
            ids::source.eq("imdb"),
            ids::external_id.eq("tt1"),
        ))
        .execute(&mut conn)
        .unwrap();

    conn.batch_execute(UP).unwrap();
    let expected = vec![
        None,
        Some(0),
        Some(100),
        Some(78),
        Some(83),
        Some(100),
        None,
        None,
        None,
    ];
    let scores = links::table
        .order(links::id)
        .select(links::rating)
        .load::<Option<i16>>(&mut conn)
        .unwrap();
    assert_eq!(scores, expected);
    assert_eq!(
        links::table
            .order(links::id)
            .select(links::external_id)
            .load::<String>(&mut conn)
            .unwrap(),
        (1..=9).map(|id| id.to_string()).collect::<Vec<_>>()
    );
    assert_eq!(
        ids::table
            .select((ids::metadata_link_id, ids::external_id))
            .first::<(i32, String)>(&mut conn)
            .unwrap(),
        (1, "tt1".into())
    );

    // Existing uniqueness constraints remain active after changing only the rating column.
    assert!(
        diesel::insert_into(old::table)
            .values((
                old::media_item_id.eq(1),
                old::provider_id.eq("tmdb"),
                old::external_id.eq("duplicate"),
                old::locale_key.eq("test-1"),
                old::rating.eq(1.0),
            ))
            .execute(&mut conn)
            .is_err()
    );
    // SQLite affinity must not admit fractional scores into the integer column.
    assert!(
        diesel::update(old::table.find(1))
            .set(old::rating.eq(78.5))
            .execute(&mut conn)
            .is_err()
    );

    conn.batch_execute(DOWN).unwrap();
    let restored = old::table
        .order(old::id)
        .select(old::rating)
        .load::<Option<f64>>(&mut conn)
        .unwrap();
    assert_eq!(
        restored,
        vec![
            None,
            Some(0.0),
            Some(10.0),
            Some(7.8),
            Some(8.3),
            Some(10.0),
            None,
            None,
            None
        ]
    );
    conn.batch_execute(UP).unwrap();
    assert_eq!(
        links::table
            .order(links::id)
            .select(links::rating)
            .load::<Option<i16>>(&mut conn)
            .unwrap(),
        expected
    );
    assert_eq!(ids::table.count().get_result::<i64>(&mut conn).unwrap(), 1);
    diesel::delete(links::table.find(1))
        .execute(&mut conn)
        .unwrap();
    assert_eq!(ids::table.count().get_result::<i64>(&mut conn).unwrap(), 0);
}
