//! Removal preserves playable DVR files and rejects active captures without partial deletion.
use super::*;

#[test]
fn removal_keeps_saved_video_and_other_sources_but_clears_configuration_and_timers() {
    let mut conn = crate::live_tv::tests::database();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("saved.ts");
    std::fs::write(&path, b"saved video").unwrap();
    for (start, state, file) in [
        (100_i64, "completed", Some(path.to_str().unwrap())),
        (300, "scheduled", None),
        (500, "scheduled", Some(path.to_str().unwrap())),
    ] {
        diesel::insert_into(r::table)
            .values((
                r::user_id.eq(1),
                r::channel_id.eq(1),
                r::title.eq("Saved program"),
                r::starts_at.eq(start),
                r::ends_at.eq(start + 100),
                r::padding_before.eq(0),
                r::padding_after.eq(0),
                r::state.eq(state),
                r::file_name.eq(file),
                r::metadata_json.eq("{}"),
            ))
            .execute(&mut conn)
            .unwrap();
    }
    diesel::insert_into(rules::table)
        .values((
            rules::user_id.eq(1),
            rules::channel_id.eq(1),
            rules::title.eq("Series"),
            rules::new_only.eq(false),
            rules::padding_before.eq(0),
            rules::padding_after.eq(0),
        ))
        .execute(&mut conn)
        .unwrap();
    super::super::setup::save_groups(
        &mut conn,
        vec![
            super::super::setup::SourceGroup {
                id: "combined".into(),
                name: "Combined".into(),
                source_ids: vec![1, 2],
            },
        ],
    )
    .unwrap();
    for key in [
        "live_tv_metadata:1",
        "live_tv_feed_sizes:1",
        "live_tv_image:1:",
        "live_tv_image:1:espn",
        "live_tv_metadata:2",
        "live_tv_image:11:",
    ] {
        super::super::setup::save(&mut conn, key, "fixture").unwrap();
    }
    let guide = super::super::formats::parse_guide(include_str!("../fixtures/guide.xml")).unwrap();
    storage::save_guide(&mut conn, 1, &guide).unwrap();
    assert!(remove_connection(&mut conn, 1).is_ok());
    assert_eq!(std::fs::read(&path).unwrap(), b"saved video");
    assert_eq!(storage::sources(&mut conn).unwrap().len(), 1);
    assert!(storage::source(&mut conn, 1).is_err());
    assert_eq!(storage::channels(&mut conn, true).unwrap().len(), 1);
    let saved = storage::recordings(&mut conn, 1).unwrap();
    assert_eq!(saved.len(), 2);
    assert_eq!(saved[0].state, "cancelled");
    assert_eq!(saved[1].state, "completed");
    assert_eq!(
        super::super::settings::capture_path(&mut conn, saved[0].id).unwrap(),
        path
    );
    assert!(storage::rules(&mut conn, 1).unwrap().is_empty());
    assert!(super::super::setup::groups(&mut conn).unwrap().is_empty());
    assert_eq!(p::table.count().get_result::<i64>(&mut conn).unwrap(), 0);
    assert_eq!(
        stations::table
            .count()
            .get_result::<i64>(&mut conn)
            .unwrap(),
        0
    );
    assert_eq!(
        a::table
            .select(a::key)
            .order(a::key)
            .load::<String>(&mut conn)
            .unwrap(),
        vec![
            "live_tv_image:11:",
            "live_tv_metadata:2",
            "live_tv_source_groups"
        ]
    );
    assert_eq!(
        s::table
            .find(1)
            .select((s::kind, s::config_json, s::enabled))
            .first::<(String, String, bool)>(&mut conn)
            .unwrap(),
        (REMOVED.into(), "{}".into(), false)
    );
    assert!(
        c::table
            .find(1)
            .select(c::stream_url)
            .first::<String>(&mut conn)
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        remove_connection(&mut conn, 1),
        Err(RemoveError::Database(diesel::result::Error::NotFound))
    ));
    assert!(remove_connection(&mut conn, 2).is_ok());
    assert_eq!(
        s::table
            .filter(s::id.eq(2))
            .count()
            .get_result::<i64>(&mut conn)
            .unwrap(),
        0
    );
}

#[test]
fn active_source_removal_is_rejected_without_changing_its_settings() {
    let mut conn = crate::live_tv::tests::database();
    let id = 93001;
    diesel::insert_into(s::table)
        .values((
            s::id.eq(id),
            s::name.eq("Busy"),
            s::kind.eq("m3u"),
            s::config_json.eq("{}"),
            s::capacity.eq(1),
        ))
        .execute(&mut conn)
        .unwrap();
    let source = storage::source(&mut conn, id).unwrap();
    let lease = stream::lease(&source).unwrap();
    assert!(matches!(
        remove_connection(&mut conn, id),
        Err(RemoveError::Busy)
    ));
    assert!(storage::source(&mut conn, id).unwrap().enabled);
    drop(lease);
    assert!(remove_connection(&mut conn, id).is_ok());
}
