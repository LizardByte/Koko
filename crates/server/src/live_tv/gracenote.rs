//! Postal-code provider discovery and bounded six-hour paging of Zap2It's public TV grid.

use super::formats::{
    Guide,
    Program,
    ProgramMetadata,
    Station,
};
use super::sources::client;
use reqwest::Url;
use rocket_okapi::okapi::schemars::{
    self,
    JsonSchema,
};
use serde::{
    Deserialize,
    Serialize,
};
use serde_json::Value;
use std::collections::{
    HashMap,
    HashSet,
};

const BASE: &str = "https://tvlistings.gracenote.com";
const PAGE_SECONDS: i64 = 6 * 3600;

async fn request(
    client: &reqwest::Client,
    url: Url,
) -> Result<Value, String> {
    let mut response = client
        .get(url)
        .header(
            reqwest::header::USER_AGENT,
            concat!(
                "Mozilla/5.0 (compatible; Koko/",
                env!("CARGO_PKG_VERSION"),
                "; +https://github.com/LizardByte/Koko)"
            ),
        )
        .header(reqwest::header::ACCEPT, "application/json")
        .header(
            reqwest::header::REFERER,
            "https://tvlistings.gracenote.com/",
        )
        .send()
        .await
        .map_err(|_| {
            "Unable to reach Zap2It; check the server's network connection and try again"
        })?;
    if !response.status().is_success() {
        return Err(format!(
            "Zap2It could not load listings (HTTP {}). Try again later or use an XMLTV guide",
            response.status().as_u16()
        ));
    }
    let limit = 32 * 1024 * 1024;
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err("The Zap2It response is too large".into());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "The Zap2It transfer was interrupted; try again")?
    {
        if body.len() + chunk.len() > limit {
            return Err("The Zap2It response is too large".into());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body)
        .map_err(|_| "Zap2It returned an unsupported response; try again later".into())
}

#[derive(Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(default)]
pub(crate) struct Selection {
    pub country: String,
    pub postal_code: String,
    pub lineup_id: String,
    pub headend_id: String,
    pub device: String,
    pub timezone: String,
    /// Number of days to fetch, from one to fourteen; zero selects fourteen.
    pub days: u32,
}

#[derive(Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Provider {
    #[serde(rename = "type")]
    pub kind: String,
    pub device: String,
    pub lineup_id: String,
    pub name: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub timezone: String,
    pub headend_id: String,
}

fn postal(
    country: &str,
    value: &str,
) -> Result<String, String> {
    let value = value.trim().to_ascii_uppercase().replace(' ', "");
    let valid = match country {
        "USA" => value.len() == 5 && value.bytes().all(|b| b.is_ascii_digit()),
        "CAN" => {
            value.len() == 6
                && value.bytes().enumerate().all(|(i, b)| {
                    if i % 2 == 0 { b.is_ascii_alphabetic() } else { b.is_ascii_digit() }
                })
        }
        _ => false,
    };
    if !valid {
        return Err(
            "Choose USA with a five-digit ZIP code or CAN with a Canadian postal code".into(),
        );
    }
    Ok(value)
}

pub(crate) fn validate(selection: &Selection) -> Result<(), String> {
    postal(&selection.country, &selection.postal_code)?;
    for value in [
        &selection.lineup_id,
        &selection.headend_id,
    ] {
        if value.is_empty()
            || value.len() > 150
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err("Search for and select a Zap2It lineup".into());
        }
    }
    if selection.device.len() > 20 || selection.timezone.len() > 100 || selection.days > 14 {
        return Err("Select a valid lineup and a guide horizon of at most fourteen days".into());
    }
    Ok(())
}

pub(crate) async fn providers(
    country: &str,
    code: &str,
) -> Result<Vec<Provider>, String> {
    let code = postal(country, code)?;
    let mut url = Url::parse(BASE).unwrap();
    url.path_segments_mut().unwrap().extend([
        "gapzap_webapi",
        "api",
        "Providers",
        "getPostalCodeProviders",
        country,
        &code,
        "cha",
        "en",
    ]);
    let value = request(&client()?, url).await?;
    serde_json::from_value(
        value
            .get("Providers")
            .cloned()
            .ok_or("Zap2It returned no provider list")?,
    )
    .map_err(|_| "Zap2It returned an unsupported provider response".into())
}

pub(super) fn page_url(
    selection: &Selection,
    time: i64,
) -> Result<Url, String> {
    validate(selection)?;
    let mut url = Url::parse(BASE).unwrap().join("/api/grid").unwrap();
    url.query_pairs_mut().extend_pairs([
        ("lineupId", selection.lineup_id.clone()),
        ("timespan", "6".into()),
        ("headendId", selection.headend_id.clone()),
        ("country", selection.country.clone()),
        ("timezone", selection.timezone.clone()),
        (
            "device",
            if selection.device.is_empty() { "-".into() } else { selection.device.clone() },
        ),
        (
            "postalCode",
            postal(&selection.country, &selection.postal_code)?,
        ),
        ("isOverride", "true".into()),
        ("time", time.to_string()),
        ("pref", "16,128".into()),
        ("userId", "-".into()),
        ("aid", "cha".into()),
        ("languagecode", "en".into()),
    ]);
    Ok(url)
}

fn field(
    value: &Value,
    key: &str,
) -> Option<String> {
    value
        .get(key)
        .and_then(|v| {
            v.as_str()
                .map(str::to_owned)
                .or_else(|| v.as_i64().map(|v| v.to_string()))
        })
        .filter(|s| !s.is_empty() && s != "null")
}
fn image(value: &str) -> Option<String> {
    let url = if value.starts_with("//") {
        format!("https:{value}")
    } else if value.starts_with("http") {
        value.into()
    } else if value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        format!("https://zpmc.tmsimg.com/assets/{value}.jpg")
    } else {
        return None;
    };
    super::formats::http_url(&url).ok().map(|u| u.to_string())
}

pub(super) fn parse(value: &Value) -> Result<Guide, String> {
    let rows = value["channels"]
        .as_array()
        .ok_or("Zap2It returned an unsupported grid response")?;
    if rows.len() > 20_000 {
        return Err("The lineup exceeds 20,000 stations".into());
    }
    let mut stations = Vec::new();
    let mut programs = Vec::new();
    for channel in rows {
        // channelId identifies the station; id identifies a station/number row in this grid.
        let Some(key) = field(channel, "channelId") else {
            continue;
        };
        let name = field(channel, "callSign").unwrap_or_else(|| key.clone());
        let number = field(channel, "channelNo").unwrap_or_default();
        stations.push(Station {
            source_id: 0,
            guide_key: key.clone(),
            name: name.clone(),
            number: number.clone(),
            aliases: vec![name, number],
            logo: field(channel, "thumbnail").as_deref().and_then(image),
        });
        for event in channel["events"].as_array().into_iter().flatten() {
            let timestamp = |key| {
                field(event, key)
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|t| t.timestamp())
            };
            let (Some(start), Some(end), Some(title)) = (
                timestamp("startTime"),
                timestamp("endTime"),
                field(&event["program"], "title"),
            ) else {
                continue;
            };
            if end <= start {
                continue;
            }
            let details = &event["program"];
            let season = field(details, "season");
            let episode = field(details, "episode");
            let episode_id = field(details, "tmsId")
                .or_else(|| field(details, "id"))
                .filter(|s| s.starts_with("EP") || s.starts_with("MV"));
            let flags = event["flag"].as_array();
            let metadata = ProgramMetadata {
                subtitle: field(details, "episodeTitle"),
                description: field(details, "shortDesc").or_else(|| field(details, "description")),
                categories: event["filter"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .filter_map(|s| s.strip_prefix("filter-"))
                    .map(str::to_owned)
                    .collect(),
                image: field(event, "thumbnail").as_deref().and_then(image),
                rating: field(event, "rating"),
                episode: episode.map(|e| match season {
                    Some(s) => format!("S{s}E{e}"),
                    None => format!("Episode {e}"),
                }),
                episode_id,
                is_new: flags.map(|flags| {
                    flags
                        .iter()
                        .any(|f| f.as_str().is_some_and(|s| s.eq_ignore_ascii_case("New")))
                }),
                ..Default::default()
            };
            programs.push(Program {
                id: 0,
                source_id: 0,
                guide_key: key.clone(),
                starts_at: start,
                ends_at: end,
                title,
                metadata,
            });
        }
    }
    Ok(Guide { stations, programs })
}

pub(super) async fn guide(selection: &Selection) -> Result<Guide, String> {
    let start = super::now().div_euclid(3600) * 3600;
    let end = start + i64::from(if selection.days == 0 { 14 } else { selection.days }) * 86400;
    let client = client()?;
    let mut stations: HashMap<String, Station> = HashMap::new();
    let mut programs = HashMap::new();
    let mut pages = HashSet::new();
    let mut time = start;
    while time < end {
        let page = parse(&request(&client, page_url(selection, time)?).await?)?;
        // An ignored time parameter must not repeatedly download the same grid for fourteen days.
        let signature = page.programs.iter().map(|p| p.starts_at).max();
        if let Some(signature) = signature {
            if !pages.insert(signature) {
                break;
            }
        } else {
            break;
        }
        for station in page.stations {
            let existing = stations
                .entry(station.guide_key.clone())
                .or_insert_with(|| station.clone());
            existing.aliases.extend(station.aliases);
            existing.aliases.sort();
            existing.aliases.dedup();
        }
        for program in page.programs {
            if program.ends_at > start && program.starts_at < end {
                programs.insert((program.guide_key.clone(), program.starts_at), program);
            }
        }
        if programs.len() > 250_000 {
            return Err(
                "The Zap2It guide exceeds 250,000 programs; choose fewer guide days".into(),
            );
        }
        time += PAGE_SECONDS;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    if programs.is_empty() {
        return Err("Zap2It returned no programs for this lineup".into());
    }
    Ok(Guide {
        stations: stations.into_values().collect(),
        programs: programs.into_values().collect(),
    })
}
