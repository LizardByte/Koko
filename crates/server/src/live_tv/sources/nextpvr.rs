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

pub(super) async fn guide(config: &SourceConfig) -> Result<Vec<Program>, String> {
    let sid = login(config).await?;
    let channels = parse_channels(&text(endpoint(config, "channel.list", Some(&sid))?).await?)?;
    let mut programs = Vec::new();
    for channel in channels {
        let mut url = endpoint(config, "channel.listings", Some(&sid))?;
        url.query_pairs_mut()
            .append_pair("channel_id", &channel.source_key)
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
                metadata: ProgramMetadata {
                    subtitle: child_text(node, "subtitle"),
                    description: child_text(node, "description"),
                    episode_id: child_text(node, "unique_id"),
                    is_new: child_text(node, "firstrun").map(|s| s == "true"),
                    ..Default::default()
                },
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
