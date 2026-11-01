//! Disk-backed playlist and XMLTV transfers, without fixed feed size or entry-count caps.

use super::{
    Channel,
    formats,
    sources,
};
use std::io::{
    BufRead,
    BufReader,
    Seek,
    SeekFrom,
};
use tokio::io::AsyncWriteExt;

/// The OS removes this anonymous file when its last handle closes, including on failure.
pub(crate) fn temporary() -> Result<tokio::fs::File, String> {
    tempfile::tempfile()
        .map(tokio::fs::File::from_std)
        .map_err(|_| {
            "Unable to create a temporary TV feed file; check free space and permissions".into()
        })
}

async fn download(url: reqwest::Url) -> Result<tokio::fs::File, String> {
    download_file(url, false)
        .await?
        .ok_or_else(|| "The TV feed is unavailable".into())
}

async fn download_file(
    url: reqwest::Url,
    optional_bridge: bool,
) -> Result<Option<tokio::fs::File>, String> {
    // An idle transfer should fail, but a large feed that keeps delivering data should
    // not be cut off by the short total timeout used for tuner control requests.
    let client = sources::client_builder()
        .read_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|_| "Unable to create TV feed client")?;
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|_| "Unable to connect to the TV or guide source")?;
    // FastChannels explicitly returns Conflict when PrismCast is not configured. A warming or
    // failed export must still fail the import, preserving the previously imported lineup.
    if optional_bridge && response.status() == reqwest::StatusCode::CONFLICT {
        return Ok(None);
    }
    response = response.error_for_status().map_err(|_| {
        "The TV or guide source returned an error; check its address and credentials"
    })?;
    let mut file = temporary()?;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "The TV source transfer failed")?
    {
        file.write_all(&chunk)
            .await
            .map_err(|_| "Unable to save the TV feed; check temporary disk space")?;
    }
    file.flush()
        .await
        .map_err(|_| "Unable to save the TV feed; check temporary disk space")?;
    Ok(Some(file))
}

fn reader(mut file: std::fs::File) -> Result<Box<dyn BufRead + Send>, String> {
    file.seek(SeekFrom::Start(0))
        .map_err(|_| "Unable to read the temporary TV feed")?;
    let mut input = BufReader::with_capacity(64 * 1024, file);
    if input
        .fill_buf()
        .map_err(|_| "Unable to read the TV feed")?
        .starts_with(&[0x1f, 0x8b])
    {
        // Handle .gz downloads without Content-Encoding as well as concatenated gzip members.
        Ok(Box::new(BufReader::new(
            flate2::bufread::MultiGzDecoder::new(input),
        )))
    } else {
        Ok(Box::new(input))
    }
}

pub(super) async fn playlist_input(url: reqwest::Url) -> Result<(Vec<Channel>, u64), String> {
    playlist_details(url, false, false)
        .await
        .map(|(p, size)| (p.channels, size))
}

pub(super) async fn playlist_details(
    url: reqwest::Url,
    stable_ids: bool,
    allow_empty: bool,
) -> Result<(formats::Playlist, u64), String> {
    let bridge = stable_ids && allow_empty && url.path().contains("/prismcast");
    let Some(file) = download_file(url.clone(), bridge).await? else {
        return Ok((
            formats::Playlist {
                channels: vec![],
                identities: Default::default(),
            },
            0,
        ));
    };
    let size = file
        .metadata()
        .await
        .map_err(|_| "Unable to read the TV feed size")?
        .len();
    let file = file.into_std().await;
    let channels = tokio::task::spawn_blocking(move || {
        formats::read_playlist(reader(file)?, url.as_str(), stable_ids, allow_empty)
    })
    .await
    .map_err(|_| "Unable to process the channel playlist")??;
    Ok((channels, size))
}

pub(super) async fn guide_input(url: reqwest::Url) -> Result<(formats::Guide, u64), String> {
    let file = download(url).await?;
    let size = file
        .metadata()
        .await
        .map_err(|_| "Unable to read the guide size")?
        .len();
    Ok((read_guide(file.into_std().await).await?, size))
}

#[cfg(test)]
pub(super) async fn playlist(url: reqwest::Url) -> Result<Vec<Channel>, String> {
    playlist_input(url).await.map(|v| v.0)
}

#[cfg(test)]
pub(super) async fn guide(url: reqwest::Url) -> Result<formats::Guide, String> {
    guide_input(url).await.map(|v| v.0)
}

pub(super) async fn read_guide(file: std::fs::File) -> Result<formats::Guide, String> {
    tokio::task::spawn_blocking(move || formats::read_guide(reader(file)?, Some(super::now())))
        .await
        .map_err(|_| "Unable to process the XMLTV guide")?
}

#[cfg(test)]
mod tests;
