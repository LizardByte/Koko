//! Bounded playlist and XMLTV parsing without resolving external XML entities.

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

pub(super) fn parse_guide(value: &str) -> Result<Guide, String> {
    let programs = xmltv(value)?;
    let doc = roxmltree::Document::parse_with_options(
        value,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            nodes_limit: 3_000_000,
            ..Default::default()
        },
    )
    .map_err(|_| "Invalid XMLTV guide")?;
    let mut stations = Vec::new();
    for node in doc
        .root_element()
        .children()
        .filter(|n| n.has_tag_name("channel"))
    {
        let Some(key) = node.attribute("id") else {
            continue;
        };
        let aliases = node
            .children()
            .filter(|n| n.has_tag_name("display-name"))
            .filter_map(|n| n.text().map(str::to_owned))
            .collect::<Vec<_>>();
        let number = child_text(node, "lcn")
            .or_else(|| {
                aliases
                    .iter()
                    .find(|s| {
                        s.chars()
                            .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
                    })
                    .cloned()
            })
            .unwrap_or_default();
        stations.push(Station {
            source_id: 0,
            guide_key: key.into(),
            name: aliases.first().cloned().unwrap_or_else(|| key.into()),
            number,
            aliases,
            logo: node
                .children()
                .find(|n| n.has_tag_name("icon"))
                .and_then(|n| n.attribute("src"))
                .filter(|url| http_url(url).is_ok())
                .map(str::to_owned),
        });
    }
    // Some minimal guides omit channel declarations. Keep their explicit station IDs mappable.
    let mut keys = stations
        .iter()
        .map(|s| s.guide_key.clone())
        .collect::<HashSet<_>>();
    for p in &programs {
        if keys.insert(p.guide_key.clone()) {
            stations.push(Station {
                source_id: 0,
                guide_key: p.guide_key.clone(),
                name: p.guide_key.clone(),
                number: String::new(),
                aliases: vec![],
                logo: None,
            });
        }
    }
    Ok(Guide { programs, stations })
}

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

pub(super) fn m3u(
    value: &str,
    base: &str,
) -> Result<Vec<Channel>, String> {
    let base = http_url(base)?;
    let mut rows = Vec::new();
    let mut pending: Option<(HashMap<String, String>, String)> = None;
    let mut seen = HashSet::new();
    if !value
        .trim_start_matches('\u{feff}')
        .trim_start()
        .starts_with("#EXTM3U")
    {
        return Err("The channel source is not an extended M3U playlist".into());
    }
    if value.lines().any(|line| {
        line.starts_with("#EXT-X-TARGETDURATION") || line.starts_with("#EXT-X-STREAM-INF")
    }) {
        return Err("Enter a channel playlist, rather than an individual HLS stream".into());
    }
    for line in value.lines().map(str::trim) {
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
            let guide_key = attrs.get("tvg-id").cloned().unwrap_or_default();
            // The URL distinguishes multiple qualities with the same EPG ID.
            let key = if guide_key.is_empty() {
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
            let mut channel = Channel::new(key, name, url.to_string());
            channel.guide_key = guide_key;
            channel.number = attrs.get("tvg-chno").cloned().unwrap_or_default();
            channel.group_name = attrs.get("group-title").cloned().unwrap_or_default();
            channel.logo = attrs
                .get("tvg-logo")
                .filter(|v| http_url(v).is_ok())
                .cloned();
            rows.push(channel);
            if rows.len() > 20_000 {
                return Err("The playlist exceeds 20,000 channels".into());
            }
        }
    }
    if rows.is_empty() {
        return Err("No playable channels were found in the playlist".into());
    }
    Ok(rows)
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

pub(super) fn xmltv(value: &str) -> Result<Vec<Program>, String> {
    // Allow the conventional external DOCTYPE declaration; roxmltree never fetches it.
    // Entity declarations are rejected to avoid expansion and unsupported custom entities.
    if value.contains("<!ENTITY") {
        return Err("XMLTV entity declarations are not supported".into());
    }
    let doc = roxmltree::Document::parse_with_options(
        value,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            nodes_limit: 3_000_000,
            ..Default::default()
        },
    )
    .map_err(|_| "The guide is not valid XMLTV")?;
    if !doc.root_element().has_tag_name("tv") {
        return Err("The guide must contain an XMLTV tv root".into());
    }
    let mut rows = Vec::new();
    for node in doc
        .root_element()
        .children()
        .filter(|n| n.has_tag_name("programme"))
    {
        let (Some(key), Some(start)) = (
            node.attribute("channel"),
            node.attribute("start").and_then(xml_time),
        ) else {
            continue;
        };
        let end = node.attribute("stop").and_then(xml_time);
        let Some(title) = child_text(node, "title") else {
            continue;
        };
        let mut metadata = ProgramMetadata {
            subtitle: child_text(node, "sub-title"),
            description: child_text(node, "desc"),
            ..Default::default()
        };
        for child in node.children() {
            match child.tag_name().name() {
                "category" => {
                    if let Some(text) = child.text() {
                        metadata.categories.push(text.to_owned());
                    }
                }
                "icon" => {
                    metadata.image = child
                        .attribute("src")
                        .filter(|s| http_url(s).is_ok())
                        .map(str::to_owned)
                }
                "rating" => metadata.rating = child_text(child, "value"),
                "episode-num" => {
                    let episode = child.text().map(str::to_owned);
                    if child.attribute("system") == Some("onscreen") {
                        metadata.episode = episode;
                    } else if metadata.episode_id.is_none() {
                        metadata.episode_id = episode.map(|id| {
                            format!("{}:{id}", child.attribute("system").unwrap_or("unknown"))
                        });
                    }
                }
                "new" => metadata.is_new = Some(true),
                "premiere" => {
                    metadata.is_premiere = Some(true);
                    metadata.is_new = Some(true);
                }
                "live" => metadata.is_live = Some(true),
                "finale" => metadata.is_finale = Some(true),
                "previously-shown" => metadata.is_new = Some(false),
                "credits" => {
                    metadata.credits = child
                        .children()
                        .filter_map(|c| c.text())
                        .map(str::to_owned)
                        .collect()
                }
                _ => {}
            }
        }
        rows.push(Program {
            id: 0,
            source_id: 0,
            guide_key: key.into(),
            starts_at: start,
            ends_at: end.unwrap_or(0),
            title,
            metadata,
        });
        if rows.len() > 250_000 {
            return Err("The guide exceeds 250,000 programs".into());
        }
    }
    rows.sort_by(|a, b| (&a.guide_key, a.starts_at).cmp(&(&b.guide_key, b.starts_at)));
    // XMLTV stop is optional. Infer it only from the next event on the same station.
    for index in 0..rows.len() {
        if rows[index].ends_at == 0
            && index + 1 < rows.len()
            && rows[index].guide_key == rows[index + 1].guide_key
        {
            rows[index].ends_at = rows[index + 1].starts_at;
        }
    }
    rows.retain(|p| p.ends_at > p.starts_at);
    rows.dedup_by(|a, b| a.guide_key == b.guide_key && a.starts_at == b.starts_at);
    Ok(rows)
}
