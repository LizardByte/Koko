//! Network tuner and native IPTV adapters; connection settings remain private to the server.

mod nextpvr;

use super::formats::{
    Guide,
    Station,
};
use super::{
    Channel,
    Source,
    formats,
    storage,
};
use crate::{
    db::{
        DbConn,
        schema::live_tv_sources as s,
    },
    secrets,
};
use diesel::prelude::*;
use reqwest::{
    Client,
    Url,
};
use rocket_okapi::okapi::schemars::{
    self,
    JsonSchema,
};
use serde::{
    Deserialize,
    Serialize,
};
use std::time::Duration;

#[derive(Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(default)]
pub(crate) struct SourceConfig {
    pub url: String,
    pub pin: String,
    pub xmltv_url: String,
    pub guide_kind: String,
    pub adapter: u32,
    pub frontend: u32,
    pub zap_path: String,
    pub multiplex_urls: Vec<String>,
    #[serde(alias = "gracenote")]
    pub zap2it: super::gracenote::Selection,
    /// Zero selects the provider's recommended interval; Zap2It defaults to daily.
    pub refresh_hours: u32,
    pub native_device: String,
    pub delivery_system: String,
    pub region: String,
    pub tuning_options: String,
}

#[derive(Deserialize, JsonSchema)]
pub(crate) struct SourceInput {
    pub name: String,
    /// hdhomerun, m3u, nextpvr, or tvheadend.
    pub kind: String,
    pub capacity: i32,
    pub config: SourceConfig,
}

pub(crate) async fn nextpvr_artwork(
    db: &crate::db::DbConn,
    id: i32,
) -> Result<(String, Vec<u8>), String> {
    use crate::db::schema::live_tv_programs as programs;
    use diesel::prelude::*;
    let (source, title) = db
        .run(move |conn| {
            let (source_id, title) = programs::table
                .find(id)
                .select((programs::source_id, programs::title))
                .first::<(i32, String)>(conn)?;
            Ok::<_, diesel::result::Error>((storage::source(conn, source_id)?, title))
        })
        .await
        .map_err(super::db_error)?;
    nextpvr_image(source, "name", title).await
}

pub(crate) async fn nextpvr_channel_artwork(
    db: &crate::db::DbConn,
    id: i32,
) -> Result<(String, Vec<u8>), String> {
    use crate::db::schema::live_tv_channels as channels;
    use diesel::prelude::*;
    let (source, key) = db
        .run(move |conn| {
            let (source_id, key) = channels::table
                .find(id)
                .filter(channels::enabled.eq(true).and(channels::available.eq(true)))
                .select((channels::source_id, channels::source_key))
                .first::<(i32, String)>(conn)?;
            Ok::<_, diesel::result::Error>((storage::source(conn, source_id)?, key))
        })
        .await
        .map_err(super::db_error)?;
    nextpvr_image(source, "channel_id", key).await
}

async fn nextpvr_image(
    source: Source,
    parameter: &str,
    value: String,
) -> Result<(String, Vec<u8>), String> {
    if source.kind != "nextpvr" || !source.enabled {
        return Err("Artwork is unavailable".into());
    }
    let config = config(&source)?;
    let sid = nextpvr::login(&config).await?;
    let mut url = formats::endpoint(&config.url, "service")?;
    url.query_pairs_mut()
        .append_pair(
            "method",
            if parameter == "name" { "channel.show.artwork" } else { "channel.icon" },
        )
        .append_pair("sid", &sid)
        .append_pair(parameter, &value)
        .append_pair("prefer", "landscape");
    let mut response = client()?
        .get(url)
        .send()
        .await
        .map_err(|_| "Artwork is unavailable")?;
    if !response.status().is_success() {
        return Err("Artwork is unavailable".into());
    }
    let mime = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .to_owned();
    if !matches!(
        mime.as_str(),
        "image/jpeg" | "image/png" | "image/webp" | "image/gif"
    ) {
        return Err("Unsupported artwork format".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Artwork transfer failed")?
    {
        if bytes.len() + chunk.len() > 8 * 1024 * 1024 {
            return Err("Artwork exceeds the image size limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((mime, bytes))
}

pub(super) fn client() -> Result<Client, String> {
    Client::builder()
        .user_agent(concat!("Koko/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(90))
        .gzip(true)
        .build()
        .map_err(|_| "Unable to create Live TV HTTP client".into())
}

pub(super) async fn bytes(
    request: reqwest::RequestBuilder,
    limit: usize,
) -> Result<Vec<u8>, String> {
    let mut response = request
        .send()
        .await
        .map_err(|_| "Unable to connect to the TV or guide source")?
        .error_for_status()
        .map_err(|_| {
            "The TV or guide source returned an error; check its address and credentials"
        })?;
    if response
        .content_length()
        .is_some_and(|size| size > limit as u64)
    {
        return Err("The TV source response is too large".into());
    }
    let mut data = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "The TV source transfer failed")?
    {
        if data.len() + chunk.len() > limit {
            return Err("The TV source response is too large".into());
        }
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}

pub(super) async fn text(url: Url) -> Result<String, String> {
    let mut data = bytes(client()?.get(url), 64 * 1024 * 1024).await?;
    if data.starts_with(&[0x1f, 0x8b]) {
        use std::io::Read;
        let mut decoded = Vec::new();
        flate2::read::GzDecoder::new(data.as_slice())
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut decoded)
            .map_err(|_| "The compressed guide could not be read")?;
        if decoded.len() > 64 * 1024 * 1024 {
            return Err("The expanded guide exceeds 64 MiB".into());
        }
        data = decoded;
    }
    String::from_utf8(data).map_err(|_| "The TV source must use UTF-8 text".into())
}

pub(super) async fn json(request: reqwest::RequestBuilder) -> Result<serde_json::Value, String> {
    serde_json::from_slice(&bytes(request, 32 * 1024 * 1024).await?)
        .map_err(|_| "The TV source returned invalid JSON".into())
}

pub(crate) fn config(source: &Source) -> Result<SourceConfig, String> {
    if source.config_json.starts_with('{') {
        return serde_json::from_str(&source.config_json)
            .map_err(|_| "The TV source settings could not be read".into());
    }
    let value = secrets::load_secret(&source.config_json)?
        .ok_or("TV source credentials are unavailable; configure the source again")?;
    serde_json::from_str(&value).map_err(|_| "The TV source settings could not be read".into())
}

fn validate(input: &mut SourceInput) -> Result<(), String> {
    if input.name.trim().is_empty() || input.name.len() > 200 || !(1..=64).contains(&input.capacity)
    {
        return Err("Enter a source name and a stream limit between 1 and 64".into());
    }
    if !matches!(
        input.kind.as_str(),
        "hdhomerun" | "m3u" | "nextpvr" | "tvheadend" | "dvb" | "native"
    ) {
        return Err("Select HDHomeRun, IPTV (M3U), a local tuner, NextPVR, or Tvheadend".into());
    }
    if input.kind == "dvb" {
        if !cfg!(target_os = "linux") {
            return Err(
                "DVB channel files require Linux; choose Local tuner for Windows BDA".into(),
            );
        }
        if input.config.url.is_empty() || input.config.adapter > 63 || input.config.frontend > 63 {
            return Err(
                "Enter the DVB channel file path and adapter/frontend numbers from 0 to 63".into(),
            );
        }
        input.capacity = 1;
    } else if input.kind == "native" {
        if !cfg!(any(target_os = "windows", target_os = "linux")) {
            return Err("Direct local tuners are supported on Windows and Linux".into());
        }
        if !matches!(
            input.config.delivery_system.as_str(),
            "" | "ATSC" | "DVB-T" | "DVB-T2" | "ISDB-T"
        ) || input.config.native_device.len() > 1000
            || input.config.region.len() > 100
            || input.config.tuning_options.len() > 2000
        {
            return Err("Select a supported native tuner and terrestrial delivery system".into());
        }
        input.capacity = 1;
    } else {
        formats::http_url(&input.config.url)?;
    }
    if !input.config.xmltv_url.is_empty() {
        formats::http_url(&input.config.xmltv_url)?;
    }
    if !matches!(
        input.config.guide_kind.as_str(),
        "" | "automatic" | "xmltv" | "broadcast" | "gracenote" | "zap2it" | "upload"
    ) {
        return Err("Select a supported guide source".into());
    }
    if input.config.multiplex_urls.len() > 100 {
        return Err("Configure at most 100 broadcast multiplexes".into());
    }
    for url in &input.config.multiplex_urls {
        formats::http_url(url)?;
    }
    if input.config.guide_kind == "xmltv" && input.config.xmltv_url.is_empty() {
        return Err("Enter the XMLTV guide URL".into());
    }
    if input.config.refresh_hours > 168 {
        return Err("Choose a refresh interval from 1 to 168 hours, or zero for automatic".into());
    }
    if matches!(input.config.guide_kind.as_str(), "gracenote" | "zap2it") {
        super::gracenote::validate(&input.config.zap2it)?;
    }
    Ok(())
}

async fn resolve_native_device(
    db: &DbConn,
    input: &mut SourceInput,
    except: Option<i32>,
) -> Result<(), String> {
    if input.kind != "native" {
        return Ok(());
    }
    if input.config.native_device.is_empty() {
        input.config.native_device = super::tsduck::devices()
            .await?
            .first()
            .ok_or("No local tuners were found; check the driver and device permissions")?
            .id
            .clone();
    }
    let identity = |name: &str| {
        if cfg!(windows) {
            name.chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .to_lowercase()
        } else {
            name.to_owned()
        }
    };
    let selected = identity(&input.config.native_device);
    // A manually entered driver name/path remains usable when enumeration cannot find it.
    let sources = db.run(storage::sources).await.map_err(super::db_error)?;
    if sources
        .iter()
        .filter(|s| s.kind == "native" && Some(s.id) != except)
        .filter_map(|s| config(s).ok())
        .any(|c| identity(&c.native_device) == selected)
    {
        return Err(
            "This tuner is already configured. Edit its existing source or select another tuner"
                .into(),
        );
    }
    Ok(())
}

pub(crate) async fn save_source(
    db: &DbConn,
    mut input: SourceInput,
) -> Result<i32, String> {
    let setup_lock = source_lock(0);
    let _setup_guard = setup_lock.lock().await;
    validate(&mut input)?;
    resolve_native_device(db, &mut input, None).await?;
    // Persist settings with the source so headless builds do not lose guide/tuner setup on restart.
    let encoded = serde_json::to_string(&input.config).map_err(|_| "Unable to encode TV source")?;
    db.run(move |conn| {
        conn.transaction(|conn| {
            diesel::insert_into(s::table)
                .values((
                    s::name.eq(input.name.trim()),
                    s::kind.eq(input.kind),
                    s::config_json.eq(&encoded),
                    s::capacity.eq(input.capacity),
                ))
                .execute(conn)?;
            s::table
                .order(s::id.desc())
                .select(s::id)
                .first::<i32>(conn)
        })
    })
    .await
    .map_err(super::db_error)
}

pub(crate) async fn replace_source(
    db: &DbConn,
    id: i32,
    mut input: SourceInput,
) -> Result<(), String> {
    let setup_lock = source_lock(0);
    let _setup_guard = setup_lock.lock().await;
    let lock = source_lock(id);
    let _guard = lock.lock().await;
    validate(&mut input)?;
    resolve_native_device(db, &mut input, Some(id)).await?;
    let previous = db
        .run(move |conn| storage::source(conn, id))
        .await
        .map_err(super::db_error)?;
    if input.kind != previous.kind {
        return Err("Add a new source to use a different tuner type".into());
    }
    let encoded =
        serde_json::to_string(&input.config).map_err(|_| "Unable to encode source settings")?;
    let result = db
        .run(move |conn| {
            diesel::update(s::table.find(id))
                .set((
                    s::name.eq(input.name.trim()),
                    s::config_json.eq(encoded),
                    s::capacity.eq(input.capacity),
                    s::next_refresh.eq(0_i64),
                    s::last_error.eq(None::<String>),
                ))
                .execute(conn)
        })
        .await
        .map_err(super::db_error);
    if result.is_ok() && !previous.config_json.starts_with('{') {
        let _ = secrets::delete_secret(&previous.config_json);
    }
    result.map(|_| ())
}

async fn hdhomerun(source: &SourceConfig) -> Result<Vec<Channel>, String> {
    let base = formats::http_url(&source.url)?;
    let value =
        json(client()?.get(base.join("/lineup.json").map_err(|_| "Invalid tuner URL")?)).await?;
    let rows = value.as_array().ok_or("Invalid HDHomeRun channel lineup")?;
    let mut channels = Vec::new();
    for row in rows {
        let Some(url) = row["URL"].as_str() else {
            continue;
        };
        formats::http_url(url)?;
        // Copy-protected channels cannot be played or captured by Koko.
        if row["DRM"].as_i64().unwrap_or(0) != 0 || row["DRM"].as_bool().unwrap_or(false) {
            continue;
        }
        let number = row["GuideNumber"].as_str().unwrap_or("").to_owned();
        let mut channel = Channel::new(
            number.clone(),
            row["GuideName"].as_str().unwrap_or(&number).into(),
            url.into(),
        );
        channel.number = number;
        channels.push(channel);
    }
    if channels.is_empty() {
        return Err(
            "No unencrypted channels were found; scan channels in HDHomeRun setup first".into(),
        );
    }
    Ok(channels)
}

async fn automatic_guide(
    source: &Source,
    config: &SourceConfig,
) -> Result<Option<Guide>, String> {
    let parsed = match source.kind.as_str() {
        "hdhomerun" => {
            let base = formats::http_url(&config.url)?;
            let discover = json(
                client()?.get(
                    base.join("/discover.json")
                        .map_err(|_| "Invalid tuner URL")?,
                ),
            )
            .await?;
            let auth = discover["DeviceAuth"]
                .as_str()
                .ok_or("The HDHomeRun did not supply guide credentials")?;
            let mut url = formats::http_url("https://api.hdhomerun.com/api/xmltv")?;
            url.query_pairs_mut().append_pair("DeviceAuth", auth);
            formats::parse_guide(&text(url).await?)
        }
        "nextpvr" => nextpvr::guide(config).await.map(from_programs),
        "tvheadend" => {
            formats::parse_guide(&text(formats::endpoint(&config.url, "xmltv/channels")?).await?)
        }
        _ if !config.xmltv_url.is_empty() => {
            formats::parse_guide(&text(formats::http_url(&config.xmltv_url)?).await?)
        }
        _ => return Ok(None),
    }?;
    Ok(Some(parsed))
}

fn from_programs(programs: Vec<super::Program>) -> Guide {
    let mut keys = std::collections::HashSet::new();
    let stations = programs
        .iter()
        .filter(|p| keys.insert(p.guide_key.clone()))
        .map(|p| Station {
            source_id: 0,
            guide_key: p.guide_key.clone(),
            name: p.guide_key.clone(),
            number: p.guide_key.clone(),
            aliases: vec![],
            logo: None,
        })
        .collect();
    Guide { programs, stations }
}

static SOURCE_LOCKS: once_cell::sync::Lazy<
    std::sync::Mutex<std::collections::HashMap<i32, std::sync::Weak<tokio::sync::Mutex<()>>>>,
> = once_cell::sync::Lazy::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

pub(super) fn source_lock(id: i32) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    let mut locks = SOURCE_LOCKS.lock().expect("Live TV source lock poisoned");
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(&id).and_then(std::sync::Weak::upgrade) {
        return lock;
    }
    let lock = std::sync::Arc::new(tokio::sync::Mutex::new(()));
    locks.insert(id, std::sync::Arc::downgrade(&lock));
    lock
}

pub(crate) async fn refresh(
    db: &DbConn,
    id: i32,
) -> Result<(), String> {
    let lock = source_lock(id);
    let _guard = lock.lock().await;
    let source = db
        .run(move |conn| storage::source(conn, id))
        .await
        .map_err(super::db_error)?;
    if !source.enabled {
        return Err("This TV source is disabled".into());
    }
    let result = refresh_inner(db, &source).await;
    let error = result.as_ref().err().cloned();
    // SiliconDust requests randomized refresh intervals of 20–28 hours.
    let delay = if result.is_err() {
        3600
    } else if let Ok(settings) = config(&source) {
        if source.kind == "hdhomerun" && matches!(settings.guide_kind.as_str(), "automatic" | "") {
            rand::random_range(20 * 3600..28 * 3600)
        } else if settings.refresh_hours > 0 {
            i64::from(settings.refresh_hours) * 3600
        } else if matches!(settings.guide_kind.as_str(), "gracenote" | "zap2it") {
            24 * 3600
        } else {
            6 * 3600
        }
    } else {
        6 * 3600
    };
    db.run(move |conn| {
        diesel::update(s::table.find(id))
            .set((
                s::next_refresh.eq(super::now() + delay),
                s::last_error.eq(error),
            ))
            .execute(conn)
    })
    .await
    .map_err(super::db_error)?;
    result
}

async fn refresh_inner(
    db: &DbConn,
    source: &Source,
) -> Result<(), String> {
    let config = config(source)?;
    let rows = match source.kind.as_str() {
        "hdhomerun" => hdhomerun(&config).await?,
        "nextpvr" => nextpvr::channels(&config).await?,
        "dvb" => super::native::channels(&config).await?,
        "native" => super::tsduck::channels(source, &config).await?,
        "tvheadend" => {
            let url = formats::endpoint(&config.url, "playlist/channels.m3u")?;
            formats::m3u(&text(url.clone()).await?, url.as_str())?
        }
        _ => formats::m3u(&text(formats::http_url(&config.url)?).await?, &config.url)?,
    };
    let id = source.id;
    db.run(move |conn| storage::save_channels(conn, id, &rows))
        .await
        .map_err(super::db_error)?;
    let guide = match config.guide_kind.as_str() {
        "gracenote" | "zap2it" => Some(super::gracenote::guide(&config.zap2it).await?),
        "xmltv" => Some(formats::parse_guide(
            &text(formats::http_url(&config.xmltv_url)?).await?,
        )?),
        "upload" => None,
        "broadcast" => Some(super::broadcast::collect(db, source, &config).await?),
        "automatic" | "" if matches!(source.kind.as_str(), "dvb" | "native") => {
            Some(super::broadcast::collect(db, source, &config).await?)
        }
        _ => automatic_guide(source, &config).await?,
    };
    let broadcast = config.guide_kind == "broadcast"
        || (matches!(source.kind.as_str(), "dvb" | "native")
            && matches!(config.guide_kind.as_str(), "" | "automatic"));
    if let Some(guide) = guide {
        db.run(move |conn| {
            if broadcast {
                storage::merge_guide(conn, id, guide)
            } else {
                storage::save_guide(conn, id, &guide)
            }
        })
        .await
        .map_err(super::db_error)?;
    }
    Ok(())
}

pub(crate) async fn stream_url(
    source: &Source,
    channel: &Channel,
) -> Result<String, String> {
    if source.kind == "nextpvr" {
        nextpvr::stream_url(&config(source)?, &channel.source_key).await
    } else {
        formats::http_url(&channel.stream_url).map(|url| url.to_string())
    }
}

pub(super) fn keepalive(
    source: &Source,
    url: &str,
) -> Result<Option<tokio::task::JoinHandle<()>>, String> {
    if source.kind == "nextpvr" {
        nextpvr::keepalive(&config(source)?, url).map(Some)
    } else {
        Ok(None)
    }
}
