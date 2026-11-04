//! IPTV provider identities, shared visibility and appearance, kept separate from channel/guide
//! IDs.
use super::{
    Channel,
    Source,
    setup,
    sources,
    storage,
};
use crate::db::schema::{
    app_settings as a,
    live_tv_channels as c,
};
use base64::{
    Engine,
    engine::general_purpose::STANDARD,
};
use diesel::prelude::*;
use rocket_okapi::okapi::schemars::{
    self,
    JsonSchema,
};
use serde::{
    Deserialize,
    Serialize,
};
use std::collections::{
    BTreeMap,
    HashMap,
};

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct ChannelIdentity {
    pub provider: String,
    pub bridge: bool,
    pub identity: String,
}
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct Appearance {
    pub name: Option<String>,
    pub hidden: bool,
    pub image: Option<String>,
    pub icon_channel_id: Option<i32>,
}
#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct Metadata {
    pub channels: HashMap<String, ChannelIdentity>,
    pub providers: BTreeMap<String, Appearance>,
    pub image: Option<String>,
}
impl Metadata {
    pub(super) fn image_url(
        &self,
        id: i32,
        provider: Option<&str>,
    ) -> Option<String> {
        let version = if let Some(provider) = provider {
            self.providers.get(provider)?.image.as_ref()?
        } else {
            self.image.as_ref()?
        };
        let mut url = format!("/api/v1/live-tv/sources/{id}/image?v={version}");
        if let Some(provider) = provider {
            let mut params = reqwest::Url::parse("http://localhost").unwrap();
            params.query_pairs_mut().append_pair("provider", provider);
            url.push('&');
            url.push_str(params.query().unwrap());
        }
        Some(url)
    }
}
pub(super) fn load(
    conn: &mut SqliteConnection,
    id: i32,
) -> QueryResult<Metadata> {
    setup::load(conn, &format!("live_tv_metadata:{id}"))
}
pub(super) fn save(
    conn: &mut SqliteConnection,
    id: i32,
    data: &Metadata,
) -> QueryResult<()> {
    setup::save(conn, &format!("live_tv_metadata:{id}"), data)
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct ProviderSummary {
    pub source_id: i32,
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub channel_count: usize,
    pub image_url: Option<String>,
    pub icon_channel_id: Option<i32>,
    pub has_custom_icon: bool,
}
#[derive(Serialize, JsonSchema)]
pub(crate) struct PublicChannel {
    #[serde(flatten)]
    pub channel: Channel,
    pub provider_id: Option<String>,
}

pub(crate) fn friendly_name(id: &str) -> String {
    let name = match id {
        "adultswim" => "Adult Swim",
        "amazon" | "prime" | "amazon_prime_free" => "Amazon Prime Free Channels",
        "bally" | "ballysports" => "Bally Sports Live",
        "cbs" | "cbs_sports" => "CBS Sports",
        "espn" => "ESPN",
        "foxone" | "fox_one" => "FOX One",
        "lg" | "lg-channels" => "LG Channels",
        "localnow" => "Local Now",
        "freelivesports" => "Free Live Sports",
        "nbc" => "NBC",
        "nfl" => "NFL",
        "plex" => "Plex",
        "pluto" => "Pluto TV",
        "roku" => "The Roku Channel",
        "samsung" => "Samsung TV Plus",
        "sling" => "Sling",
        "reelz_tve" => "REELZ",
        "stirr" => "STIRR",
        "tcl" => "TCL TV+",
        "tubi" => "Tubi",
        "vizio" => "Vizio WatchFree+",
        "whale" => "Whale TV+",
        "wsn" => "Women's Sports Network",
        "xumo" => "Xumo Play",
        "other" => "Other channels",
        _ => {
            return id
                .split(['_', '-'])
                .filter(|s| !s.is_empty())
                .map(|s| {
                    let mut chars = s.chars();
                    chars
                        .next()
                        .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join(" ");
        }
    };
    name.into()
}
pub(super) fn grouped(source: &Source) -> bool {
    source.kind == "m3u" && sources::config(source).is_ok_and(|c| c.provider_grouping)
}
pub(super) fn allowed(
    source: &Source,
    channel: &Channel,
    data: &Metadata,
) -> bool {
    !grouped(source)
        || data
            .channels
            .get(&channel.source_key)
            .and_then(|c| data.providers.get(&c.provider))
            .is_none_or(|p| !p.hidden)
}
pub(crate) fn decorate(
    conn: &mut SqliteConnection,
    rows: Vec<Channel>,
    admin: bool,
) -> QueryResult<Vec<PublicChannel>> {
    let sources = storage::sources(conn)?
        .into_iter()
        .map(|s| (s.id, s))
        .collect::<HashMap<_, _>>();
    let metadata = sources
        .keys()
        .map(|id| Ok((*id, load(conn, *id)?)))
        .collect::<QueryResult<HashMap<_, _>>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|channel| {
            let source = sources.get(&channel.source_id)?;
            let data = metadata.get(&source.id)?;
            if !admin && !allowed(source, &channel, data) {
                return None;
            }
            let provider_id = grouped(source)
                .then(|| {
                    data.channels
                        .get(&channel.source_key)
                        .map(|c| c.provider.clone())
                })
                .flatten();
            Some(PublicChannel {
                channel,
                provider_id,
            })
        })
        .collect())
}
pub(super) fn summaries(
    conn: &mut SqliteConnection,
    source: &Source,
    data: &Metadata,
) -> QueryResult<Vec<ProviderSummary>> {
    if !grouped(source) {
        return Ok(vec![]);
    }
    let channels = c::table
        .filter(c::source_id.eq(source.id))
        .filter(c::available.eq(true))
        .select(Channel::as_select())
        .load::<Channel>(conn)?;
    let mut counts = BTreeMap::new();
    for channel in &channels {
        if let Some(info) = data.channels.get(&channel.source_key) {
            *counts.entry(info.provider.clone()).or_insert(0) += 1;
        }
    }
    Ok(counts
        .into_iter()
        .map(|(id, count)| {
            let settings = data.providers.get(&id).cloned().unwrap_or_default();
            let selected = settings
                .icon_channel_id
                .and_then(|id| channels.iter().find(|c| c.id == id))
                .and_then(|c| c.logo.clone());
            ProviderSummary {
                source_id: source.id,
                name: settings.name.unwrap_or_else(|| friendly_name(&id)),
                enabled: !settings.hidden,
                channel_count: count,
                image_url: selected.or_else(|| data.image_url(source.id, Some(&id))),
                icon_channel_id: settings.icon_channel_id,
                has_custom_icon: settings.image.is_some(),
                id,
            }
        })
        .collect())
}
pub(crate) fn all_summaries(conn: &mut SqliteConnection) -> QueryResult<Vec<ProviderSummary>> {
    let mut result = Vec::new();
    for source in storage::sources(conn)?.into_iter().filter(|s| s.enabled) {
        let data = load(conn, source.id)?;
        result.extend(summaries(conn, &source, &data)?);
    }
    Ok(result)
}
#[derive(Deserialize, JsonSchema)]
pub(crate) struct ProviderUpdate {
    pub name: String,
    pub enabled: bool,
    pub icon_channel_id: Option<i32>,
    #[serde(default)]
    pub remove_icon: bool,
}
pub(crate) fn update(
    conn: &mut SqliteConnection,
    id: i32,
    provider: &str,
    input: ProviderUpdate,
) -> Result<(), String> {
    let _source = storage::source(conn, id).map_err(super::db_error)?;
    let data = load(conn, id).map_err(super::db_error)?;
    if !data.channels.values().any(|c| c.provider == provider) {
        return Err("Provider not found".into());
    }
    let name = input.name.trim();
    if name.is_empty() || name.len() > 200 {
        return Err("Enter a provider name up to 200 characters".into());
    }
    if let Some(channel) = input.icon_channel_id {
        let channel = storage::channel(conn, channel).map_err(super::db_error)?;
        let existing =
            data.providers.get(provider).and_then(|p| p.icon_channel_id) == Some(channel.id);
        if channel.source_id != id
            || (!existing
                && data
                    .channels
                    .get(&channel.source_key)
                    .is_none_or(|c| c.provider != provider))
        {
            return Err("Choose an icon from this provider's channels".into());
        }
    }
    conn.immediate_transaction::<_, diesel::result::Error, _>(|conn| {
        // Merge into the latest metadata under the write lock; a refresh may have completed
        // since validation, or another provider's icon/name may have changed.
        let mut data = load(conn, id)?;
        let settings = data.providers.entry(provider.into()).or_default();
        settings.name = Some(name.into());
        settings.hidden = !input.enabled;
        settings.icon_channel_id = input.icon_channel_id;
        if input.remove_icon || input.icon_channel_id.is_some() {
            settings.image = None;
        }
        if input.remove_icon || input.icon_channel_id.is_some() {
            diesel::delete(a::table.find(image_key(id, Some(provider)))).execute(conn)?;
        }
        save(conn, id, &data)
    })
    .map_err(super::db_error)
}

pub(crate) fn prepare_image(bytes: &[u8]) -> Result<String, String> {
    if bytes.is_empty() || bytes.len() > 5 * 1024 * 1024 {
        return Err("Choose an image smaller than 5 MiB".into());
    }
    let format =
        image::guess_format(bytes).map_err(|_| "Choose a PNG, JPEG, WebP, or GIF image")?;
    if !matches!(
        format,
        image::ImageFormat::Png
            | image::ImageFormat::Jpeg
            | image::ImageFormat::WebP
            | image::ImageFormat::Gif
    ) {
        return Err("Choose a PNG, JPEG, WebP, or GIF image".into());
    }
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| "Unable to read this image")?
        .thumbnail(256, 256);
    let mut output = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|_| "Unable to save this image")?;
    Ok(STANDARD.encode(output.into_inner()))
}
fn image_key(
    id: i32,
    provider: Option<&str>,
) -> String {
    format!("live_tv_image:{id}:{}", provider.unwrap_or(""))
}
pub(crate) fn set_image(
    conn: &mut SqliteConnection,
    id: i32,
    provider: Option<&str>,
    image: Option<String>,
) -> Result<(), String> {
    storage::source(conn, id).map_err(super::db_error)?;
    conn.immediate_transaction::<_, diesel::result::Error, _>(|conn| {
        let mut data = load(conn, id)?;
        let version = image
            .as_ref()
            .map(|_| format!("{:016x}", rand::random::<u64>()));
        if let Some(provider) = provider {
            if !data.channels.values().any(|c| c.provider == provider) {
                return Err(diesel::result::Error::NotFound);
            }
            let settings = data.providers.entry(provider.into()).or_default();
            settings.image = version;
            settings.icon_channel_id = None;
        } else {
            data.image = version;
        }
        let key = image_key(id, provider);
        if let Some(image) = image {
            setup::save(conn, &key, image)?;
        } else {
            diesel::delete(a::table.find(key)).execute(conn)?;
        }
        save(conn, id, &data)
    })
    .map_err(super::db_error)
}
pub(crate) fn image(
    conn: &mut SqliteConnection,
    id: i32,
    provider: Option<&str>,
) -> Result<Vec<u8>, String> {
    storage::source(conn, id).map_err(super::db_error)?;
    let data = load(conn, id).map_err(super::db_error)?;
    if data.image_url(id, provider).is_none() {
        return Err("Image not found".into());
    }
    let encoded: String = setup::load(conn, &image_key(id, provider)).map_err(super::db_error)?;
    STANDARD
        .decode(encoded)
        .map_err(|_| "Image not found".into())
}

#[cfg(test)]
mod tests;
