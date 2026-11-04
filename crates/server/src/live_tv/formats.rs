//! Channel playlist parsing and shared guide data types.

mod xmltv;

use rocket_okapi::okapi::schemars::{
    self,
    JsonSchema,
};
use serde::{
    Deserialize,
    Serialize,
};
use sha2::{
    Digest,
    Sha256,
};
use std::collections::{
    HashMap,
    HashSet,
};

#[derive(
    Clone, Debug, Serialize, Deserialize, JsonSchema, diesel::Queryable, diesel::Selectable,
)]
#[diesel(table_name = crate::db::schema::live_tv_channels)]
pub(crate) struct Channel {
    pub id: i32,
    pub source_id: i32,
    pub source_key: String,
    pub name: String,
    pub number: String,
    pub group_name: String,
    pub logo: Option<String>,
    #[serde(skip)]
    pub stream_url: String,
    pub guide_key: String,
    pub guide_override: Option<String>,
    pub enabled: bool,
    pub available: bool,
}

impl Channel {
    pub(super) fn new(
        key: String,
        name: String,
        url: String,
    ) -> Self {
        Self {
            id: 0,
            source_id: 0,
            guide_key: key.clone(),
            source_key: key,
            name,
            number: String::new(),
            group_name: String::new(),
            logo: None,
            stream_url: url,
            guide_override: None,
            enabled: true,
            available: true,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub(crate) struct ProgramMetadata {
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub categories: Vec<String>,
    pub image: Option<String>,
    pub poster: Option<String>,
    pub year: Option<i32>,
    pub series_id: Option<String>,
    pub artwork_sources: Vec<String>,
    pub rating: Option<String>,
    pub episode: Option<String>,
    pub episode_id: Option<String>,
    pub credits: Vec<String>,
    pub is_new: Option<bool>,
    pub is_live: Option<bool>,
    pub is_premiere: Option<bool>,
    pub is_finale: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub(crate) struct Program {
    pub id: i32,
    pub source_id: i32,
    pub guide_key: String,
    pub starts_at: i64,
    pub ends_at: i64,
    pub title: String,
    pub metadata: ProgramMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub(crate) struct Station {
    pub source_id: i32,
    pub guide_key: String,
    pub name: String,
    pub number: String,
    pub aliases: Vec<String>,
    pub logo: Option<String>,
}

pub(super) struct Guide {
    pub programs: Vec<Program>,
    pub stations: Vec<Station>,
}

pub(super) fn endpoint(
    base: &str,
    path: &str,
) -> Result<reqwest::Url, String> {
    let mut base = http_url(base)?;
    if !base.path().ends_with('/') {
        base.set_path(&format!("{}/", base.path()));
    }
    base.join(path.trim_start_matches('/'))
        .map_err(|_| "Invalid TV server address".into())
}

#[cfg(test)]
pub(super) fn parse_guide(value: &str) -> Result<Guide, String> {
    xmltv::read(value.as_bytes(), None)
}

pub(super) use xmltv::read as read_guide;

pub(super) fn http_url(value: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(value.trim()).map_err(|_| "Enter a valid HTTP or HTTPS URL")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || url.fragment().is_some()
    {
        return Err("Use an HTTP or HTTPS URL without a fragment".into());
    }
    Ok(url)
}

fn attributes(value: &str) -> HashMap<String, String> {
    // IPTV attributes are quoted; do not split names or URLs at embedded spaces or commas.
    static ATTR: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| regex::Regex::new(r#"([\w-]+)\s*=\s*"([^"]*)""#).unwrap());
    ATTR.captures_iter(value)
        .map(|c| (c[1].into(), c[2].into()))
        .collect()
}

fn display_separator(value: &str) -> Option<usize> {
    let mut quoted = false;
    for (index, c) in value.char_indices() {
        if c == '"' {
            quoted = !quoted;
        }
        if c == ',' && !quoted {
            return Some(index);
        }
    }
    None
}

#[cfg(test)]
pub(super) fn m3u(
    value: &str,
    base: &str,
) -> Result<Vec<Channel>, String> {
    read_m3u(value.as_bytes(), base)
}

#[cfg(test)]
pub(super) fn read_m3u(
    input: impl std::io::BufRead,
    base: &str,
) -> Result<Vec<Channel>, String> {
    read_playlist(input, base, false, false).map(|p| p.channels)
}

pub(super) struct Playlist {
    pub channels: Vec<Channel>,
    pub identities: HashMap<String, String>,
}

/// Preserve provider identity separately from guide IDs and optional stable FastChannels IDs.
pub(super) fn read_playlist(
    mut input: impl std::io::BufRead,
    base: &str,
    stable_ids: bool,
    allow_empty: bool,
) -> Result<Playlist, String> {
    let base = http_url(base)?;
    let mut rows = Vec::new();
    let mut identities = HashMap::new();
    let mut pending: Option<(HashMap<String, String>, String)> = None;
    let mut seen = HashSet::new();
    let mut line = String::new();
    loop {
        if input
            .read_line(&mut line)
            .map_err(|_| "Unable to read the channel playlist")?
            == 0
        {
            return Err("The channel source is not an extended M3U playlist".into());
        }
        let header = line.trim_start_matches('\u{feff}').trim();
        if !header.is_empty() {
            break;
        }
        line.clear();
    }
    if !line
        .trim_start_matches('\u{feff}')
        .trim_start()
        .starts_with("#EXTM3U")
    {
        return Err("The channel source is not an extended M3U playlist".into());
    }
    loop {
        line.clear();
        if input
            .read_line(&mut line)
            .map_err(|_| "Unable to read the channel playlist")?
            == 0
        {
            break;
        }
        let line = line.trim();
        if line.starts_with("#EXT-X-TARGETDURATION") || line.starts_with("#EXT-X-STREAM-INF") {
            return Err("Enter a channel playlist, rather than an individual HLS stream".into());
        }
        if let Some(info) = line.strip_prefix("#EXTINF:") {
            let split = display_separator(info).ok_or("A channel entry has no display name")?;
            pending = Some((
                attributes(&info[..split]),
                info[split + 1..].trim().to_owned(),
            ));
        } else if !line.is_empty() && !line.starts_with('#') {
            let Some((attrs, name)) = pending.take() else {
                continue;
            };
            let url = base
                .join(line)
                .map_err(|_| "A channel has an invalid stream URL")?;
            http_url(url.as_str())?;
            let identity = attrs.get("channel-id").cloned().unwrap_or_default();
            let guide_key = attrs
                .get("tvg-id")
                .cloned()
                .or_else(|| stable_ids.then(|| identity.clone()))
                .unwrap_or_default();
            // The URL distinguishes multiple qualities with the same EPG ID.
            let key = if stable_ids && !identity.is_empty() {
                format!("fastchannels:{identity}")
            } else if guide_key.is_empty() {
                format!(
                    "stream:{}",
                    Sha256::digest(url.as_str().as_bytes())
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>()
                )
            } else {
                format!("{guide_key}:{}", attrs.get("tvg-chno").unwrap_or(&name))
            };
            if !seen.insert(key.clone()) {
                continue;
            }
            identities.insert(key.clone(), identity);
            let mut channel = Channel::new(key, name, url.to_string());
            channel.guide_key = guide_key;
            channel.number = attrs.get("tvg-chno").cloned().unwrap_or_default();
            channel.group_name = attrs.get("group-title").cloned().unwrap_or_default();
            channel.logo = attrs
                .get("tvg-logo")
                .filter(|v| http_url(v).is_ok())
                .cloned();
            rows.push(channel);
        }
    }
    if rows.is_empty() && !allow_empty {
        return Err("No playable channels were found in the playlist".into());
    }
    Ok(Playlist {
        channels: rows,
        identities,
    })
}

pub(super) fn xml_time(value: &str) -> Option<i64> {
    // XMLTV permits an omitted zone; the standard default is UTC, not server local time.
    let parts = value.split_whitespace().collect::<Vec<_>>();
    let stamp = *parts.first()?;
    let zone = parts.get(1).copied().unwrap_or("+0000");
    let format = match stamp.len() {
        14 => "%Y%m%d%H%M%S %z",
        12 => "%Y%m%d%H%M %z",
        _ => return None,
    };
    chrono::DateTime::parse_from_str(&format!("{stamp} {zone}"), format)
        .ok()
        .map(|time| time.timestamp())
}

pub(super) fn child_text(
    node: roxmltree::Node<'_, '_>,
    tag: &str,
) -> Option<String> {
    node.children()
        .find(|c| c.has_tag_name(tag))
        .and_then(|c| c.text())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}
