//! Linux DVB tuning through v4l-utils, with argument-safe subprocesses and full multiplex capture.

use super::{
    Channel,
    SourceConfig,
};
use std::{
    collections::HashMap,
    process::Stdio,
};
use tokio::process::{
    Child,
    Command,
};

pub(super) fn parse_channels(value: &str) -> Result<Vec<Channel>, String> {
    let mut name = String::new();
    let mut fields = HashMap::<String, String>::new();
    let mut rows = Vec::new();
    fn add(
        rows: &mut Vec<Channel>,
        name: &str,
        fields: &HashMap<String, String>,
    ) {
        let (Some(service), Some(frequency)) = (fields.get("SERVICE_ID"), fields.get("FREQUENCY"))
        else {
            return;
        };
        let mut row = Channel::new(format!("{frequency}:{service}"), name.into(), name.into());
        row.number = fields
            .get("VCHANNEL")
            .or_else(|| fields.get("CHANNEL_NUMBER"))
            .cloned()
            .unwrap_or_default();
        rows.push(row);
    }
    for line in value.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            add(&mut rows, &name, &fields);
            fields.clear();
            name = line[1..line.len() - 1].trim().into();
        } else if !line.starts_with('#') {
            if let Some((key, value)) = line.split_once('=') {
                fields.insert(key.trim().to_owned(), value.trim().to_owned());
            }
        }
    }
    add(&mut rows, &name, &fields);
    if rows.is_empty() {
        return Err(
            "No DVB services were found. Scan channels with dvbv5-scan and enter the resulting \
             channel file path"
                .into(),
        );
    }
    if rows.len() > 20_000 {
        return Err("The DVB channel file exceeds 20,000 services".into());
    }
    Ok(rows)
}

pub(super) async fn channels(config: &SourceConfig) -> Result<Vec<Channel>, String> {
    use tokio::io::AsyncReadExt;
    if !cfg!(target_os = "linux") {
        return Err(
            "DVB channel files require Linux. For Windows, choose Local tuner (Windows BDA)".into(),
        );
    }
    let mut file = tokio::fs::File::open(&config.url)
        .await
        .map_err(|_| "Unable to read the DVB channel file; check its path and server permissions")?
        .take(16 * 1024 * 1024 + 1);
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .await
        .map_err(|_| "Unable to read the DVB channel file")?;
    if data.len() > 16 * 1024 * 1024 {
        return Err("The DVB channel file exceeds 16 MiB".into());
    }
    parse_channels(std::str::from_utf8(&data).map_err(|_| "The DVB channel file must use UTF-8")?)
}

pub(super) fn tune(
    config: &SourceConfig,
    channel: &Channel,
    all: bool,
) -> Result<Child, String> {
    let path = if config.zap_path.is_empty() { "dvbv5-zap" } else { &config.zap_path };
    let mut command = Command::new(path);
    command.args([
        "-a",
        &config.adapter.to_string(),
        "-f",
        &config.frontend.to_string(),
        "-c",
        &config.url,
        "-p",
        "-o",
        "-",
    ]);
    if all {
        command.arg("-P");
    }
    command
        .arg("--")
        .arg(&channel.name)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    command.spawn().map_err(|_| {
        "Unable to start dvbv5-zap; install v4l-utils and check the tuner permissions".into()
    })
}
