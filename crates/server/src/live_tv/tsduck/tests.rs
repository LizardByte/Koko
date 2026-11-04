//! The packaged native ABI, file-backed tuning, and safe failure paths need no physical tuner.

use super::*;

#[rocket::async_test]
async fn the_bundled_component_enumerates_regions_and_decodes_transport_tables() {
    let regions = regions().await.unwrap();
    assert!(
        regions
            .iter()
            .any(|name| name.eq_ignore_ascii_case("north-america"))
    );
    assert!(
        regions
            .windows(2)
            .all(|pair| pair[0].to_lowercase() <= pair[1].to_lowercase())
    );
    let tuners = devices().await.unwrap();
    assert!(tuners.iter().all(|device| !device.id.is_empty()));
    let guide = decode(include_bytes!("../fixtures/sample.mpegts").to_vec())
        .await
        .unwrap();
    assert!(guide.programs.is_empty());
    assert!(
        guide
            .stations
            .iter()
            .any(|station| station.name == "Service01")
    );
    let native = api().unwrap();
    assert!(
        unsafe { native.string(std::ptr::null_mut()) }
            .unwrap_err()
            .contains("Native TV operation failed")
    );
    assert!(cstring("invalid\0device").is_err());
}

#[test]
fn native_scan_results_validate_identifiers_and_keep_tuning_and_display_metadata() {
    let channels = parse_channels(
        "<channels><channel id='1' frequency='473000000' tuning='--frequency 473000000' \
         name='Workshop' number='7.1'/><channel id='2' frequency='479000000' tuning='--frequency \
         479000000'/><channel id='missing'/></channels>",
    )
    .unwrap();
    assert_eq!(channels.len(), 2);
    assert_eq!(channels[0].source_key, "473000000:1");
    assert_eq!(channels[0].name, "Workshop");
    assert_eq!(channels[0].number, "7.1");
    assert_eq!(channels[1].name, "Service 2");
    for xml in [
        "broken XML",
        "<channels/>",
        "<channels><channel id='bad' frequency='1' tuning='x'/></channels>",
        "<channels><channel id='1' frequency='bad' tuning='x'/></channels>",
    ] {
        assert!(parse_channels(xml).is_err());
    }
    let config = SourceConfig::default();
    assert_ne!(cache(1, &config), cache(2, &config));
    let other = SourceConfig {
        region: "europe".into(),
        ..Default::default()
    };
    assert_ne!(cache(1, &config), cache(1, &other));
}

#[rocket::async_test]
async fn rejected_scans_release_the_source_and_do_not_require_a_real_tuner() {
    let database = crate::live_tv::test_support::Database::new().await;
    let config = SourceConfig {
        native_device: database
            .directory
            .path()
            .join("missing.xml")
            .to_string_lossy()
            .into_owned(),
        ..Default::default()
    };
    let (source, _) = database.source("native", config.clone(), "").await;
    assert!(scan_channels(&source, &config).await.is_err());
    assert!(!crate::live_tv::stream::source_in_use(source.id));
    let invalid = SourceConfig {
        native_device: "bad\0device".into(),
        ..Default::default()
    };
    assert!(
        scan_channels(&source, &invalid)
            .await
            .err()
            .unwrap()
            .contains("invalid character")
    );
    assert!(!crate::live_tv::stream::source_in_use(source.id));
    let db = database.conn().await;
    rescan(&db, source.id).await.unwrap();
    let id = source.id;
    assert_eq!(
        db.run(move |conn| crate::live_tv::storage::source(conn, id))
            .await
            .unwrap()
            .next_refresh,
        0
    );
    let (other, _) = database.source("m3u", SourceConfig::default(), "").await;
    assert!(
        rescan(&db, other.id)
            .await
            .unwrap_err()
            .contains("local tuner")
    );
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
#[rocket::async_test]
async fn a_file_backed_tuner_reads_transport_packets_and_closes_on_disconnect() {
    use tokio::io::AsyncReadExt;
    let directory = tempfile::tempdir().unwrap();
    let transport = directory.path().join("sample.mpegts");
    tokio::fs::write(&transport, include_bytes!("../fixtures/sample.mpegts"))
        .await
        .unwrap();
    let emulator = directory.path().join("tuner.xml");
    tokio::fs::write(
        &emulator,
        concat!(
            "<tsduck><tuner type='ATSC'/><defaults directory='.'/>",
            "<channel frequency='473000000' delivery='ATSC' file='sample.mpegts'/></tsduck>"
        ),
    )
    .await
    .unwrap();
    let config = SourceConfig {
        native_device: emulator.to_string_lossy().into_owned(),
        ..Default::default()
    };
    let channel = Channel::new(
        "473000000:1".into(),
        "Synthetic tuner".into(),
        "--delivery-system ATSC --frequency 473000000".into(),
    );
    let mut reader = tune(&config, &channel).await.unwrap();
    let mut packets = [0; 188 * 3];
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        reader.read_exact(&mut packets),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        packets
            .as_chunks::<188>()
            .0
            .iter()
            .all(|packet| packet[0] == 0x47)
    );
    drop(reader);
    let destination = directory.path().join("released.mpegts");
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while tokio::fs::rename(&transport, &destination).await.is_err() {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}
