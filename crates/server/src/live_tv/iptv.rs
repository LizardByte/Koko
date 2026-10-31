//! One IPTV connection can import complementary FastChannels transports with stable channel IDs.
use super::{
    Channel,
    feeds,
    formats,
    providers::{
        ChannelIdentity,
        Metadata,
    },
    sources::SourceConfig,
};
use std::collections::HashMap;

pub(super) struct Imported {
    pub channels: Vec<Channel>,
    pub identities: HashMap<String, ChannelIdentity>,
    pub playlist_bytes: u64,
    pub guide: Option<(formats::Guide, u64)>,
}

pub(super) fn provider(identity: &str) -> String {
    identity
        .split_once('.')
        .map(|(prefix, _)| prefix.trim())
        .filter(|p| !p.is_empty())
        .unwrap_or("other")
        .to_ascii_lowercase()
}

pub(super) fn endpoint(
    base: &str,
    path: &str,
) -> Result<reqwest::Url, String> {
    let url = formats::http_url(base)?;
    if url.query().is_some() {
        return Err(
            "Enter the FastChannels server address without a playlist path or query".into(),
        );
    }
    formats::endpoint(base, path)
}

pub(super) async fn import(config: &SourceConfig) -> Result<Imported, String> {
    let fast = config.iptv_mode == "fastchannels";
    let urls = if fast {
        vec![
            (endpoint(&config.url, "feeds/default/native/m3u")?, false),
            (endpoint(&config.url, "feeds/default/m3u/prismcast")?, true),
            (
                endpoint(&config.url, "feeds/default/m3u/prismcast/gracenote")?,
                true,
            ),
        ]
    } else {
        vec![(formats::http_url(&config.url)?, false)]
    };
    let mut channels = Vec::new();
    let mut identities = HashMap::new();
    let mut playlist_bytes = 0;
    for (url, bridge) in urls {
        let (playlist, size) = feeds::playlist_details(url, fast, fast).await?;
        playlist_bytes += size;
        for channel in playlist.channels {
            if identities.contains_key(&channel.source_key) {
                continue;
            }
            let identity = playlist
                .identities
                .get(&channel.source_key)
                .map(String::as_str)
                .unwrap_or("");
            identities.insert(
                channel.source_key.clone(),
                ChannelIdentity {
                    provider: provider(identity),
                    bridge,
                    identity: identity.into(),
                },
            );
            channels.push(channel);
        }
    }
    if channels.is_empty() {
        return Err("No playable IPTV channels were found".into());
    }
    // Native XMLTV includes both direct and DRM-bridge channels, including Gracenote-mapped ones.
    let guide = if fast && matches!(config.guide_kind.as_str(), "" | "automatic") {
        Some(feeds::guide_input(endpoint(&config.url, "feeds/default/native/epg.xml")?).await?)
    } else {
        None
    };
    Ok(Imported {
        channels,
        identities,
        playlist_bytes,
        guide,
    })
}

pub(super) fn merge_metadata(
    metadata: &mut Metadata,
    identities: HashMap<String, ChannelIdentity>,
) {
    metadata.channels = identities;
    // Provider preferences remain even if a provider temporarily disappears from the upstream feed.
}

/// Converting an existing playlist to the server preset preserves unambiguous channel identities.
pub(super) fn reconcile(
    conn: &mut diesel::SqliteConnection,
    id: i32,
    rows: &[Channel],
    incoming: &HashMap<String, ChannelIdentity>,
    previous: &Metadata,
) -> diesel::QueryResult<()> {
    use crate::db::schema::live_tv_channels as c;
    use diesel::prelude::*;
    let existing = c::table
        .filter(c::source_id.eq(id))
        .select(Channel::as_select())
        .load::<Channel>(conn)?;
    let mut keys = existing
        .iter()
        .map(|c| c.source_key.clone())
        .collect::<std::collections::HashSet<_>>();
    let mut claimed = std::collections::HashSet::new();
    let mut identities: HashMap<&str, Vec<&Channel>> = HashMap::new();
    let mut guide_numbers: HashMap<(&str, &str), Vec<&Channel>> = HashMap::new();
    for channel in &existing {
        if let Some(info) = previous
            .channels
            .get(&channel.source_key)
            .filter(|i| !i.identity.is_empty())
        {
            identities.entry(&info.identity).or_default().push(channel);
        }
        guide_numbers
            .entry((&channel.guide_key, &channel.number))
            .or_default()
            .push(channel);
    }
    for row in rows {
        if keys.contains(&row.source_key) {
            continue;
        }
        let identity = incoming
            .get(&row.source_key)
            .map(|i| i.identity.as_str())
            .unwrap_or("");
        let matching = identities
            .get(identity)
            .or_else(|| guide_numbers.get(&(row.guide_key.as_str(), row.number.as_str())));
        if let Some(matching) = matching.filter(|matches| matches.len() == 1) {
            let channel = matching[0];
            if claimed.insert(channel.id) {
                diesel::update(c::table.find(channel.id))
                    .set(c::source_key.eq(&row.source_key))
                    .execute(conn)?;
                keys.insert(row.source_key.clone());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[rocket::async_test]
    async fn one_address_imports_native_and_both_prismcast_variants_with_their_guide() {
        use std::io::{
            Read,
            Write,
        };
        for bridge_status in [200, 409, 503] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let native="#EXTM3U\n#EXTINF:-1 channel-id=\"pluto.news\" \
                tvg-id=\"pluto.news\",News\nhttp://tv.example/news.ts\n";
            let bridge="#EXTM3U\n#EXTINF:-1 channel-id=\"espn.espn1\" \
                tvg-id=\"espn.espn1\",ESPN\nhttp://bridge.example/espn.m3u8\n";
            let gracenote="#EXTM3U\n#EXTINF:-1 channel-id=\"sling.sports\" \
                tvc-guide-stationid=\"123\",Sports\nhttp://bridge.example/sports.m3u8\n";
            let now = chrono::Utc::now();
            let guide = format!(
                "<tv><channel \
                 id='espn.espn1'><display-name>ESPN</display-name></channel><programme \
                 channel='espn.espn1' start='{} +0000' stop='{} \
                 +0000'><title>Event</title></programme></tv>",
                now.format("%Y%m%d%H%M%S"),
                (now + chrono::Duration::hours(1)).format("%Y%m%d%H%M%S")
            );
            let mut responses = vec![
                ("/feeds/default/native/m3u", 200, native.into()),
                ("/feeds/default/m3u/prismcast", bridge_status, bridge.into()),
            ];
            if bridge_status != 503 {
                responses.push((
                    "/feeds/default/m3u/prismcast/gracenote",
                    bridge_status,
                    gracenote.into(),
                ));
                responses.push(("/feeds/default/native/epg.xml", 200, guide));
            }
            let server = std::thread::spawn(move || {
                for (path, status, body) in responses {
                    let (mut socket, _) = listener.accept().unwrap();
                    socket
                        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                        .unwrap();
                    let mut buffer = [0; 4096];
                    let size = socket.read(&mut buffer).unwrap();
                    assert!(
                        String::from_utf8_lossy(&buffer[..size])
                            .starts_with(&format!("GET {path} "))
                    );
                    write!(
                        socket,
                        "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: \
                         close\r\n\r\n{body}",
                        body.len()
                    )
                    .unwrap();
                }
            });
            let config = SourceConfig {
                url: base,
                iptv_mode: "fastchannels".into(),
                guide_kind: "automatic".into(),
                provider_grouping: true,
                ..Default::default()
            };
            let result = import(&config).await;
            server.join().unwrap();
            if bridge_status == 503 {
                assert!(result.is_err());
                continue;
            }
            let result = result.unwrap();
            assert!(result.playlist_bytes > 0);
            assert_eq!(result.guide.as_ref().unwrap().0.programs[0].title, "Event");
            assert_eq!(
                result.channels.len(),
                if bridge_status == 409 { 1 } else { 3 }
            );
            if bridge_status == 200 {
                assert!(result.identities["fastchannels:espn.espn1"].bridge);
                assert_eq!(
                    result
                        .channels
                        .iter()
                        .find(|c| c.name == "Sports")
                        .unwrap()
                        .guide_key,
                    "sling.sports"
                );
            }
        }
    }
    #[test]
    fn provider_prefix_is_independent_of_guide_ids_and_channel_names() {
        assert_eq!(provider("espn.espn1"), "espn");
        assert_eq!(provider("foxone.MyNetwork TV"), "foxone");
        assert_eq!(provider("124636"), "other");
        assert_eq!(
            endpoint("http://localhost:5523/sub/", "feeds/default/native/m3u")
                .unwrap()
                .path(),
            "/sub/feeds/default/native/m3u"
        );
        assert!(endpoint("http://localhost:5523?secret=x", "feeds/default/native/m3u").is_err());
    }

    #[test]
    fn fastchannels_ids_stay_stable_across_guide_mapping_number_and_transport_changes() {
        let mut keys = Vec::new();
        for (guide, number, url) in [
            ("32645", "5167", "direct.ts"),
            ("espn.espn1", "90", "bridge.m3u8"),
        ] {
            let m3u = format!("#EXTM3U\n#EXTINF:-1 channel-id=\"espn.espn1\" \
                tvg-id=\"{guide}\" tvg-chno=\"{number}\",ESPN\nhttp://example.com/{url}\n");
            let parsed =
                formats::read_playlist(m3u.as_bytes(), "http://example.com", true, true).unwrap();
            assert_eq!(parsed.channels[0].guide_key, guide);
            assert_eq!(
                parsed.identities[&parsed.channels[0].source_key],
                "espn.espn1"
            );
            keys.push(parsed.channels[0].source_key.clone());
        }
        assert_eq!(keys[0], keys[1]);
        let eplus="#EXTM3U\n#EXTINF:0 channel-id=\"foxone.FOX\" \
            tvg-id=\"410.eplustv\" tvg-chno=\"410\",FOX\nhttp://example.com/fox.ts\n";
        let parsed =
            formats::read_playlist(eplus.as_bytes(), "http://example.com", false, false).unwrap();
        assert_eq!(parsed.channels[0].source_key, "410.eplustv:410");
        assert_eq!(
            provider(&parsed.identities[&parsed.channels[0].source_key]),
            "foxone"
        );
        assert!(
            formats::read_playlist("#EXTM3U\n".as_bytes(), "http://example.com", true, true)
                .unwrap()
                .channels
                .is_empty()
        );
        assert!(
            formats::read_playlist("#EXTM3U\n".as_bytes(), "http://example.com", false, false)
                .is_err()
        );
    }

    #[test]
    fn converting_existing_playlist_preserves_channel_ids_and_user_overrides() {
        use crate::db::schema::live_tv_channels as c;
        use diesel::prelude::*;
        let mut conn = crate::live_tv::tests::database();
        let before = c::table
            .filter(c::source_id.eq(1))
            .select(Channel::as_select())
            .first::<Channel>(&mut conn)
            .unwrap();
        diesel::update(c::table.find(before.id))
            .set((c::enabled.eq(false), c::guide_override.eq("custom")))
            .execute(&mut conn)
            .unwrap();
        let mut next = before.clone();
        next.source_key = "fastchannels:espn.espn1".into();
        reconcile(
            &mut conn,
            1,
            &[next.clone()],
            &HashMap::new(),
            &Metadata::default(),
        )
        .unwrap();
        super::super::storage::save_channels(&mut conn, 1, &[next]).unwrap();
        let after = super::super::storage::channel(&mut conn, before.id).unwrap();
        assert_eq!(after.source_key, "fastchannels:espn.espn1");
        assert!(!after.enabled);
        assert_eq!(after.guide_override.as_deref(), Some("custom"));
    }
}
