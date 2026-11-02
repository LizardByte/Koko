//! Decode broadcast DVB EIT and ATSC PSIP through TSDuck's standards-aware table decoder.

use super::{
    Source,
    SourceConfig,
    formats::{
        Guide,
        Program,
        ProgramMetadata,
        Station,
        child_text,
    },
};
use crate::db::{
    DbConn,
    schema::{
        live_tv_channels as c,
        live_tv_recordings as r,
    },
};
use diesel::prelude::*;
use std::{
    collections::{
        HashMap,
        HashSet,
    },
    time::Duration,
};
use tokio::io::{
    AsyncReadExt,
    AsyncWriteExt,
};

static SCANS: once_cell::sync::Lazy<
    std::sync::Mutex<HashMap<i32, tokio::sync::watch::Sender<bool>>>,
> = once_cell::sync::Lazy::new(|| std::sync::Mutex::new(HashMap::new()));
pub(super) struct Scan(i32);
impl Drop for Scan {
    fn drop(&mut self) {
        if let Ok(mut scans) = SCANS.lock() {
            scans.remove(&self.0);
        }
    }
}
pub(super) fn scan(id: i32) -> (Scan, tokio::sync::watch::Receiver<bool>) {
    let (sender, receiver) = tokio::sync::watch::channel(false);
    SCANS
        .lock()
        .expect("Broadcast scan lock poisoned")
        .insert(id, sender);
    (Scan(id), receiver)
}
pub(super) fn preempt(id: i32) -> bool {
    SCANS
        .lock()
        .ok()
        .and_then(|scans| scans.get(&id).map(|sender| sender.send(true).is_ok()))
        .unwrap_or(false)
}

async fn decode_idle(
    config: &SourceConfig,
    input: impl tokio::io::AsyncRead + Unpin,
    mut cancel: tokio::sync::watch::Receiver<bool>,
) -> Result<Guide, String> {
    if *cancel.borrow() {
        return Err("Broadcast guide scan yielded its tuner to playback or recording".into());
    }
    tokio::select! {
        _ = cancel.changed() => {
            Err("Broadcast guide scan yielded its tuner to playback or recording".into())
        },
        result=decode(config,input)=>result,
    }
}

fn number(value: Option<&str>) -> Option<u32> {
    let value = value?.replace(',', "");
    if let Some(hex) = value.strip_prefix("0x") {
        u32::from_str_radix(hex, 16).ok()
    } else {
        value.parse().ok()
    }
}
fn stamp(value: Option<&str>) -> Option<i64> {
    chrono::NaiveDateTime::parse_from_str(value?, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|d| d.and_utc().timestamp())
}
fn duration(value: &str) -> Option<i64> {
    let parts = value
        .split(':')
        .map(str::parse::<i64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if parts.len() != 3
        || parts[0] < 0
        || !(0..60).contains(&parts[1])
        || !(0..60).contains(&parts[2])
    {
        return None;
    }
    Some(parts[0] * 3600 + parts[1] * 60 + parts[2])
}
fn multiple_string(
    node: roxmltree::Node<'_, '_>,
    tag: &str,
) -> Option<String> {
    node.children()
        .find(|n| n.has_tag_name(tag))?
        .children()
        .find(|n| n.has_tag_name("string"))?
        .attribute("text")
        .map(str::to_owned)
}

fn station_tables(
    root: roxmltree::Node<'_, '_>
) -> (HashMap<String, Station>, HashMap<u32, String>) {
    let mut stations = HashMap::new();
    let mut descriptions = HashMap::new();
    for table in root.children() {
        if matches!(table.tag_name().name(), "TVCT" | "CVCT") {
            for channel in table.children().filter(|n| n.has_tag_name("channel")) {
                let Some(id) = number(channel.attribute("source_id")) else {
                    continue;
                };
                let key = format!("atsc:{id}");
                let major = number(channel.attribute("major_channel_number")).unwrap_or(0);
                let minor = number(channel.attribute("minor_channel_number")).unwrap_or(0);
                stations.insert(
                    key.clone(),
                    Station {
                        source_id: 0,
                        guide_key: key,
                        name: channel.attribute("short_name").unwrap_or("").trim().into(),
                        number: format!("{major}.{minor}"),
                        aliases: vec![],
                        logo: None,
                    },
                );
            }
        } else if table.has_tag_name("SDT") {
            let tsid = number(table.attribute("transport_stream_id")).unwrap_or(0);
            let network = number(table.attribute("original_network_id")).unwrap_or(0);
            for service in table.children().filter(|n| n.has_tag_name("service")) {
                let Some(id) = number(service.attribute("service_id")) else {
                    continue;
                };
                let key = format!("dvb:{network}:{tsid}:{id}");
                let name = service
                    .children()
                    .find(|n| n.has_tag_name("service_descriptor"))
                    .and_then(|n| n.attribute("service_name"))
                    .unwrap_or("");
                stations.insert(
                    key.clone(),
                    Station {
                        source_id: 0,
                        guide_key: key,
                        name: name.into(),
                        number: String::new(),
                        aliases: vec![],
                        logo: None,
                    },
                );
            }
        } else if table.has_tag_name("ETT") {
            if let (Some(id), Some(text)) = (
                number(table.attribute("ETM_id")),
                multiple_string(table, "extended_text_message"),
            ) {
                descriptions.insert(id, text);
            }
        }
    }
    (stations, descriptions)
}

fn event_description(
    table: roxmltree::Node<'_, '_>,
    event: roxmltree::Node<'_, '_>,
    descriptions: &HashMap<u32, String>,
) -> Option<String> {
    let atsc = table.has_tag_name("ATSC_EIT");
    let short = event
        .children()
        .find(|n| n.has_tag_name("short_event_descriptor"));
    let mut description = None;

    if atsc {
        if let (Some(source), Some(id)) = (
            number(table.attribute("source_id")),
            number(event.attribute("event_id")),
        ) {
            description = descriptions.get(&((source << 16) | (id << 2) | 2)).cloned();
        }
    } else {
        let mut text = short
            .and_then(|n| child_text(n, "text"))
            .unwrap_or_default();
        let mut extended = event
            .children()
            .filter(|n| n.has_tag_name("extended_event_descriptor"))
            .collect::<Vec<_>>();
        extended.sort_by_key(|n| number(n.attribute("descriptor_number")).unwrap_or(0));
        for node in extended {
            if let Some(extra) = child_text(node, "text") {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&extra);
            }
        }
        description = (!text.is_empty()).then_some(text);
    }
    description
}

fn event_program(
    table: roxmltree::Node<'_, '_>,
    event: roxmltree::Node<'_, '_>,
    key: &str,
    descriptions: &HashMap<u32, String>,
) -> Option<Program> {
    let atsc = table.has_tag_name("ATSC_EIT");
    let start = stamp(event.attribute("start_time"))?;
    let length = if atsc {
        number(event.attribute("length_in_seconds")).map(i64::from)
    } else {
        event.attribute("duration").and_then(duration)
    }
    .unwrap_or(0);
    if length <= 0 {
        return None;
    }
    let short = event
        .children()
        .find(|n| n.has_tag_name("short_event_descriptor"));
    let title = if atsc {
        multiple_string(event, "title_text")
    } else {
        short.and_then(|n| child_text(n, "event_name"))
    };
    let title = title.filter(|s| !s.trim().is_empty())?;
    // Broadcast event IDs are reused and do not identify stable episodes.
    Some(Program {
        id: 0,
        source_id: 0,
        guide_key: key.into(),
        starts_at: start,
        ends_at: start + length,
        title,
        metadata: ProgramMetadata {
            description: event_description(table, event, descriptions),
            ..Default::default()
        },
    })
}

pub(super) fn parse(value: &str) -> Result<Guide, String> {
    let doc = roxmltree::Document::parse_with_options(
        value,
        roxmltree::ParsingOptions {
            nodes_limit: 1_000_000,
            ..Default::default()
        },
    )
    .map_err(|_| "The broadcast guide decoder returned invalid XML")?;
    if !doc.root_element().has_tag_name("tsduck") {
        return Err("The broadcast guide decoder returned an unexpected document".into());
    }
    let (mut stations, descriptions) = station_tables(doc.root_element());
    let mut programs = HashMap::new();
    for table in doc.root_element().children().filter(|n| {
        matches!(n.tag_name().name(), "EIT" | "ATSC_EIT") && n.attribute("current") != Some("false")
    }) {
        let atsc = table.has_tag_name("ATSC_EIT");
        let key = if atsc {
            format!("atsc:{}", number(table.attribute("source_id")).unwrap_or(0))
        } else {
            format!(
                "dvb:{}:{}:{}",
                number(table.attribute("original_network_id")).unwrap_or(0),
                number(table.attribute("transport_stream_id")).unwrap_or(0),
                number(table.attribute("service_id")).unwrap_or(0)
            )
        };
        for event in table.children().filter(|n| n.has_tag_name("event")) {
            if let Some(program) = event_program(table, event, &key, &descriptions) {
                programs.insert((key.clone(), program.starts_at), program);
            }
        }
    }
    for (key, _) in programs.keys() {
        stations.entry(key.clone()).or_insert_with(|| Station {
            source_id: 0,
            guide_key: key.clone(),
            name: key.clone(),
            number: String::new(),
            aliases: vec![],
            logo: None,
        });
    }
    Ok(Guide {
        programs: programs.into_values().collect(),
        stations: stations.into_values().collect(),
    })
}

pub(super) async fn decode(
    _config: &SourceConfig,
    mut input: impl tokio::io::AsyncRead + Unpin,
) -> Result<Guide, String> {
    let mut data = Vec::new();
    let mut buffer = vec![0; 188 * 256];
    let end = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let read = tokio::select! {
            _ = tokio::time::sleep_until(end) => break,
            result = input.read(&mut buffer) => {
                result.map_err(|_| "The broadcast stream could not be read")?
            },
        };
        if read == 0 || data.len() + read > 64 * 1024 * 1024 {
            break;
        }
        data.extend_from_slice(&buffer[..read]);
    }
    super::tsduck::decode(data).await
}
async fn recording_soon(
    db: &DbConn,
    source: i32,
) -> bool {
    db.run(move |conn| {
        r::table
            .filter(r::channel_id.eq_any(c::table.filter(c::source_id.eq(source)).select(c::id)))
            .filter(r::state.eq_any(["scheduled", "recording"]))
            .filter(r::starts_at.le(super::now() + 3660))
            .filter(r::ends_at.gt(super::now()))
            .select((r::starts_at, r::padding_before))
            .load::<(i64, i32)>(conn)
    })
    .await
    .map(|timers| {
        timers
            .iter()
            .any(|(start, padding)| start - i64::from(*padding) <= super::now() + 45)
    })
    .unwrap_or(true)
}

async fn collect_native(
    db: &DbConn,
    source: &Source,
    config: &SourceConfig,
) -> Result<(Guide, Option<String>), String> {
    let mut result = Guide {
        programs: Vec::new(),
        stations: Vec::new(),
    };
    let mut last_error = None;
    let channels = if source.kind == "native" {
        super::tsduck::channels(source, config).await?
    } else {
        super::native::channels(config).await?
    };
    let mut seen = HashSet::new();
    for channel in channels {
        if !seen.insert(
            channel
                .source_key
                .split(':')
                .next()
                .unwrap_or("")
                .to_owned(),
        ) {
            continue;
        }
        if recording_soon(db, source.id).await {
            break;
        }
        let _lease = super::stream::lease(source)?;
        let (_scan, cancel) = scan(source.id);
        let mut tuner;
        let output: Box<dyn tokio::io::AsyncRead + Unpin + Send> = if source.kind == "native" {
            Box::new(super::tsduck::tune(config, &channel).await?)
        } else {
            tuner = super::native::tune(config, &channel, true)?;
            Box::new(tuner.stdout.take().ok_or("Unable to read the DVB tuner")?)
        };
        match decode_idle(config, output, cancel).await {
            Ok(guide) => {
                result.programs.extend(guide.programs);
                result.stations.extend(guide.stations);
            }
            Err(error) => {
                last_error = Some(error);
                break;
            }
        }
    }
    Ok((result, last_error))
}

async fn collect_network(
    db: &DbConn,
    source: &Source,
    config: &SourceConfig,
) -> Result<(Guide, Option<String>), String> {
    let mut result = Guide {
        programs: Vec::new(),
        stations: Vec::new(),
    };
    let mut last_error = None;
    if config.multiplex_urls.is_empty() {
        return Err(
            "Enter unfiltered multiplex URLs for broadcast guide scanning. HDHomeRun requires \
             physical RF channels or frequencies, rather than virtual channel URLs"
                .into(),
        );
    }
    for value in &config.multiplex_urls {
        if recording_soon(db, source.id).await {
            break;
        }
        let _lease = super::stream::lease(source)?;
        let (_scan, cancel) = scan(source.id);
        let url = super::formats::http_url(value)?;
        let response = super::sources::client()?
            .get(url)
            .send()
            .await
            .map_err(|_| "Unable to tune the broadcast multiplex")?
            .error_for_status()
            .map_err(|_| "The broadcast tuner is unavailable")?;
        // A small duplex pipe bounds memory while the HTTP producer feeds the table decoder.
        let (mut writer, reader) = tokio::io::duplex(188 * 1024);
        let producer = AbortTask(tokio::spawn(async move {
            let mut response = response;
            while let Ok(Some(chunk)) = response.chunk().await {
                if writer.write_all(&chunk).await.is_err() {
                    break;
                }
            }
        }));
        let guide = decode_idle(config, reader, cancel).await;
        drop(producer);
        match guide {
            Ok(guide) => {
                result.programs.extend(guide.programs);
                result.stations.extend(guide.stations);
            }
            Err(error) => {
                last_error = Some(error);
                break;
            }
        }
    }
    Ok((result, last_error))
}

pub(super) async fn collect(
    db: &DbConn,
    source: &Source,
    config: &SourceConfig,
) -> Result<Guide, String> {
    if recording_soon(db, source.id).await {
        return Err(
            "Broadcast guide scanning is deferred while a recording is due or in progress".into(),
        );
    }
    let (mut result, last_error) = if matches!(source.kind.as_str(), "dvb" | "native") {
        collect_native(db, source, config).await?
    } else {
        collect_network(db, source, config).await?
    };
    if result.programs.is_empty() {
        return Err(last_error.unwrap_or_else(|| {
            "No broadcast schedule was received. Check reception and that the source includes DVB \
             EIT or ATSC PSIP tables"
                .into()
        }));
    }
    let mut keys = HashSet::new();
    result.stations.retain(|s| keys.insert(s.guide_key.clone()));
    Ok(result)
}

struct AbortTask(tokio::task::JoinHandle<()>);
impl Drop for AbortTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}
