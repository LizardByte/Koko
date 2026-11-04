//! Source groups and import statistics preserve the underlying channels and tuner capacities.
use super::*;

#[test]
fn source_groups_are_durable_and_invalid_edits_preserve_the_saved_lineup() {
    let mut conn = crate::live_tv::tests::database();
    let before = crate::live_tv::storage::sources(&mut conn)
        .unwrap()
        .into_iter()
        .map(|s| (s.id, s.capacity))
        .collect::<Vec<_>>();
    save_groups(
        &mut conn,
        vec![SourceGroup {
            id: "fast".into(),
            name: "FastChannels".into(),
            source_ids: vec![2, 1, 1],
        }],
    )
    .unwrap();
    let saved = groups(&mut conn).unwrap();
    assert_eq!(saved[0].source_ids, vec![1, 2]);
    for (name, ids) in [
        ("", vec![1, 2]),
        ("Invalid", vec![1]),
        ("Missing", vec![1, 999]),
    ] {
        assert!(
            save_groups(
                &mut conn,
                vec![SourceGroup {
                    id: "fast".into(),
                    name: name.into(),
                    source_ids: ids
                }]
            )
            .is_err()
        );
        assert_eq!(groups(&mut conn).unwrap()[0].name, "FastChannels");
    }
    let after = crate::live_tv::storage::sources(&mut conn)
        .unwrap()
        .into_iter()
        .map(|s| (s.id, s.capacity))
        .collect::<Vec<_>>();
    assert_eq!(before, after);
    save_groups(&mut conn, Vec::new()).unwrap();
    assert!(groups(&mut conn).unwrap().is_empty());
    assert_eq!(
        crate::live_tv::storage::channels(&mut conn, true)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn source_cards_count_available_channels_and_keep_last_import_sizes() {
    let mut conn = crate::live_tv::tests::database();
    diesel::update(s::table.find(1)).set(s::config_json.eq(serde_json::json!({
        "iptv_mode":"fastchannels", "url":"http://private-server.example", "pin":"private-pin"
    }).to_string())).execute(&mut conn).unwrap();
    save_feed_sizes(
        &mut conn,
        1,
        &FeedSizes {
            playlist_bytes: Some(2_000_000),
            guide_bytes: Some(150_000_000),
        },
    )
    .unwrap();
    let rows = sources(&mut conn).unwrap();
    let source = rows.iter().find(|s| s.source.id == 1).unwrap();
    assert_eq!(source.channel_count, 1);
    assert_eq!(source.sizes.guide_bytes, Some(150_000_000));
    assert_eq!(source.iptv_mode.as_deref(), Some("fastchannels"));
    let guide = super::super::storage::guide_sources(&mut conn).unwrap();
    assert_eq!(guide[0].iptv_mode.as_deref(), Some("fastchannels"));
    assert!(guide[1].iptv_mode.is_none());
    let public_guide = serde_json::to_string(&guide).unwrap();
    assert!(!public_guide.contains("private-server") && !public_guide.contains("private-pin"));
    diesel::update(c::table.filter(c::source_id.eq(1)))
        .set(c::available.eq(false))
        .execute(&mut conn)
        .unwrap();
    assert_eq!(
        sources(&mut conn)
            .unwrap()
            .iter()
            .find(|s| s.source.id == 1)
            .unwrap()
            .channel_count,
        0
    );
    let json = serde_json::to_string(&rows).unwrap();
    assert!(!json.contains("secret-reference"));
    assert!(!json.contains("config_json"));
    assert!(!json.contains("private-server") && !json.contains("private-pin"));
}
