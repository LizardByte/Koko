//! Guide parsing, durable DVR behavior, credentials, and tuner-capacity regressions.

use super::*;
use crate::db::schema::{
    live_tv_channels as c,
    live_tv_recordings as r,
    live_tv_series_rules as rules,
    live_tv_sources as s,
    users,
};
use diesel::prelude::*;

const XML: &str = include_str!("fixtures/guide.xml");

#[test]
fn legacy_guide_settings_keep_the_selected_lineup_after_the_zap2it_rename() {
    let config: SourceConfig = serde_json::from_value(serde_json::json!({
        "guide_kind": "gracenote",
        "gracenote": {
            "country": "USA", "postal_code": "14850",
            "lineup_id": "USA-lineupId-DEFAULT", "headend_id": "lineupId"
        }
    }))
    .unwrap();
    assert_eq!(config.zap2it.postal_code, "14850");
    let serialized = serde_json::to_value(config).unwrap();
    assert!(serialized.get("gracenote").is_none());
    assert_eq!(serialized["zap2it"]["lineup_id"], "USA-lineupId-DEFAULT");
}

#[test]
fn manual_server_locations_keep_reverse_proxy_paths() {
    assert_eq!(
        formats::endpoint("https://example.com/tv", "service")
            .unwrap()
            .as_str(),
        "https://example.com/tv/service"
    );
    assert_eq!(
        formats::endpoint("https://example.com/tv/", "playlist/channels.m3u")
            .unwrap()
            .as_str(),
        "https://example.com/tv/playlist/channels.m3u"
    );
}

#[rocket::async_test]
#[ignore = "Requires KOKO_TEST_TSTABLES pointing to an installed TSDuck executable"]
async fn broadcast_transport_stream_round_trip() {
    let regions = tsduck::regions().await.unwrap();
    assert!(
        regions
            .iter()
            .any(|region| region.eq_ignore_ascii_case("north-america"))
    );
    let tables = std::path::PathBuf::from(std::env::var("KOKO_TEST_TSTABLES").unwrap());
    let packetizer =
        tables.with_file_name(if cfg!(windows) { "tspacketize.exe" } else { "tspacketize" });
    let root = std::env::temp_dir().join(format!("koko-broadcast-{:032x}", rand::random::<u128>()));
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("tables.xml");
    let output = root.join("broadcast.ts");
    std::fs::write(&input, include_str!("fixtures/broadcast.xml")).unwrap();
    let status = std::process::Command::new(packetizer)
        .args(["--pid", "0x1ffb", "--output"])
        .arg(&output)
        .arg(&input)
        .status()
        .unwrap();
    assert!(status.success());
    let config = SourceConfig::default();
    let file = tokio::fs::File::open(&output).await.unwrap();
    let guide = broadcast::decode(&config, file).await.unwrap();
    assert_eq!(guide.programs.len(), 2);
    assert!(
        guide
            .programs
            .iter()
            .any(|p| p.title == "ATSC show" && p.ends_at - p.starts_at == 3600)
    );
    assert!(guide.stations.iter().any(|s| s.number == "7.1"));
    // Exercise the exact native tuner bridge using TSDuck's file-backed tuner emulator.
    let emulator = root.join("tuner.xml");
    std::fs::write(
        &emulator,
        concat!(
            "<tsduck><tuner type=\"ATSC\"/><defaults directory=\".\"/>",
            "<channel frequency=\"473000000\" ",
            "delivery=\"ATSC\" file=\"broadcast.ts\"/></tsduck>"
        ),
    )
    .unwrap();
    let config = SourceConfig {
        native_device: emulator.to_string_lossy().into(),
        ..Default::default()
    };
    let channel = Channel::new(
        "473000000:1".into(),
        "Emulated service".into(),
        "--delivery-system ATSC --frequency 473000000".into(),
    );
    use tokio::io::AsyncReadExt;
    let mut tuner = tsduck::tune(&config, &channel).await.unwrap();
    let mut packet = vec![0; 188 * 3];
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tuner.read_exact(&mut packet),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(packet.as_chunks::<188>().0.iter().all(|p| p[0] == 0x47));
    drop(tuner);
    // Native capture must release its file after the consumer disconnects.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match tokio::fs::remove_file(&output).await {
            Ok(()) => break,
            Err(e)
                if (e.kind() == std::io::ErrorKind::PermissionDenied
                    || (cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33))))
                    && tokio::time::Instant::now() < deadline =>
            {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            Err(e) => panic!("Native capture did not release its file: {e}"),
        }
    }
    std::fs::remove_file(emulator).unwrap();
    std::fs::remove_file(input).unwrap();
    std::fs::remove_dir(root).unwrap();
}

pub(super) fn database() -> SqliteConnection {
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    crate::db::configure_sqlite_connection(&mut conn).unwrap();
    crate::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    for id in [1, 2] {
        diesel::insert_into(users::table)
            .values((
                users::id.eq(id),
                users::username.eq(format!("viewer{id}")),
                users::password.eq("fixture-hash"),
                users::admin.eq(id == 1),
            ))
            .execute(&mut conn)
            .unwrap();
    }
    for id in [1, 2] {
        diesel::insert_into(s::table)
            .values((
                s::id.eq(id),
                s::name.eq(format!("Source {id}")),
                s::kind.eq("m3u"),
                s::config_json.eq("secret-reference"),
                s::capacity.eq(1),
            ))
            .execute(&mut conn)
            .unwrap();
        let mut channel = Channel::new(
            "stable-key".into(),
            "Network One".into(),
            "http://example.com/stream.ts?credential=hidden".into(),
        );
        channel.number = "7.1".into();
        storage::save_channels(&mut conn, id, &[channel]).unwrap();
    }
    conn
}

#[test]
fn guide_retention_prunes_disabled_and_offline_sources_without_touching_dvr_metadata() {
    use crate::db::schema::live_tv_programs as p;
    let mut conn = database();
    let now = now();
    diesel::update(s::table.find(2))
        .set(s::enabled.eq(false))
        .execute(&mut conn)
        .unwrap();
    for (source, start, end) in [
        (1, now - 3 * 86400, now - 2 * 86400),
        (2, now - 3 * 86400, now - 2 * 86400),
        (1, now + 22 * 86400, now + 23 * 86400),
        (1, now - 100, now + 100),
    ] {
        diesel::insert_into(p::table)
            .values((
                p::source_id.eq(source),
                p::guide_key.eq("station"),
                p::starts_at.eq(start),
                p::ends_at.eq(end),
                p::title.eq("Retention fixture"),
                p::metadata_json.eq("{}"),
            ))
            .execute(&mut conn)
            .unwrap();
    }
    let snapshot = serde_json::json!({
        "description":"Recorded episode metadata",
        "episode_id":"retained-episode"
    })
    .to_string();
    diesel::insert_into(r::table)
        .values((
            r::user_id.eq(1),
            r::channel_id.eq(1),
            r::title.eq("Saved DVR"),
            r::starts_at.eq(now - 3 * 86400),
            r::ends_at.eq(now - 2 * 86400),
            r::padding_before.eq(0),
            r::padding_after.eq(0),
            r::state.eq("completed"),
            r::metadata_json.eq(&snapshot),
        ))
        .execute(&mut conn)
        .unwrap();
    assert_eq!(retention::prune(&mut conn, now).unwrap(), 3);
    assert_eq!(p::table.count().get_result::<i64>(&mut conn).unwrap(), 1);
    assert_eq!(
        r::table
            .select(r::metadata_json)
            .first::<String>(&mut conn)
            .unwrap(),
        snapshot
    );
}

#[test]
fn guide_ingestion_bounds_history_and_future_without_accumulating_refreshes() {
    let mut conn = database();
    let now = now();
    let rows = vec![
        program(now - 3 * 86400, now - 2 * 86400, "past"),
        program(now, now + 1800, "current"),
        program(now + 22 * 86400, now + 23 * 86400, "future"),
    ];
    storage::save_programs(&mut conn, 1, &rows).unwrap();
    storage::save_programs(&mut conn, 1, &rows).unwrap();
    let kept = storage::guide(&mut conn, now - 4 * 86400, now + 24 * 86400).unwrap();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].metadata.episode_id.as_deref(), Some("current"));
}

#[test]
fn channel_favorites_and_recent_history_are_private_deduplicated_and_persistent() {
    let mut conn = database();
    let channels = storage::channels(&mut conn, false).unwrap();
    let first = channels[0].id;
    let second = channels[1].id;
    preferences::favorite(&mut conn, 1, first, true).unwrap();
    preferences::favorite(&mut conn, 1, first, true).unwrap();
    preferences::played(&mut conn, 1, first).unwrap();
    preferences::played(&mut conn, 1, second).unwrap();
    preferences::played(&mut conn, 1, first).unwrap();
    let saved = preferences::load(&mut conn, 1).unwrap();
    assert_eq!(saved.favorites, vec![first]);
    assert_eq!(saved.recent, vec![first, second]);
    let other = preferences::load(&mut conn, 2).unwrap();
    assert!(other.favorites.is_empty() && other.recent.is_empty());
    preferences::favorite(&mut conn, 1, first, false).unwrap();
    assert!(
        preferences::load(&mut conn, 1)
            .unwrap()
            .favorites
            .is_empty()
    );
}

#[test]
fn xmltv_live_premiere_and_new_flags_remain_distinct() {
    let xml = concat!(
        "<tv><programme channel=\"one\" start=\"20261030120000 +0000\" stop=\"20261030130000 \
         +0000\">",
        "<title>News</title><live/><new/><premiere/></programme></tv>"
    );
    let guide = formats::parse_guide(xml).unwrap();
    let flags = &guide.programs[0].metadata;
    assert_eq!(flags.is_live, Some(true));
    assert_eq!(flags.is_new, Some(true));
    assert_eq!(flags.is_premiere, Some(true));
}
fn timer(
    channel: i32,
    start: i64,
    end: i64,
) -> ScheduleRequest {
    ScheduleRequest {
        channel_id: channel,
        starts_at: start,
        ends_at: end,
        title: "Evening News".into(),
        series: false,
        new_only: false,
        padding_before: 0,
        padding_after: 0,
        private: true,
    }
}

#[test]
fn recordings_are_shared_by_default_and_private_only_when_requested() {
    let mut conn = database();
    let mut shared = timer(1, now() + 100, now() + 1000);
    shared.private = false;
    schedule(&mut conn, 1, shared).unwrap();
    schedule(&mut conn, 1, timer(1, now() + 2000, now() + 3000)).unwrap();
    assert_eq!(storage::recordings(&mut conn, 1).unwrap().len(), 2);
    let visible = storage::recordings(&mut conn, 2).unwrap();
    assert_eq!(visible.len(), 1);
    assert!(visible[0].shared);
    assert_eq!(visible[0].user_id, 1);
    assert!(
        storage::claim_deletion(&mut conn, 2, visible[0].id)
            .unwrap()
            .is_none()
    );
    let request: ScheduleRequest = serde_json::from_value(serde_json::json!({
        "channel_id": 1,
        "starts_at": now()+10,
        "ends_at": now()+20,
        "title": "Default sharing"
    }))
    .unwrap();
    assert!(!request.private);
}

#[test]
fn cancelled_timers_require_an_explicit_retry_and_keep_the_new_privacy_choice() {
    let mut conn = database();
    let start = now() + 100;
    schedule(&mut conn, 1, timer(1, start, start + 1000)).unwrap();
    diesel::update(r::table.find(1))
        .set(r::state.eq("cancelled"))
        .execute(&mut conn)
        .unwrap();
    let mut retry = timer(1, start, start + 1000);
    retry.private = false;
    schedule(&mut conn, 1, retry).unwrap();
    let saved = storage::recordings(&mut conn, 2).unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].state, "scheduled");
    assert!(saved[0].shared);
}

#[test]
fn denied_accounts_cannot_schedule_and_lose_future_reservations() {
    let mut conn = database();
    schedule(&mut conn, 2, timer(1, now() + 100, now() + 1000)).unwrap();
    settings::set_permission(&mut conn, 2, false).unwrap();
    assert!(!settings::can_record(&mut conn, 2).unwrap());
    assert!(schedule(&mut conn, 2, timer(1, now() + 2000, now() + 3000)).is_err());
    assert_eq!(
        storage::recordings(&mut conn, 2).unwrap()[0].state,
        "cancelled"
    );
    assert!(settings::can_record(&mut conn, 1).unwrap());
}

#[test]
fn dvr_folder_changes_apply_to_new_captures_and_preserve_existing_paths() {
    let mut conn = database();
    schedule(&mut conn, 1, timer(1, now() + 100, now() + 1000)).unwrap();
    schedule(&mut conn, 1, timer(1, now() + 2000, now() + 3000)).unwrap();
    let first = std::env::temp_dir().join("koko-first-recording.ts");
    diesel::update(r::table.find(1))
        .set(r::file_name.eq(first.to_string_lossy().as_ref()))
        .execute(&mut conn)
        .unwrap();
    let folder = std::env::temp_dir().join("koko-new-dvr-folder");
    settings::save(
        &mut conn,
        &settings::DvrSettings {
            recording_dir: folder.to_string_lossy().into(),
        },
    )
    .unwrap();
    assert_eq!(settings::capture_path(&mut conn, 1).unwrap(), first);
    assert_eq!(
        settings::capture_path(&mut conn, 2).unwrap(),
        folder.join("recording-2.ts")
    );
}

#[test]
fn gracenote_maps_station_identity_metadata_and_six_hour_requests() {
    let selection = gracenote::Selection {
        country: "CAN".into(),
        postal_code: "K1A 0B1".into(),
        lineup_id: "CAN-TEST-X".into(),
        headend_id: "TEST".into(),
        days: 2,
        ..Default::default()
    };
    let url = gracenote::page_url(&selection, 1793372400).unwrap();
    let query = url
        .query_pairs()
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(query["timespan"], "6");
    assert_eq!(query["postalCode"], "K1A0B1");
    assert_eq!(query["lineupId"], "CAN-TEST-X");
    let page = serde_json::from_str(include_str!("fixtures/gracenote.json")).unwrap();
    let guide = gracenote::parse(&page).unwrap();
    assert_eq!(guide.stations[0].guide_key, "17043");
    assert_eq!(
        guide.stations[0].logo.as_deref(),
        Some("https://example.com/logo.png")
    );
    assert_eq!(
        guide.programs[0].metadata.episode.as_deref(),
        Some("S30E39")
    );
    assert_eq!(guide.programs[0].metadata.is_new, Some(true));
    assert_eq!(guide.programs[0].metadata.year, Some(2026));
    assert_eq!(
        guide.programs[0].metadata.series_id.as_deref(),
        Some("SH00225395")
    );
    assert_eq!(
        guide.programs[0].metadata.image.as_deref(),
        Some("https://zpmc.tmsimg.com/assets/p184466_b_v13_ah.jpg?w=960")
    );
    assert_eq!(
        guide.programs[0].metadata.episode_id.as_deref(),
        Some("EP002253956808")
    );
    assert!(guide.programs[1].metadata.episode_id.is_none());
}
fn program(
    start: i64,
    end: i64,
    episode: &str,
) -> Program {
    Program {
        id: 0,
        source_id: 1,
        guide_key: "station".into(),
        starts_at: start,
        ends_at: end,
        title: "Evening News".into(),
        metadata: formats::ProgramMetadata {
            episode_id: Some(episode.into()),
            is_new: Some(true),
            ..Default::default()
        },
    }
}

#[test]
fn m3u_keeps_quoted_commas_groups_and_relative_urls_without_exposing_credentials() {
    let playlist = concat!(
        "\u{feff}#EXTM3U\n#EXTINF:-1 tvg-id=\"station\" tvg-chno=\"7.1\" ",
        "group-title=\"News, local\",Network One\n../live.ts?token=secret\n",
        "#EXTINF:-1,Unmapped\nhttps://example.com/watch?password=hidden\n",
    );
    let rows = formats::m3u(playlist, "https://example.com/playlists/tv.m3u").unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].group_name, "News, local");
    assert_eq!(
        rows[0].stream_url,
        "https://example.com/live.ts?token=secret"
    );
    let public = serde_json::to_string(&rows).unwrap();
    assert!(!public.contains("token=secret"));
    assert!(!public.contains("password=hidden"));
    assert!(!public.contains("stream_url"));
}

#[test]
fn playlists_reject_active_urls_and_individual_hls_manifests() {
    for url in [
        "file:///private/secret",
        "javascript:alert(1)",
        "data:video/mp2t,secret",
    ] {
        assert!(
            formats::m3u(
                &format!("#EXTM3U\n#EXTINF:-1,TV\n{url}"),
                "https://example.com/list.m3u"
            )
            .is_err()
        );
    }
    assert!(
        formats::m3u(
            "#EXTM3U\n#EXT-X-TARGETDURATION:6\nsegment.ts",
            "https://example.com/live.m3u8"
        )
        .is_err()
    );
}

#[test]
fn xmltv_keeps_rich_metadata_and_uses_broadcast_timezone() {
    let guide = formats::parse_guide(XML).unwrap();
    assert_eq!(guide.programs.len(), 3);
    let program = &guide.programs[0];
    assert_eq!(
        program.starts_at,
        chrono::DateTime::parse_from_rfc3339("2026-10-30T23:00:00Z")
            .unwrap()
            .timestamp()
    );
    assert_eq!(
        program.metadata.description.as_deref(),
        Some("News & weather")
    );
    assert_eq!(program.metadata.categories, ["News"]);
    assert_eq!(program.metadata.credits, ["Host"]);
    assert_eq!(program.metadata.episode.as_deref(), Some("S1E2"));
    assert_eq!(program.metadata.is_new, Some(true));
    assert_eq!(guide.programs[1].ends_at, guide.programs[2].starts_at);
    assert_eq!(guide.stations[0].number, "7.1");
    assert!(formats::parse_guide("<!DOCTYPE tv [<!ENTITY x 'expansion'>]><tv/>").is_err());
    assert_eq!(formats::xml_time("20261030230000"), Some(program.starts_at));
}

#[test]
fn xmltv_artwork_matching_reads_movie_year_and_zero_based_episode_numbers() {
    let xml = XML.replace(
        "<episode-num system=\"onscreen\">S1E2</episode-num>",
        "<date>19580401</date>",
    );
    let guide = formats::parse_guide(&xml).unwrap();
    assert_eq!(guide.programs[0].metadata.year, Some(1958));
    assert_eq!(guide.programs[0].metadata.episode.as_deref(), Some("S1E2"));
    let invalid = xml.replace("0.1.", "4294967295.1.");
    assert!(
        formats::parse_guide(&invalid).unwrap().programs[0]
            .metadata
            .episode
            .is_none()
    );
}

#[test]
fn refresh_preserves_channel_ids_visibility_and_explicit_mapping() {
    let mut conn = database();
    storage::save_guide(&mut conn, 1, &formats::parse_guide(XML).unwrap()).unwrap();
    let before = storage::channels(&mut conn, true)
        .unwrap()
        .into_iter()
        .find(|c| c.source_id == 1)
        .unwrap();
    assert_eq!(before.guide_key, "station");
    assert!(before.logo.is_some());
    diesel::update(c::table.find(before.id))
        .set((c::enabled.eq(false), c::guide_override.eq("manual-station")))
        .execute(&mut conn)
        .unwrap();
    let mut replacement = before.clone();
    replacement.name = "Renamed channel".into();
    storage::save_channels(&mut conn, 1, &[replacement]).unwrap();
    storage::save_guide(&mut conn, 1, &formats::parse_guide(XML).unwrap()).unwrap();
    let after = storage::channel(&mut conn, before.id).unwrap();
    assert_eq!(after.name, "Renamed channel");
    assert!(!after.enabled);
    assert_eq!(after.guide_override.as_deref(), Some("manual-station"));
    storage::save_channels(&mut conn, 1, &[]).unwrap();
    assert!(!storage::channel(&mut conn, before.id).unwrap().available);
}

#[test]
fn failed_guide_transaction_preserves_previous_programs() {
    let mut conn = database();
    let start = now() + 1000;
    storage::save_programs(
        &mut conn,
        1,
        &[program(
            start,
            start + 1800,
            "first",
        )],
    )
    .unwrap();
    let invalid = program(start + 3600, start + 3500, "broken");
    assert!(storage::save_programs(&mut conn, 1, &[invalid]).is_err());
    let kept = storage::guide(&mut conn, start, start + 7200).unwrap();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].metadata.episode_id.as_deref(), Some("first"));
}

#[test]
fn ota_refresh_merges_other_multiplexes_instead_of_erasing_them() {
    let mut conn = database();
    let start = now() + 1000;
    storage::save_programs(
        &mut conn,
        1,
        &[program(
            start,
            start + 1800,
            "first",
        )],
    )
    .unwrap();
    let mut second = program(start, start + 1800, "second");
    second.guide_key = "another-multiplex".into();
    storage::merge_guide(
        &mut conn,
        1,
        formats::Guide {
            programs: vec![second],
            stations: vec![],
        },
    )
    .unwrap();
    assert_eq!(
        storage::guide(&mut conn, start, start + 7200)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn dvr_conflicts_include_padding_and_keep_other_users_private() {
    let mut conn = database();
    let start = now() + 3600;
    let mut first = timer(1, start, start + 1800);
    first.padding_after = 60;
    schedule(&mut conn, 1, first).unwrap();
    schedule(&mut conn, 2, timer(1, start + 1800, start + 3600)).unwrap();
    assert_eq!(dvr::conflicts(&mut conn, 1).unwrap().len(), 1);
    let own = storage::recordings(&mut conn, 1).unwrap();
    assert_eq!(own.len(), 1);
    assert_eq!(own[0].user_id, 1);
    diesel::update(s::table.find(1))
        .set(s::capacity.eq(2))
        .execute(&mut conn)
        .unwrap();
    assert!(dvr::conflicts(&mut conn, 1).unwrap().is_empty());
    diesel::update(s::table.find(1))
        .set(s::capacity.eq(1))
        .execute(&mut conn)
        .unwrap();
    diesel::update(r::table.filter(r::user_id.eq(1)))
        .set(r::padding_after.eq(0))
        .execute(&mut conn)
        .unwrap();
    assert!(dvr::conflicts(&mut conn, 1).unwrap().is_empty());
}

#[test]
fn series_timers_are_durable_deduplicate_episodes_and_link_the_selected_program() {
    let mut conn = database();
    let start = now() + 3600;
    let mut guide = formats::parse_guide(XML).unwrap();
    guide.programs = vec![
        program(start, start + 1000, "episode-1"),
        program(start + 2000, start + 3000, "episode-2"),
        program(start + 4000, start + 5000, "episode-2"),
    ];
    storage::save_guide(&mut conn, 1, &guide).unwrap();
    let mut request = timer(1, start, start + 1000);
    request.series = true;
    schedule(&mut conn, 1, request).unwrap();
    dvr::expand_rules(&mut conn).unwrap();
    dvr::expand_rules(&mut conn).unwrap();
    let recordings = storage::recordings(&mut conn, 1).unwrap();
    assert_eq!(recordings.len(), 2);
    assert!(recordings.iter().all(|r| r.rule_id.is_some()));
    assert_eq!(storage::rules(&mut conn, 2).unwrap().len(), 0);
    diesel::update(rules::table)
        .set(rules::enabled.eq(false))
        .execute(&mut conn)
        .unwrap();
    dvr::expand_rules(&mut conn).unwrap();
    assert_eq!(storage::recordings(&mut conn, 1).unwrap().len(), 2);
}

#[test]
fn new_only_rules_do_not_guess_when_broadcast_metadata_is_unknown() {
    let mut conn = database();
    let start = now() + 3600;
    let mut guide = formats::parse_guide(XML).unwrap();
    let mut unknown = program(start + 2000, start + 3000, "unknown");
    unknown.metadata.is_new = None;
    guide.programs = vec![
        program(start, start + 1000, "selected"),
        unknown,
    ];
    storage::save_guide(&mut conn, 1, &guide).unwrap();
    let mut request = timer(1, start, start + 1000);
    request.series = true;
    request.new_only = true;
    schedule(&mut conn, 1, request).unwrap();
    dvr::expand_rules(&mut conn).unwrap();
    assert_eq!(storage::recordings(&mut conn, 1).unwrap().len(), 1);
}

#[test]
fn recovery_keeps_recorded_file_references_and_reschedules_interrupted_capture() {
    let mut conn = database();
    let start = now() + 100;
    schedule(&mut conn, 1, timer(1, start, start + 3600)).unwrap();
    diesel::update(r::table)
        .set((r::state.eq("recording"), r::file_name.eq("recording-1.ts")))
        .execute(&mut conn)
        .unwrap();
    dvr::recover(&mut conn).unwrap();
    let saved = storage::recordings(&mut conn, 1).unwrap();
    assert_eq!(saved[0].state, "scheduled");
    assert!(saved[0].has_video);
    assert!(saved[0].last_error.is_some());
}

#[test]
fn invalid_timers_do_not_change_the_database() {
    let mut conn = database();
    let start = now() + 100;
    for request in [
        timer(1, start, start),
        timer(1, start, start + 86401),
        timer(1, 0, 1),
    ] {
        assert!(schedule(&mut conn, 1, request).is_err());
    }
    assert!(storage::recordings(&mut conn, 1).unwrap().is_empty());
}

#[test]
fn tuner_capacity_is_released_when_a_viewer_disconnects() {
    let source = Source {
        id: 900_001,
        name: "Lease test".into(),
        kind: "hdhomerun".into(),
        config_json: String::new(),
        enabled: true,
        capacity: 1,
        next_refresh: 0,
        last_error: None,
    };
    let first = stream::lease(&source).unwrap();
    assert!(stream::lease(&source).is_err());
    drop(first);
    assert!(stream::lease(&source).is_ok());
}

#[test]
fn broadcast_dvb_and_atsc_metadata_share_the_guide_model() {
    let guide = broadcast::parse(include_str!("fixtures/broadcast.xml")).unwrap();
    assert_eq!(guide.programs.len(), 2);
    let dvb = guide
        .programs
        .iter()
        .find(|p| p.guide_key == "dvb:32:16:48")
        .unwrap();
    assert_eq!(dvb.title, "Résumé");
    assert_eq!(
        dvb.metadata.description.as_deref(),
        Some("Headlines\nMore detail")
    );
    let atsc = guide
        .programs
        .iter()
        .find(|p| p.guide_key == "atsc:2")
        .unwrap();
    assert_eq!(
        atsc.metadata.description.as_deref(),
        Some("ATSC description")
    );
    assert_eq!(atsc.ends_at - atsc.starts_at, 3600);
    assert_eq!(atsc.metadata.episode_id, None);
    assert!(
        guide
            .stations
            .iter()
            .any(|s| s.guide_key == "atsc:2" && s.number == "7.1")
    );
}

#[test]
fn native_scan_files_keep_service_identity_and_virtual_channel_numbers() {
    let channels = native::parse_channels(
        "[Network One]\nFREQUENCY = 473000000\nSERVICE_ID = 12\nVCHANNEL = 7.1\n[Network \
         Two]\nFREQUENCY = 473000000\nSERVICE_ID = 13\nVCHANNEL = 7.2\n",
    )
    .unwrap();
    assert_eq!(channels.len(), 2);
    assert_eq!(channels[0].source_key, "473000000:12");
    assert_eq!(channels[0].number, "7.1");
}

#[rocket::async_test]
async fn compressed_xmltv_urls_work_without_an_http_content_encoding_header() {
    use std::io::{
        Read,
        Write,
    };
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(XML.as_bytes()).unwrap();
    let data = encoder.finish().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0; 4096];
        let _ = stream.read(&mut buffer);
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            data.len()
        )
        .unwrap();
        stream.write_all(&data).unwrap();
    });
    let value =
        sources::text(formats::http_url(&format!("http://{address}/guide.xml.gz")).unwrap())
            .await
            .unwrap();
    server.join().unwrap();
    assert_eq!(formats::parse_guide(&value).unwrap().programs.len(), 3);
}
