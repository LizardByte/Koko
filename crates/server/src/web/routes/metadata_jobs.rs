//! Keep obsolete metadata work from restoring an unlinked or replaced match.

use std::collections::HashMap;
use std::sync::Mutex;

use diesel::{
    Connection,
    QueryResult,
    SqliteConnection,
};
use once_cell::sync::Lazy;

use crate::db::DbConn;

static REVISIONS: Lazy<Mutex<HashMap<i32, u64>>> = Lazy::new(|| Mutex::new(HashMap::new()));
/// Serialize reset requests through activity detachment and replacement registration.
pub(super) static RESET_REQUEST: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
tokio::task_local! { static ACTIVE_REVISION: MetadataRevision; }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct MetadataRevision {
    item_id: i32,
    version: u64,
}

impl MetadataRevision {
    pub(super) fn current(item_id: i32) -> Self {
        Self {
            item_id,
            version: *REVISIONS.lock().unwrap().get(&item_id).unwrap_or(&0),
        }
    }

    pub(super) fn inherited(item_id: i32) -> Self {
        ACTIVE_REVISION
            .try_with(|revision| *revision)
            .unwrap_or_else(|_| Self::current(item_id))
    }

    pub(super) fn is_current(self) -> bool {
        Self::current(self.item_id) == self
    }

    pub(super) async fn scope<T>(
        self,
        future: impl std::future::Future<Output = T>,
    ) -> T {
        ACTIVE_REVISION.scope(self, future).await
    }
}

/// Capture identities and revisions under the same lock used by resets. Work
/// loaded before a rematch must keep its old revision even while waiting in a queue.
pub(super) fn load<T>(
    conn: &mut SqliteConnection,
    query: impl FnOnce(&mut SqliteConnection) -> QueryResult<Vec<T>>,
    item_id: impl Fn(&T) -> i32,
) -> QueryResult<Vec<(T, MetadataRevision)>> {
    let versions = REVISIONS.lock().unwrap();
    Ok(query(conn)?
        .into_iter()
        .map(|value| {
            let item_id = item_id(&value);
            let revision = MetadataRevision {
                item_id,
                version: *versions.get(&item_id).unwrap_or(&0),
            };
            (value, revision)
        })
        .collect())
}

/// The guard lives inside the blocking DB closure, so cancellation and network
/// awaits cannot allow a stale write to run after the synchronous reset.
pub(super) async fn write<T: Send + 'static>(
    db: &DbConn,
    action: impl FnOnce(&mut SqliteConnection) -> QueryResult<T> + Send + 'static,
) -> QueryResult<T> {
    let expected = ACTIVE_REVISION.try_with(|revision| *revision).ok();
    db.run(move |conn| {
        let versions = REVISIONS.lock().unwrap();
        if expected.is_some_and(|revision| {
            *versions.get(&revision.item_id).unwrap_or(&0) != revision.version
        }) {
            return Err(diesel::result::Error::RollbackTransaction);
        }
        action(conn)
    })
    .await
}

pub(super) fn reset<T>(
    conn: &mut SqliteConnection,
    item_id: i32,
    unlink: bool,
    prepare: impl FnOnce(&mut SqliteConnection) -> QueryResult<T>,
) -> QueryResult<(Vec<i32>, MetadataRevision, T)> {
    reset_scope(conn, item_id, unlink, None, prepare)
}

/// Replace one book metadata source without discarding independently selected work/recording
/// sources.
pub(super) fn replace_book_source<T>(
    conn: &mut SqliteConnection,
    item_id: i32,
    provider: crate::config::MetadataProviderId,
    prepare: impl FnOnce(&mut SqliteConnection) -> QueryResult<T>,
) -> QueryResult<(Vec<i32>, MetadataRevision, T)> {
    reset_scope(conn, item_id, false, Some(provider), prepare)
}

fn reset_scope<T>(
    conn: &mut SqliteConnection,
    item_id: i32,
    unlink: bool,
    source: Option<crate::config::MetadataProviderId>,
    prepare: impl FnOnce(&mut SqliteConnection) -> QueryResult<T>,
) -> QueryResult<(Vec<i32>, MetadataRevision, T)> {
    crate::db::configure_sqlite_connection(conn)?;
    let mut versions = REVISIONS.lock().unwrap();
    let (ids, result) = conn.transaction(|conn| {
        let ids = if let Some(source) = &source {
            use crate::db::schema::item_metadata_links as links;
            use diesel::prelude::*;
            diesel::delete(
                links::table
                    .filter(links::media_item_id.eq(item_id))
                    .filter(links::provider_id.eq(source.as_storage_value())),
            )
            .execute(conn)?;
            if *source == crate::config::MetadataProviderId::Libex {
                diesel::delete(
                    links::table
                        .filter(links::media_item_id.eq(item_id))
                        .filter(links::provider_id.eq("audiosilo"))
                        .filter(links::external_id.like("audiobook:libex:%")),
                )
                .execute(conn)?;
            }
            vec![item_id]
        } else {
            crate::metadata::clear_item_metadata_tree_in_transaction(conn, item_id, unlink)?
        };
        Ok::<_, diesel::result::Error>((ids, prepare(conn)?))
    })?;
    for id in &ids {
        *versions.entry(*id).or_default() += 1;
    }
    let revision = MetadataRevision {
        item_id,
        version: versions[&item_id],
    };
    Ok((ids, revision, result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::connection::SimpleConnection;

    #[test]
    fn adding_a_book_source_preserves_other_sources_and_rejects_obsolete_writes() {
        use crate::config::MetadataProviderId;
        use crate::db::schema::{
            item_metadata_links as links,
            media_items as items,
            media_libraries as libraries,
        };
        use diesel::prelude::*;
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        crate::db::run_pending_sqlite_migrations(&mut conn).unwrap();
        const ID: i32 = 191_000_003;
        diesel::insert_into(libraries::table)
            .values((
                libraries::id.eq(ID),
                libraries::name.eq("Books"),
                libraries::path.eq("books"),
                libraries::kind.eq("books"),
            ))
            .execute(&mut conn)
            .unwrap();
        diesel::insert_into(items::table)
            .values((
                items::id.eq(ID),
                items::library_id.eq(ID),
                items::identity_key.eq("audiobook"),
                items::item_type.eq("audiobook"),
                items::display_title.eq("Book"),
            ))
            .execute(&mut conn)
            .unwrap();
        for provider in ["libex", "open_library"] {
            diesel::insert_into(links::table)
                .values((
                    links::media_item_id.eq(ID),
                    links::provider_id.eq(provider),
                    links::external_id.eq("old"),
                ))
                .execute(&mut conn)
                .unwrap();
        }
        let old = MetadataRevision::current(ID);
        let (_, current, ()) =
            replace_book_source(&mut conn, ID, MetadataProviderId::OpenLibrary, |_| Ok(()))
                .unwrap();
        let remaining = links::table
            .filter(links::media_item_id.eq(ID))
            .select(links::provider_id)
            .load::<String>(&mut conn)
            .unwrap();
        assert_eq!(remaining, vec!["libex"]);
        assert!(!old.is_current());
        assert!(current.is_current());
        assert!(
            replace_book_source(&mut conn, ID, MetadataProviderId::Libex, |_| Err::<(), _>(
                diesel::result::Error::RollbackTransaction
            ))
            .is_err()
        );
        assert_eq!(
            links::table.count().get_result::<i64>(&mut conn).unwrap(),
            1
        );
        assert!(current.is_current());
    }

    #[tokio::test]
    async fn reset_is_atomic_and_rejects_old_root_and_descendant_writes() {
        use crate::config::MetadataProviderId;
        use crate::metadata::{
            MetadataRefreshStateUpdate,
            get_item_metadata_summaries,
            set_item_metadata_refresh_state_for_locale,
        };
        const ROOT: i32 = 191_000_001;
        const CHILD: i32 = 191_000_002;
        let rocket = rocket::custom(rocket::Config::figment().merge((
            "databases.sqlite_db",
            serde_json::json!({"url": ":memory:", "pool_size": 1}),
        )))
        .attach(DbConn::fairing())
        .ignite()
        .await
        .unwrap();
        let db = DbConn::get_one(&rocket).await.unwrap();
        db.run(|conn| {
            crate::db::run_pending_sqlite_migrations(conn).unwrap();
            crate::db::configure_sqlite_connection(conn).unwrap();
            conn.batch_execute(
                "INSERT INTO media_libraries (id,name,path,kind) VALUES \
                 (191000001,'Shows','shows','shows');
                INSERT INTO media_items \
                 (id,library_id,parent_id,identity_key,item_type,display_title) VALUES
                (191000001,191000001,NULL,'root','show','Show'), \
                 (191000002,191000001,191000001,'child','season','Season');
                INSERT INTO item_metadata_links \
                 (media_item_id,provider_id,external_id,title,overview,last_refreshed_at)
                VALUES (191000001,'tmdb','old','Old title','Old overview',1), \
                 (191000002,'tmdb','old-child','Old season','Old overview',1);",
            )
            .unwrap();
        })
        .await;
        let old_root = MetadataRevision::current(ROOT);
        let old_child = MetadataRevision::current(CHILD);
        // A failed replacement leaves the original metadata and revision intact.
        assert!(
            db.run(|conn| reset::<()>(conn, ROOT, false, |_| Err(
                diesel::result::Error::RollbackTransaction
            )))
            .await
            .is_err()
        );
        assert!(old_root.is_current());
        assert_eq!(
            db.run(|conn| get_item_metadata_summaries(conn, ROOT))
                .await
                .unwrap()[0]
                .title
                .as_deref(),
            Some("Old title")
        );

        let (ids, new_revision, pending) = db
            .run(|conn| {
                reset(conn, ROOT, false, |conn| {
                    crate::metadata::set_item_metadata_refresh_state_in_transaction(
                        conn,
                        &MetadataRefreshStateUpdate {
                            item_id: ROOT,
                            provider_id: MetadataProviderId::Tvdb,
                            external_id: "new",
                            media_type: Some("series"),
                            locale_key: "en-US",
                            refresh_state: "pending",
                            refresh_error: None,
                        },
                    )
                })
            })
            .await
            .unwrap();
        assert_eq!(ids, vec![ROOT, CHILD]);
        assert!(pending.title.is_none());
        assert!(pending.overview.is_none());
        assert!(pending.artwork_url.is_none());
        assert_eq!(pending.external_id, "new");
        assert!(!old_root.is_current());
        assert!(!old_child.is_current());
        for revision in [old_root, old_child] {
            let result = revision
                .scope(write(&db, |conn| {
                    set_item_metadata_refresh_state_for_locale(
                        conn,
                        MetadataRefreshStateUpdate {
                            item_id: CHILD,
                            provider_id: MetadataProviderId::Tmdb,
                            external_id: "old-child",
                            media_type: Some("season"),
                            locale_key: "en-US",
                            refresh_state: "error",
                            refresh_error: Some("obsolete failure"),
                        },
                    )
                }))
                .await;
            assert!(matches!(
                result,
                Err(diesel::result::Error::RollbackTransaction)
            ));
        }
        new_revision
            .scope(write(&db, |conn| {
                set_item_metadata_refresh_state_for_locale(
                    conn,
                    MetadataRefreshStateUpdate {
                        item_id: CHILD,
                        provider_id: MetadataProviderId::Tvdb,
                        external_id: "new-child",
                        media_type: Some("season"),
                        locale_key: "en-US",
                        refresh_state: "pending",
                        refresh_error: None,
                    },
                )
            }))
            .await
            .unwrap();
        db.run(|conn| reset(conn, ROOT, true, |_| Ok(())))
            .await
            .unwrap();
        assert!(new_revision.scope(write(&db, |_| Ok(()))).await.is_err());
        assert!(
            db.run(|conn| get_item_metadata_summaries(conn, ROOT))
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            db.run(|conn| get_item_metadata_summaries(conn, CHILD))
                .await
                .unwrap()
                .is_empty()
        );
    }
}
