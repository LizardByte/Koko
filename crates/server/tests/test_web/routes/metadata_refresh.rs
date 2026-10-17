use super::*;
use diesel::connection::SimpleConnection;

const SHOW: i32 = 195_000_001;
const SEASON: i32 = 195_000_002;
const EPISODE: i32 = 195_000_003;

fn target(
    item_id: i32,
    item_type: &str,
) -> MetadataRefreshTarget {
    MetadataRefreshTarget {
        revision: MetadataRevision::current(item_id),
        locale_key: Some("en-US".into()),
        item_id,
        library_id: SHOW,
        provider_id: MetadataProviderId::Tmdb,
        item_type: item_type.into(),
        display_title: item_type.into(),
        relative_path: item_type.into(),
        external_id: "1412".into(),
        media_type: "tv".into(),
        fetch_kind: MetadataRefreshFetchKind::Direct,
    }
}

#[tokio::test]
async fn automatic_descendants_publish_pending_work_and_count_child_failures() {
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
        conn.batch_execute(
            "INSERT INTO media_libraries (id,name,path,kind) VALUES
            (195000001,'Shows','shows','shows');
            INSERT INTO media_items (id,library_id,parent_id,identity_key,item_type,display_title)
            VALUES (195000001,195000001,NULL,'show','show','Show'),
                (195000002,195000001,195000001,'season','season','Season 1'),
                (195000003,195000001,195000002,'episode','episode','Episode 1');",
        )
        .unwrap();
    })
    .await;
    let mut settings = Settings::default();
    // Fail every provider call immediately, without network traffic.
    settings.metadata.providers.clear();

    // Cover a match extending its library activity and background backfill
    // creating its own activity. Both must publish child work before fetching.
    for has_parent in [true, false] {
        let parent = if has_parent {
            Some(
                register_metadata_refresh_activity(
                    "library",
                    "automatic_match",
                    "Match show".into(),
                    Some(SHOW),
                    Some(SHOW),
                    vec![target(SHOW, "show")],
                )
                .await
                .unwrap()
                .0,
            )
        } else {
            None
        };
        let execution_block = ACTIVE_METADATA_REFRESH_EXECUTIONS.write().await;
        let observe = async {
            let activity_id = loop {
                let pending = db
                    .run(|conn| list_media_items(conn, Some(SHOW)))
                    .await
                    .unwrap();
                if pending
                    .iter()
                    .filter(|item| item.metadata_refresh_state.as_deref() == Some("pending"))
                    .count()
                    == 2
                {
                    let activity = current_system_activities()
                        .await
                        .into_iter()
                        .find(|activity| activity.library_id == Some(SHOW))
                        .unwrap();
                    assert!(activity.item_ids.contains(&SEASON));
                    assert!(activity.item_ids.contains(&EPISODE));
                    assert_eq!(activity.total_items, if has_parent { 3 } else { 2 });
                    assert_eq!(activity.completed_items, 0);
                    break activity.id;
                }
                tokio::task::yield_now().await;
            };
            drop(execution_block);
            activity_id
        };
        let (result, activity_id) =
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                tokio::join!(
                    execute_metadata_refresh_targets(
                        &db,
                        vec![
                            target(SEASON, "season"),
                            target(EPISODE, "episode")
                        ],
                        &settings,
                        parent.as_deref()
                    ),
                    observe
                )
            })
            .await
            .unwrap();
        result.unwrap();
        if let Some(parent) = parent {
            let activity = current_system_activities()
                .await
                .into_iter()
                .find(|entry| entry.id == parent)
                .unwrap();
            assert_eq!(activity.completed_items, 2);
            assert_eq!(activity.failed_items, 2);
            record_metadata_refresh_activity_progress(&parent, SHOW, false).await;
            let activity = current_system_activities()
                .await
                .into_iter()
                .find(|entry| entry.id == parent)
                .unwrap();
            assert_eq!(activity.completed_items, 3);
            assert_eq!(activity.failed_items, 2);
            complete_metadata_refresh_activity(&parent).await;
        }
        assert!(
            !current_system_activities()
                .await
                .iter()
                .any(|entry| entry.id == activity_id)
        );
        for id in [SEASON, EPISODE] {
            let rows = db
                .run(move |conn| get_item_metadata_summaries(conn, id))
                .await
                .unwrap();
            assert_eq!(rows[0].refresh_state, "error");
            assert!(!ACTIVE_METADATA_REFRESH_ITEMS.read().await.contains_key(&id));
        }
    }
}

#[tokio::test]
async fn cancelled_parent_cannot_claim_descendant_activity_ownership() {
    const CANCELLED: i32 = 196_000_001;
    let (activity_id, _) = register_metadata_refresh_activity(
        "item",
        "automatic_match",
        "Cancelled".into(),
        None,
        Some(CANCELLED),
        vec![target(CANCELLED, "show")],
    )
    .await
    .unwrap();
    cancel_metadata_refresh_activity(&activity_id).await;
    assert!(
        extend_metadata_refresh_activity(&activity_id, vec![target(CANCELLED, "season")])
            .await
            .is_empty()
    );
    assert!(
        !ACTIVE_METADATA_REFRESH_ITEMS
            .read()
            .await
            .contains_key(&CANCELLED)
    );
}
