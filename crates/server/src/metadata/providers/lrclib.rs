//! Plain and synchronized lyrics from LRCLIB, acquired only during metadata refreshes.
use super::MetadataProvider;
use crate::config::{
    MediaLibraryKind,
    MetadataProviderId,
};
use crate::media::lyrics::{
    Lookup,
    MusicLyrics,
};
use crate::metadata::{
    MetadataProviderDescriptor,
    MetadataProviderRole,
};
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::{
    collections::HashMap,
    time::Duration,
};
use tokio::{
    sync::Mutex,
    time::Instant,
};

// One production endpoint; test servers use independent entries. Hold the lock
// through each response so concurrent library refreshes share the same policy.
static NEXT_REQUEST: Lazy<Mutex<HashMap<String, Instant>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn application_id() -> Option<&'static str> {
    option_env!("KOKO_LRCLIB_APPLICATION_ID")
        .map(str::trim)
        .filter(|id| !id.is_empty())
}

fn build_request(
    url: &str,
    lookup: &Lookup,
    application_id: Option<&str>,
) -> Result<reqwest::Request, String> {
    let mut request = crate::metadata::http::client()
        .get(url)
        .query(&lookup.params())
        .timeout(Duration::from_secs(12))
        .header(
            reqwest::header::USER_AGENT,
            concat!(
                "Koko/",
                env!("CARGO_PKG_VERSION"),
                " (https://github.com/LizardByte/Koko)"
            ),
        );
    if let Some(id) = application_id.map(str::trim).filter(|id| !id.is_empty()) {
        let mut value = reqwest::header::HeaderValue::from_str(id)
            .map_err(|_| "Invalid LRCLIB application ID header".to_string())?;
        value.set_sensitive(true);
        request = request.header("Lrclib-App-Id", value);
    }
    request.build().map_err(|error| error.to_string())
}

fn retry_delay(response: &reqwest::Response) -> Duration {
    let seconds = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u32>().ok());
    let delay = seconds
        .map(|seconds| Duration::from_secs_f64(f64::from(seconds) * rand::random_range(1.0..=1.3)))
        .unwrap_or(Duration::from_millis(250));
    delay.max(Duration::from_millis(250))
}

pub(super) struct LrclibMetadataProvider;

impl MetadataProvider for LrclibMetadataProvider {
    fn descriptor(&self) -> MetadataProviderDescriptor {
        MetadataProviderDescriptor {
            id: MetadataProviderId::Lrclib,
            display_name: "LRCLIB".into(),
            description: "Plain and synchronized lyrics saved after a track is matched to its \
                          album. Playback reads saved lyrics without contacting LRCLIB."
                .into(),
            supported_kinds: vec![MediaLibraryKind::Music],
            requires_api_key: false,
            implemented: true,
            role: MetadataProviderRole::Secondary,
            extends_provider_ids: vec![MetadataProviderId::MusicBrainz],
            attribution_text: "Lyrics provided by LRCLIB.".into(),
            attribution_url: "https://lrclib.net/".into(),
            logo_light_url: Some("/assets/providers/lrclib.png".into()),
            logo_dark_url: Some("/assets/providers/lrclib.png".into()),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrcResponse {
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
    #[serde(default)]
    instrumental: bool,
}

#[cfg(test)]
tokio::task_local! { pub(crate) static TEST_LYRICS_URL: String; }

pub(crate) async fn fetch_lyrics(lookup: &Lookup) -> Result<MusicLyrics, String> {
    #[cfg(not(test))]
    let url = "https://lrclib.net/api/get".to_owned();
    #[cfg(test)]
    let url = TEST_LYRICS_URL.try_with(Clone::clone).unwrap_or_default();
    // Unmocked background refreshes in tests must never contact the real service.
    #[cfg(test)]
    if url.is_empty() {
        return Ok(MusicLyrics::empty());
    }
    let request = build_request(&url, lookup, application_id())?;
    let mut policy = NEXT_REQUEST.lock().await;
    if policy.len() > 128 {
        policy.clear();
    }
    let next_request = policy.entry(url).or_insert_with(Instant::now);
    tokio::time::sleep_until(*next_request).await;
    let result = fetch_response(request, next_request).await;
    *next_request = (*next_request).max(Instant::now() + Duration::from_millis(250));
    result
}

async fn fetch_response(
    request: reqwest::Request,
    next_request: &mut Instant,
) -> Result<MusicLyrics, String> {
    let client = crate::metadata::http::client();
    let retry_request = request
        .try_clone()
        .ok_or("Could not clone lyrics request")?;
    let first = client.execute(request).await;
    let retry = first.as_ref().map_or(true, |response| {
        response.status().is_server_error()
            || response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
    });
    let mut response = if retry {
        let delay = first
            .as_ref()
            .map(retry_delay)
            .unwrap_or(Duration::from_millis(250));
        *next_request = Instant::now() + delay;
        tokio::time::sleep_until(*next_request).await;
        client.execute(retry_request).await
    } else {
        first
    }
    .map_err(|error| error.to_string())?;
    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
        || response.status().is_server_error()
    {
        *next_request = Instant::now() + retry_delay(&response);
    }
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(MusicLyrics::empty());
    }
    response = response
        .error_for_status()
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
        if bytes.len() + chunk.len() > 1024 * 1024 {
            return Err("LRCLIB response exceeded the size limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let data: LrcResponse = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    Ok(MusicLyrics {
        plain_lyrics: data.plain_lyrics,
        synced_lyrics: data.synced_lyrics,
        instrumental: data.instrumental,
        ..MusicLyrics::empty()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_instrumental_and_missing_optional_text() {
        let response: LrcResponse =
            serde_json::from_value(serde_json::json!({"instrumental":true})).unwrap();
        assert!(response.instrumental);
        assert!(response.plain_lyrics.is_none() && response.synced_lyrics.is_none());
    }

    #[test]
    fn application_id_is_optional_and_only_sent_as_a_sensitive_header() {
        let lookup = Lookup {
            title: "Song".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            duration_ms: Some(180000),
        };
        for id in [None, Some(""), Some("  ")] {
            let request = build_request("https://lrclib.net/api/get", &lookup, id).unwrap();
            assert!(!request.headers().contains_key("Lrclib-App-Id"));
            assert!(
                request.headers()[reqwest::header::USER_AGENT]
                    .to_str()
                    .unwrap()
                    .contains("https://github.com/LizardByte/Koko")
            );
        }
        let request = build_request(
            "https://lrclib.net/api/get",
            &lookup,
            Some(" registered-app "),
        )
        .unwrap();
        assert_eq!(request.headers()["Lrclib-App-Id"], "registered-app");
        assert!(request.headers()["Lrclib-App-Id"].is_sensitive());
        assert!(!request.url().as_str().contains("registered-app"));
        assert!(
            build_request(
                "https://lrclib.net/api/get",
                &lookup,
                Some("invalid\nheader")
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn lookup_requests_are_spaced_and_honor_retry_after() {
        use std::io::{
            Read,
            Write,
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/lyrics", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let mut times = Vec::new();
            for index in 0..3 {
                let deadline = std::time::Instant::now() + Duration::from_secs(15);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(std::time::Instant::now() < deadline);
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("{error}"),
                    }
                };
                times.push(std::time::Instant::now());
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut buffer = [0; 4096];
                assert!(stream.read(&mut buffer).unwrap() > 0);
                let (status, headers, body) = if index == 0 {
                    ("429 Too Many Requests", "Retry-After: 1\r\n", "{}")
                } else {
                    ("200 OK", "", r#"{"plainLyrics":"Saved"}"#)
                };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: \
                     close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
            times
        });
        let lookup = Lookup {
            title: "Song".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            duration_ms: None,
        };
        TEST_LYRICS_URL
            .scope(url, async {
                for _ in 0..2 {
                    assert_eq!(
                        fetch_lyrics(&lookup).await.unwrap().plain_lyrics.as_deref(),
                        Some("Saved")
                    );
                }
            })
            .await;
        let times = worker.join().unwrap();
        assert!(times[1].duration_since(times[0]) >= Duration::from_secs(1));
        assert!(times[2].duration_since(times[1]) >= Duration::from_millis(250));
    }
}
