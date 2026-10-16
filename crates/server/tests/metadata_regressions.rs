use diesel::connection::SimpleConnection;
use diesel::{
    Connection,
    SqliteConnection,
};
use koko::config::MetadataProviderId;
use koko::metadata::{
    StoredMetadataSnapshot,
    get_item_metadata_summaries,
    set_item_metadata_refresh_state_for_locale,
    upsert_item_metadata_snapshot,
};

fn connection() -> SqliteConnection {
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    koko::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    conn.batch_execute(
        "INSERT INTO media_libraries (id,name,path,kind) VALUES (1,'Review','review','movies'); \
         INSERT INTO media_items (id,library_id,identity_key,item_type,display_title) VALUES \
         (1,1,'review','movie','Movie');",
    )
    .unwrap();
    conn
}

fn snapshot(locale: &str) -> StoredMetadataSnapshot {
    StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tmdb,
        external_id: "603".into(),
        media_type: Some("movie".into()),
        title: Some("Movie".into()),
        overview: None,
        artwork_url: None,
        backdrop_url: None,
        release_year: Some(1999),
        locale_key: locale.into(),
        provider_locale_key: Some(locale.into()),
        provider_payload_json: None,
    }
}

#[test]
fn metadata_refresh_state_changes_only_the_requested_translation() {
    let mut conn = connection();
    for locale in ["en-US", "es-ES"] {
        upsert_item_metadata_snapshot(&mut conn, 1, &snapshot(locale)).unwrap();
    }
    set_item_metadata_refresh_state_for_locale(
        &mut conn,
        1,
        MetadataProviderId::Tmdb,
        "603",
        Some("movie"),
        "es-ES",
        "error",
        Some("failed"),
    )
    .unwrap();
    let rows = get_item_metadata_summaries(&mut conn, 1).unwrap();
    assert_eq!(
        rows.iter()
            .find(|row| row.locale_key == "en-US")
            .unwrap()
            .refresh_state,
        "fresh"
    );
    assert_eq!(
        rows.iter()
            .find(|row| row.locale_key == "es-ES")
            .unwrap()
            .refresh_state,
        "error"
    );
    set_item_metadata_refresh_state_for_locale(
        &mut conn,
        1,
        MetadataProviderId::Tmdb,
        "603",
        Some("movie"),
        "fr-FR",
        "pending",
        None,
    )
    .unwrap();
    let rows = get_item_metadata_summaries(&mut conn, 1).unwrap();
    assert_eq!(
        rows.iter()
            .find(|row| row.locale_key == "fr-FR")
            .unwrap()
            .refresh_state,
        "pending"
    );
}
