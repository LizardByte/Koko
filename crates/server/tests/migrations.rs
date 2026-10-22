//! Validate SQLite migration ordering, idempotence, and complete upgrade/downgrade round trips.

use diesel::{
    Connection,
    SqliteConnection,
};

#[test]
fn sqlite_migrations_preserve_historical_order_and_round_trip() {
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    let revisions = koko::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    let versions = revisions
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    // Later revisions may follow these established migrations.
    assert!(versions.starts_with(&[
        "a54d52c8da5e".to_string(),
        "c18f43d2a9b7".to_string(),
        "d7e92a604b31".to_string(),
        "834267a910bb".to_string(),
    ]));
    assert!(
        koko::db::run_pending_sqlite_migrations(&mut conn)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        koko::db::revert_all_sqlite_migrations(&mut conn)
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        versions.into_iter().rev().collect::<Vec<_>>()
    );
    assert_eq!(
        koko::db::run_pending_sqlite_migrations(&mut conn).unwrap(),
        revisions
    );
}
