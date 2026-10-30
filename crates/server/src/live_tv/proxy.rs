//! Per-stream loopback proxy for TLS-capable HTTP and authenticated HLS resource rewriting.

use reqwest::Url;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        Mutex,
    },
    time::Duration,
};
use tokio::{
    io::{
        AsyncReadExt,
        AsyncWriteExt,
    },
    net::{
        TcpListener,
        TcpStream,
    },
};

struct Resources {
    clock: u64,
    next: u64,
    urls: HashMap<u64, (Url, u64)>,
    ids: HashMap<Url, u64>,
}
impl Resources {
    fn insert(
        &mut self,
        url: Url,
    ) -> u64 {
        self.clock += 1;
        if let Some(id) = self.ids.get(&url).copied() {
            if let Some(row) = self.urls.get_mut(&id) {
                row.1 = self.clock;
            }
            return id;
        }
        if self.urls.len() >= 8192 {
            if let Some(id) = self
                .urls
                .iter()
                .filter(|(id, _)| **id != 0)
                .min_by_key(|(_, (_, clock))| clock)
                .map(|(id, _)| *id)
            {
                if let Some((url, _)) = self.urls.remove(&id) {
                    self.ids.remove(&url);
                }
            }
        }
        let id = self.next;
        self.next += 1;
        self.urls.insert(id, (url.clone(), self.clock));
        self.ids.insert(url, id);
        id
    }

    fn get(
        &mut self,
        id: u64,
    ) -> Option<Url> {
        self.clock += 1;
        let row = self.urls.get_mut(&id)?;
        row.1 = self.clock;
        Some(row.0.clone())
    }
}
struct State {
    root: String,
    token: String,
    client: reqwest::Client,
    resources: Mutex<Resources>,
}
impl State {
    fn resource(
        &self,
        url: Url,
    ) -> Result<String, String> {
        super::formats::http_url(url.as_str())?;
        let suffix = url
            .path()
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let suffix = if matches!(
            suffix.as_str(),
            "m3u8" | "m3u" | "ts" | "m2ts" | "mp4" | "m4s" | "m4a" | "aac" | "mp3" | "vtt" | "key"
        ) {
            suffix.as_str()
        } else {
            "ts"
        };
        let id = self
            .resources
            .lock()
            .map_err(|_| "Unable to read stream resources")?
            .insert(url);
        Ok(format!("{}/{}/{id}.{suffix}", self.root, self.token))
    }
}

pub(super) struct Proxy {
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Proxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(super) async fn start(url: &str) -> Result<(Proxy, String), String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|_| "Unable to open the local TV stream relay")?;
    let root = format!(
        "http://{}",
        listener
            .local_addr()
            .map_err(|_| "Unable to open the local TV stream relay")?
    );
    let client = reqwest::Client::builder()
        .user_agent(concat!("Koko/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(20))
        .gzip(true)
        .build()
        .map_err(|_| "Unable to open the TV stream relay")?;
    let state = Arc::new(State {
        root,
        token: format!("{:032x}", rand::random::<u128>()),
        client,
        resources: Mutex::new(Resources {
            clock: 0,
            next: 0,
            urls: HashMap::new(),
            ids: HashMap::new(),
        }),
    });
    let input = state.resource(super::formats::http_url(url)?)?;
    let task = tokio::spawn(async move {
        let mut requests = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                accepted=listener.accept()=>{
                    let Ok((socket,_))=accepted else {break;};
                    if requests.len()>=32 {drop(socket);continue;}
                    let state=state.clone();
                    requests.spawn(async move {let _=serve(socket,state).await;});
                },
                _=requests.join_next(),if !requests.is_empty()=>{},
            }
        }
    });
    Ok((Proxy { task }, input))
}

fn rewrite(
    value: &str,
    base: &Url,
    state: &State,
) -> Result<String, String> {
    static URI: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| regex::Regex::new(r#"URI="([^"]+)""#).unwrap());
    let mut output = String::new();
    for line in value.lines() {
        if line.starts_with('#') {
            let mut offset = 0;
            for capture in URI.captures_iter(line) {
                let field = capture.get(1).unwrap();
                output.push_str(&line[offset..field.start()]);
                output.push_str(
                    &state.resource(
                        base.join(field.as_str())
                            .map_err(|_| "Invalid HLS resource URL")?,
                    )?,
                );
                offset = field.end();
            }
            output.push_str(&line[offset..]);
        } else if !line.trim().is_empty() {
            output.push_str(
                &state.resource(
                    base.join(line.trim())
                        .map_err(|_| "Invalid HLS stream URL")?,
                )?,
            );
        }
        output.push('\n');
    }
    Ok(output)
}

async fn request(socket: &mut TcpStream) -> Result<(String, Option<String>, bool), String> {
    let mut input = Vec::new();
    let mut buffer = [0; 1024];
    while !input.windows(4).any(|b| b == b"\r\n\r\n") {
        let size = socket
            .read(&mut buffer)
            .await
            .map_err(|_| "The local TV request failed")?;
        if size == 0 || input.len() + size > 16 * 1024 {
            return Err("Invalid local TV request".into());
        }
        input.extend_from_slice(&buffer[..size]);
    }
    let value = std::str::from_utf8(&input).map_err(|_| "Invalid local TV request")?;
    let mut lines = value.split("\r\n");
    let first = lines
        .next()
        .ok_or("Invalid local TV request")?
        .split_whitespace()
        .collect::<Vec<_>>();
    if first.len() != 3 || !matches!(first[0], "GET" | "HEAD") {
        return Err("Invalid local TV method".into());
    }
    let mut range = None;
    for line in lines {
        if let Some((key, value)) = line.split_once(':') {
            if key.eq_ignore_ascii_case("range") {
                let value = value.trim();
                if value.starts_with("bytes=")
                    && value[6..].bytes().all(|b| b.is_ascii_digit() || b == b'-')
                {
                    range = Some(value.into());
                }
            }
        }
    }
    Ok((first[1].into(), range, first[0] == "HEAD"))
}

async fn serve(
    mut socket: TcpStream,
    state: Arc<State>,
) -> Result<(), String> {
    let (path, range, head) = tokio::time::timeout(Duration::from_secs(5), request(&mut socket))
        .await
        .map_err(|_| "Local TV request timed out")??;
    let mut parts = path.trim_start_matches('/').split('/');
    let valid = parts.next() == Some(state.token.as_str());
    let id = parts
        .next()
        .and_then(|s| s.split('.').next())
        .and_then(|s| s.parse::<u64>().ok());
    if !valid || id.is_none() || parts.next().is_some() {
        let _ = socket
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await;
        return Ok(());
    }
    let url = state
        .resources
        .lock()
        .map_err(|_| "Unable to read stream resources")?
        .get(id.unwrap())
        .ok_or("Stream resource expired")?;
    let mut builder = state.client.get(url);
    if let Some(range) = range {
        builder = builder.header(reqwest::header::RANGE, range);
    }
    let mut response = builder
        .send()
        .await
        .map_err(|_| "Unable to connect to the TV stream")?;
    let status = response.status().as_u16();
    let base = response.url().clone();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_owned();
    let mut first = response
        .chunk()
        .await
        .map_err(|_| "Unable to read the TV stream")?
        .map(|b| b.to_vec())
        .unwrap_or_default();
    while !first.is_empty() && first.len() < 7 {
        let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "Unable to read the TV stream")?
        else {
            break;
        };
        first.extend_from_slice(&chunk);
    }
    let playlist =
        first.starts_with(b"#EXTM3U") || content_type.to_ascii_lowercase().contains("mpegurl");
    if playlist && response.status().is_success() {
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "Unable to read the HLS playlist")?
        {
            if first.len() + chunk.len() > 4 * 1024 * 1024 {
                return Err("The HLS playlist exceeds 4 MiB".into());
            }
            first.extend_from_slice(&chunk);
        }
        let text = std::str::from_utf8(&first).map_err(|_| "The HLS playlist is not UTF-8")?;
        let body = rewrite(text, &base, &state)?;
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.apple.mpegurl\r\nContent-Length: \
             {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        socket
            .write_all(header.as_bytes())
            .await
            .map_err(|_| "TV player disconnected")?;
        if !head {
            socket
                .write_all(body.as_bytes())
                .await
                .map_err(|_| "TV player disconnected")?;
        }
    } else {
        let mut header = format!(
            "HTTP/1.1 {status} Upstream\r\nContent-Type: {content_type}\r\nConnection: close\r\n"
        );
        if let Some(length) = response.content_length() {
            header.push_str(&format!("Content-Length: {length}\r\n"));
        }
        if let Some(range) = response
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|h| h.to_str().ok())
        {
            header.push_str(&format!("Content-Range: {range}\r\n"));
        }
        header.push_str("\r\n");
        socket
            .write_all(header.as_bytes())
            .await
            .map_err(|_| "TV player disconnected")?;
        if !head {
            socket
                .write_all(&first)
                .await
                .map_err(|_| "TV player disconnected")?;
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| "The upstream TV stream ended")?
            {
                socket
                    .write_all(&chunk)
                    .await
                    .map_err(|_| "TV player disconnected")?;
            }
        }
    }
    let _ = socket.shutdown().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hls_rewrites_nested_playlists_keys_maps_and_byte_range_resources() {
        let state = State {
            root: "http://127.0.0.1:1234".into(),
            token: "opaque".into(),
            client: reqwest::Client::new(),
            resources: Mutex::new(Resources {
                clock: 0,
                next: 0,
                urls: HashMap::new(),
                ids: HashMap::new(),
            }),
        };
        let original = "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"key?secret=hidden\"\n#EXT-X-MAP:\
                        URI=\"init.mp4\"\n../segment.ts?token=secret\n";
        let result = rewrite(
            original,
            &Url::parse("https://provider.example/live/index.m3u8").unwrap(),
            &state,
        )
        .unwrap();
        assert!(!result.contains("provider.example"));
        assert!(!result.contains("hidden"));
        assert!(!result.contains("token=secret"));
        assert_eq!(result.matches("http://127.0.0.1:1234/opaque/").count(), 3);
        assert!(
            rewrite(
                "#EXTM3U\nfile:///private/secret",
                &Url::parse("https://example.com/").unwrap(),
                &state
            )
            .is_err()
        );
    }
}
