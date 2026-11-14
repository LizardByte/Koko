//! NextPVR's salted PIN session protocol and channel/guide APIs.

use super::{
    SourceConfig,
    client,
    formats,
    text,
};
use crate::live_tv::{
    Channel,
    Program,
    formats::{
        ProgramMetadata,
        child_text,
    },
};
use reqwest::Url;

fn endpoint(
    config: &SourceConfig,
    method: &str,
    sid: Option<&str>,
) -> Result<Url, String> {
    let mut url = formats::endpoint(&config.url, "service")?;
    url.query_pairs_mut().append_pair("method", method);
    if let Some(sid) = sid {
        url.query_pairs_mut().append_pair("sid", sid);
    }
    Ok(url)
}

fn checked(value: &str) -> Result<roxmltree::Document<'_>, String> {
    let doc = roxmltree::Document::parse(value).map_err(|_| "NextPVR returned invalid XML")?;
    if doc.root_element().attribute("stat") != Some("ok") {
        return Err(
            "NextPVR rejected the request; check the server PIN and remote access settings".into(),
        );
    }
    Ok(doc)
}

pub(super) async fn login(config: &SourceConfig) -> Result<String, String> {
    let mut url = endpoint(config, "session.initiate", None)?;
    url.query_pairs_mut()
        .append_pair("ver", "1.0")
        .append_pair("device", "Koko");
    let response = text(url).await?;
    let doc = checked(&response)?;
    let sid = child_text(doc.root_element(), "sid").ok_or("NextPVR did not supply a session")?;
    let salt =
        child_text(doc.root_element(), "salt").ok_or("NextPVR did not supply a session salt")?;
    // NextPVR mandates MD5(":" + MD5(pin) + ":" + salt) for its session challenge.
    // This is wire-protocol compatibility, not Koko's password-storage algorithm.
    // https://forums.nextpvr.com/showthread.php?pid=524900&tid=59581
    let pin = format!("{:x}", md5::compute(config.pin.as_bytes()));
    let digest = format!("{:x}", md5::compute(format!(":{pin}:{salt}")));
    let mut url = endpoint(config, "session.login", Some(&sid))?;
    url.query_pairs_mut().append_pair("md5", &digest);
    checked(&text(url).await?)?;
    Ok(sid)
}

fn parse_channels(xml: &str) -> Result<Vec<Channel>, String> {
    let doc = checked(xml)?;
    let mut rows = Vec::new();
    for node in doc.descendants().filter(|n| n.has_tag_name("channel")) {
        let (Some(id), Some(name)) = (child_text(node, "id"), child_text(node, "name")) else {
            continue;
        };
        let mut channel = Channel::new(id, name, String::new());
        let major = child_text(node, "number").unwrap_or_default();
        let minor = child_text(node, "minor").filter(|s| s != "0");
        channel.number = minor.map_or_else(|| major.clone(), |minor| format!("{major}.{minor}"));
        rows.push(channel);
    }
    if rows.is_empty() {
        return Err("NextPVR has no configured channels; scan the tuner in NextPVR first".into());
    }
    Ok(rows)
}

pub(super) async fn channels(config: &SourceConfig) -> Result<Vec<Channel>, String> {
    let sid = login(config).await?;
    parse_channels(&text(endpoint(config, "channel.list", Some(&sid))?).await?)
}

fn timestamp(value: Option<String>) -> Option<i64> {
    let value = value?;
    let parsed = value.parse::<i64>().ok()?;
    Some(if parsed > 10_000_000_000 { parsed / 1000 } else { parsed })
}

fn metadata(node: roxmltree::Node<'_, '_>) -> ProgramMetadata {
    let significance = child_text(node, "significance")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let episode = child_text(node, "episode").map(|episode| {
        child_text(node, "season").map_or_else(
            || format!("Episode {episode}"),
            |season| format!("S{season}E{episode}"),
        )
    });
    ProgramMetadata {
        subtitle: child_text(node, "subtitle"),
        description: child_text(node, "description"),
        episode_id: child_text(node, "unique_id"),
        is_new: child_text(node, "firstrun").map(|s| s.eq_ignore_ascii_case("true")),
        categories: node
            .descendants()
            .filter(|n| n.has_tag_name("genre"))
            .filter_map(|n| n.text())
            .map(str::to_owned)
            .collect(),
        is_live: Some(significance == "live"),
        is_premiere: Some(significance.contains("premiere")),
        is_finale: Some(significance.contains("finale")),
        rating: child_text(node, "rating"),
        episode,
        ..Default::default()
    }
}

pub(super) async fn guide(config: &SourceConfig) -> Result<Vec<Program>, String> {
    let sid = login(config).await?;
    let channels = parse_channels(&text(endpoint(config, "channel.list", Some(&sid))?).await?)?;
    let mut programs = Vec::new();
    for channel in channels {
        let mut url = endpoint(config, "channel.listings", Some(&sid))?;
        url.query_pairs_mut()
            .append_pair("channel_id", &channel.source_key)
            .append_pair("genre", "all")
            .append_pair("start", &crate::live_tv::now().to_string())
            .append_pair("end", &(crate::live_tv::now() + 14 * 86400).to_string());
        let response = text(url).await?;
        let doc = checked(&response)?;
        for node in doc.descendants().filter(|n| n.has_tag_name("l")) {
            let (Some(start), Some(end), Some(title)) = (
                timestamp(child_text(node, "start")),
                timestamp(child_text(node, "end")),
                child_text(node, "name"),
            ) else {
                continue;
            };
            if end <= start {
                continue;
            }
            programs.push(Program {
                id: 0,
                source_id: 0,
                guide_key: channel.source_key.clone(),
                starts_at: start,
                ends_at: end,
                title,
                metadata: metadata(node),
            });
        }
    }
    Ok(programs)
}

pub(super) async fn stream_url(
    config: &SourceConfig,
    channel: &str,
) -> Result<String, String> {
    let sid = login(config).await?;
    let mut url = formats::endpoint(&config.url, "live")?;
    url.query_pairs_mut()
        .append_pair("channeloid", channel)
        .append_pair("client", &format!("Koko-{sid}"))
        .append_pair("sid", &sid);
    Ok(url.to_string())
}

pub(super) fn keepalive(
    config: &SourceConfig,
    stream: &str,
) -> Result<tokio::task::JoinHandle<()>, String> {
    let sid = formats::http_url(stream)?
        .query_pairs()
        .find(|(key, _)| key == "sid")
        .map(|(_, value)| value.into_owned())
        .ok_or("Missing NextPVR playback session")?;
    let url = endpoint(config, "session.valid", Some(&sid))?;
    let client = client()?;
    Ok(tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if super::bytes(client.get(url.clone()), 1024 * 1024)
                .await
                .is_err()
            {
                log::warn!("Unable to renew a NextPVR playback session");
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rocket::async_test]
    async fn sessions_channels_listings_and_keepalive_use_the_server_protocol() {
        use crate::live_tv::test_support::HttpFixture;
        let now = crate::live_tv::now();
        let fixture = HttpFixture::new(move |path| {
            let url = Url::parse(&format!("http://fixture{path}")).unwrap();
            let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
            let body = match query.get("method").map(String::as_str).unwrap() {
                "session.initiate" => {
                    "<rsp stat='ok'><sid>fixture-session</sid><salt>challenge</salt></rsp>".into()
                }
                "session.login" => {
                    assert_eq!(query["sid"], "fixture-session");
                    assert_eq!(query["md5"].len(), 32);
                    assert!(query["md5"].bytes().all(|value| value.is_ascii_hexdigit()));
                    "<rsp stat='ok'/>".into()
                }
                "channel.list" => "<rsp stat='ok'><channel><id>11</id><name>Workshop</\
                                   name><number>7</number><minor>1</minor></\
                                   channel><channel><id>12</id><name>Weather</name><number>8</\
                                   number><minor>0</minor></channel><channel><id>missing-name</\
                                   id></channel></rsp>"
                    .into(),
                "channel.listings" => format!(
                    "<rsp stat='ok'><l><start>{}</start><end>{}</end><name>Weekend \
                     Workshop</name><subtitle>Table restoration</subtitle><description>A wooden \
                     table is restored.</description><unique_id>workshop-1</\
                     unique_id><genre>Home</genre><season>2</season><episode>3</\
                     episode><firstrun>true</firstrun></l><l><start>10</start><end>5</\
                     end><name>Invalid \
                     interval</name></l><l><start>bad</start><end>20</end></l></rsp>",
                    now * 1000,
                    (now + 3600) * 1000
                ),
                "session.valid" => "<rsp stat='ok'/>".into(),
                _ => panic!("Unexpected NextPVR request"),
            };
            (200, "application/xml", body.into_bytes())
        })
        .await;
        let config = SourceConfig {
            url: fixture.url.clone(),
            pin: format!("{:x}", rand::random::<u128>()),
            ..Default::default()
        };
        let channels = channels(&config).await.unwrap();
        assert_eq!(
            channels
                .iter()
                .map(|c| c.number.as_str())
                .collect::<Vec<_>>(),
            ["7.1", "8"]
        );
        let programs = guide(&config).await.unwrap();
        assert_eq!(programs.len(), 2);
        assert_eq!(programs[0].starts_at, now);
        assert_eq!(programs[0].ends_at, now + 3600);
        assert_eq!(programs[0].metadata.episode.as_deref(), Some("S2E3"));
        assert_eq!(
            programs[0].metadata.subtitle.as_deref(),
            Some("Table restoration")
        );
        assert_eq!(
            programs[0].metadata.episode_id.as_deref(),
            Some("workshop-1")
        );
        let stream = stream_url(&config, "11").await.unwrap();
        let url = Url::parse(&stream).unwrap();
        assert_eq!(url.path(), "/live");
        assert!(
            url.query_pairs()
                .any(|(key, value)| key == "channeloid" && value == "11")
        );
        let task = keepalive(&config, &stream).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !fixture
                .requests
                .lock()
                .unwrap()
                .iter()
                .any(|path| path.contains("session.valid"))
            {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        task.abort();
        assert!(keepalive(&config, &fixture.url).is_err());
    }

    #[rocket::async_test]
    async fn malformed_or_rejected_nextpvr_sessions_do_not_become_playable_channels() {
        use crate::live_tv::test_support::HttpFixture;
        for (body, message) in [
            ("broken XML", "invalid XML"),
            ("<rsp stat='fail'/>", "rejected"),
            ("<rsp stat='ok'/>", "supply a session"),
            ("<rsp stat='ok'><sid>fixture</sid></rsp>", "session salt"),
        ] {
            let fixture =
                HttpFixture::new(move |_| (200, "application/xml", body.as_bytes().to_vec())).await;
            let config = SourceConfig {
                url: fixture.url.clone(),
                ..Default::default()
            };
            assert!(login(&config).await.unwrap_err().contains(message));
        }
        assert!(
            parse_channels("<rsp stat='ok'/>")
                .unwrap_err()
                .contains("no configured channels")
        );
        assert_eq!(timestamp(None), None);
        assert_eq!(timestamp(Some("not-a-time".into())), None);
        assert_eq!(timestamp(Some("1793595600".into())), Some(1793595600));
    }

    #[test]
    fn guide_metadata_keeps_genres_and_broadcast_flags_independent() {
        let doc = roxmltree::Document::parse(
            "<l><genres><genre>Sports</genre><genre>Football</genre></genres><significance>Live</\
             significance><firstrun>true</firstrun><season>2</season><episode>3</\
             episode><rating>TV-PG</rating></l>",
        )
        .unwrap();
        let program = metadata(doc.root_element());
        assert_eq!(program.categories, ["Sports", "Football"]);
        assert_eq!(program.is_live, Some(true));
        assert_eq!(program.is_new, Some(true));
        assert_eq!(program.is_premiere, Some(false));
        assert_eq!(program.episode.as_deref(), Some("S2E3"));
        assert_eq!(program.rating.as_deref(), Some("TV-PG"));
        let doc = roxmltree::Document::parse(
            "<l><significance>Season Premiere</significance><firstrun>false</firstrun></l>",
        )
        .unwrap();
        let program = metadata(doc.root_element());
        assert_eq!(program.is_live, Some(false));
        assert_eq!(program.is_new, Some(false));
        assert_eq!(program.is_premiere, Some(true));
    }
}
