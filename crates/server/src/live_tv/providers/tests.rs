//! Provider preferences and image links must survive refreshes without changing channel identity.
use super::*;
use crate::db::schema::live_tv_sources as s;

fn fixture() -> (SqliteConnection, Source, Channel, Metadata) {
    let mut conn = crate::live_tv::tests::database();
    let config = sources::SourceConfig {
        provider_grouping: true,
        url: "http://fixture/tv.m3u".into(),
        ..Default::default()
    };
    diesel::update(s::table.find(1))
        .set(s::config_json.eq(serde_json::to_string(&config).unwrap()))
        .execute(&mut conn)
        .unwrap();
    let channel = c::table
        .filter(c::source_id.eq(1))
        .select(Channel::as_select())
        .first::<Channel>(&mut conn)
        .unwrap();
    let mut data = Metadata::default();
    data.channels.insert(
        channel.source_key.clone(),
        ChannelIdentity {
            provider: "espn".into(),
            identity: "espn.espn1".into(),
            bridge: true,
        },
    );
    save(&mut conn, 1, &data).unwrap();
    let source = storage::source(&mut conn, 1).unwrap();
    (conn, source, channel, data)
}

#[test]
fn provider_visibility_and_names_preserve_channel_settings_across_refresh() {
    let (mut conn, source, channel, data) = fixture();
    update(
        &mut conn,
        1,
        "espn",
        ProviderUpdate {
            name: "My Sports".into(),
            enabled: false,
            icon_channel_id: None,
            remove_icon: false,
        },
    )
    .unwrap();
    assert!(!allowed(&source, &channel, &load(&mut conn, 1).unwrap()));
    assert!(storage::channel(&mut conn, channel.id).unwrap().enabled);
    assert!(
        decorate(&mut conn, vec![channel.clone()], false)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        decorate(&mut conn, vec![channel.clone()], true).unwrap()[0]
            .provider_id
            .as_deref(),
        Some("espn")
    );
    let mut saved = load(&mut conn, 1).unwrap();
    super::super::iptv::merge_metadata(&mut saved, data.channels);
    save(&mut conn, 1, &saved).unwrap();
    assert_eq!(
        summaries(&mut conn, &source, &saved).unwrap()[0].name,
        "My Sports"
    );
    update(
        &mut conn,
        1,
        "espn",
        ProviderUpdate {
            name: "ESPN".into(),
            enabled: true,
            icon_channel_id: None,
            remove_icon: false,
        },
    )
    .unwrap();
    assert!(allowed(&source, &channel, &load(&mut conn, 1).unwrap()));
    assert_eq!(
        storage::channel(&mut conn, channel.id).unwrap().source_key,
        channel.source_key
    );
}

#[test]
fn channel_linked_provider_images_follow_logo_updates_and_reject_other_sources() {
    let (mut conn, source, channel, _) = fixture();
    let foreign = c::table
        .filter(c::source_id.eq(2))
        .select(c::id)
        .first::<i32>(&mut conn)
        .unwrap();
    assert!(
        update(
            &mut conn,
            1,
            "espn",
            ProviderUpdate {
                name: "ESPN".into(),
                enabled: true,
                icon_channel_id: Some(foreign),
                remove_icon: false
            }
        )
        .is_err()
    );
    update(
        &mut conn,
        1,
        "espn",
        ProviderUpdate {
            name: "ESPN".into(),
            enabled: true,
            icon_channel_id: Some(channel.id),
            remove_icon: false,
        },
    )
    .unwrap();
    for logo in [
        "http://logos.example/first.png",
        "http://logos.example/updated.png",
    ] {
        diesel::update(c::table.find(channel.id))
            .set(c::logo.eq(logo))
            .execute(&mut conn)
            .unwrap();
        let data = load(&mut conn, 1).unwrap();
        let provider = summaries(&mut conn, &source, &data).unwrap().remove(0);
        assert_eq!(provider.image_url.as_deref(), Some(logo));
        assert_eq!(provider.icon_channel_id, Some(channel.id));
    }
    set_image(&mut conn, 1, Some("espn"), Some("fixture".into())).unwrap();
    let data = load(&mut conn, 1).unwrap();
    let provider = summaries(&mut conn, &source, &data).unwrap().remove(0);
    assert!(provider.has_custom_icon);
    assert_eq!(provider.icon_channel_id, None);
    assert!(
        provider
            .image_url
            .unwrap()
            .starts_with("/api/v1/live-tv/sources/1/image?")
    );
    set_image(&mut conn, 1, Some("espn"), None).unwrap();
    assert!(
        load(&mut conn, 1).unwrap().providers["espn"]
            .image
            .is_none()
    );
}

#[test]
fn uploaded_icons_are_normalized_bounded_and_replace_previous_images() {
    let (mut conn, _, _, _) = fixture();
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(1000, 500)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let encoded = prepare_image(png.get_ref()).unwrap();
    let resized = image::load_from_memory(&STANDARD.decode(&encoded).unwrap()).unwrap();
    assert_eq!((resized.width(), resized.height()), (256, 128));
    assert!(prepare_image(b"<svg></svg>").is_err());
    assert!(prepare_image(&vec![0; 5 * 1024 * 1024 + 1]).is_err());
    set_image(&mut conn, 1, None, Some(encoded.clone())).unwrap();
    let first = load(&mut conn, 1).unwrap().image_url(1, None).unwrap();
    set_image(&mut conn, 1, None, Some(encoded)).unwrap();
    let second = load(&mut conn, 1).unwrap().image_url(1, None).unwrap();
    assert_ne!(first, second);
    assert!(image(&mut conn, 1, None).is_ok());
    set_image(&mut conn, 1, None, None).unwrap();
    assert!(image(&mut conn, 1, None).is_err());
}
