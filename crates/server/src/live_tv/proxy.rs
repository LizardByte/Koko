//! Per-stream loopback proxy for TLS-capable HTTP and authenticated HLS resource rewriting.

use base64::{
    Engine,
    engine::general_purpose::STANDARD,
};
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

#[derive(Clone, Eq, Hash, PartialEq)]
struct Resource {
    url: Url,
    key: bool,
}

struct Resources {
    clock: u64,
    next: u64,
    urls: HashMap<u64, (Resource, u64)>,
    ids: HashMap<Resource, u64>,
}
impl Resources {
    fn insert(
        &mut self,
        resource: Resource,
    ) -> u64 {
        self.clock += 1;
        if let Some(id) = self.ids.get(&resource).copied() {
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
                if let Some((resource, _)) = self.urls.remove(&id) {
                    self.ids.remove(&resource);
                }
            }
        }
        let id = self.next;
        self.next += 1;
        self.urls.insert(id, (resource.clone(), self.clock));
        self.ids.insert(resource, id);
        id
    }

    fn get(
        &mut self,
        id: u64,
    ) -> Option<Resource> {
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
        key: bool,
    ) -> Result<String, String> {
        let suffix = if url.scheme() == "data" {
            inline_key(&url)?;
            "key".to_owned()
        } else {
            super::formats::http_url(url.as_str())?;
            if key {
                "key".to_owned()
            } else {
                url.path()
                    .rsplit('.')
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase()
            }
        };
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
            .insert(Resource { url, key });
        Ok(format!("{}/{}/{id}.{suffix}", self.root, self.token))
    }
}

/// Accept only the 16-byte AES keys embedded by HLS providers, never arbitrary inline media.
fn inline_key(url: &Url) -> Result<Vec<u8>, String> {
    let invalid = "Invalid inline HLS encryption key";
    let (metadata, encoded) = url.path().split_once(',').ok_or(invalid)?;
    if !metadata.ends_with(";base64")
        || encoded.len() != 24
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid.into());
    }
    let key = STANDARD.decode(encoded).map_err(|_| invalid)?;
    if key.len() != 16 {
        return Err(invalid.into());
    }
    Ok(key)
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
        .read_timeout(super::stream::SOURCE_READ_TIMEOUT)
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
    let input = state.resource(super::formats::http_url(url)?, false)?;
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

/// Give cold providers bounded time to recover before forwarding bytes; never replay partial media.
async fn upstream_response(builder: reqwest::RequestBuilder) -> Result<reqwest::Response, String> {
    let deadline = tokio::time::Instant::now() + super::stream::SOURCE_READ_TIMEOUT;
    for attempt in 0..5 {
        let request = builder
            .try_clone()
            .ok_or("Unable to retry the TV source request")?;
        let result = tokio::time::timeout_at(deadline, request.send()).await;
        match result {
            Ok(Ok(response)) => {
                if attempt == 4
                    || !matches!(response.status().as_u16(), 408 | 500 | 502 | 503 | 504)
                {
                    return Ok(response);
                }
                log::debug!(
                    "Retrying a TV source request after HTTP {}",
                    response.status().as_u16()
                );
            }
            Ok(Err(_)) if attempt == 4 => return Err("Unable to connect to the TV source".into()),
            Ok(Err(_)) => {}
            Err(_) => return Err("The TV source did not respond in time".into()),
        }
        tokio::time::timeout_at(
            deadline,
            tokio::time::sleep(Duration::from_secs(1 << attempt)),
        )
        .await
        .map_err(|_| "The TV source did not respond in time")?;
    }
    unreachable!("The final attempt returns its response or error")
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
            let key = line.starts_with("#EXT-X-KEY:") || line.starts_with("#EXT-X-SESSION-KEY:");
            let mut offset = 0;
            for capture in URI.captures_iter(line) {
                let field = capture.get(1).unwrap();
                output.push_str(&line[offset..field.start()]);
                output.push_str(
                    &state.resource(
                        base.join(field.as_str())
                            .map_err(|_| "Invalid HLS resource URL")?,
                        key,
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
                    false,
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
    let resource = state
        .resources
        .lock()
        .map_err(|_| "Unable to read stream resources")?
        .get(id.unwrap())
        .ok_or("Stream resource expired")?;
    let url = resource.url;
    if url.scheme() == "data" {
        let key = inline_key(&url)?;
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: \
                  16\r\nConnection: close\r\n\r\n",
            )
            .await
            .map_err(|_| "TV player disconnected")?;
        if !head {
            socket
                .write_all(&key)
                .await
                .map_err(|_| "TV player disconnected")?;
        }
        let _ = socket.shutdown().await;
        return Ok(());
    }
    let mut builder = state.client.get(url);
    if let Some(range) = range {
        builder = builder.header(reqwest::header::RANGE, range);
    }
    let mut response = match upstream_response(builder).await {
        Ok(response) => response,
        Err(message) => {
            log::warn!("Live TV relay: {message}");
            socket
                .write_all(
                    b"HTTP/1.1 502 Upstream Unavailable\r\nContent-Length: \
                0\r\nConnection: close\r\n\r\n",
                )
                .await
                .map_err(|_| "TV player disconnected")?;
            return Ok(());
        }
    };
    let status = response.status().as_u16();
    let base = response.url().clone();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_owned();
    let content_type = if resource.key { "application/octet-stream" } else { &content_type };
    // Reqwest's size hint shrinks as chunks are consumed. The relay also sends the sniffed prefix.
    let content_length = response.content_length();
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
    // Some providers label keys and segments as playlists. Keys are always opaque bytes, even
    // if a valid random key happens to begin with the playlist signature.
    let playlist = !resource.key && first.starts_with(b"#EXTM3U");
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
        if let Some(length) = content_length {
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

    async fn scripted_upstream(statuses: Vec<u16>) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/master.m3u8", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            for status in statuses {
                let (mut socket, _) = listener.accept().await.unwrap();
                request(&mut socket).await.unwrap();
                let body = "#EXTM3U\n#EXTINF:2,\nsegment.ts\n";
                let header = format!(
                    "HTTP/1.1 {status} Upstream\r\nContent-Type: \
                     application/vnd.apple.mpegurl\r\nContent-Length: {}\r\nConnection: \
                     close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(header.as_bytes()).await.unwrap();
            }
        });
        (url, task)
    }

    #[rocket::async_test]
    async fn transient_upstream_failures_retry_before_forwarding_a_playlist() {
        let (url, upstream) = scripted_upstream(vec![503, 502, 503, 503, 200]).await;
        let (_relay, input) = start(&url).await.unwrap();
        let response = reqwest::get(&input).await.unwrap();
        assert_eq!(response.status(), 200);
        let body = response.text().await.unwrap();
        assert!(body.starts_with("#EXTM3U\n"));
        assert!(!body.contains(&url));
        upstream.await.unwrap();
    }

    #[rocket::async_test]
    async fn permanent_upstream_errors_are_bounded_and_missing_channels_are_not_retried() {
        for statuses in [vec![500; 5], vec![404]] {
            let status = statuses[0];
            let (url, upstream) = scripted_upstream(statuses).await;
            let (_relay, input) = start(&url).await.unwrap();
            let response = reqwest::get(&input).await.unwrap();
            assert_eq!(response.status().as_u16(), status);
            upstream.await.unwrap();
        }
    }

    #[rocket::async_test]
    async fn inline_hls_keys_are_relayed_as_bounded_binary_resources() {
        let key = b"0123456789abcdef";
        let inline = format!("data:text/plain;base64,{}", STANDARD.encode(key));
        let master = format!(
            "#EXTM3U\n#EXT-X-SESSION-KEY:METHOD=AES-128,URI=\"{inline}\"\n#EXT-X-STREAM-INF:\
             BANDWIDTH=1000000\nmedia.m3u8\n"
        );
        let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/master.m3u8", upstream.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = upstream.accept().await.unwrap();
            request(&mut socket).await.unwrap();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: \
                 application/vnd.apple.mpegurl\r\nContent-Length: {}\r\nConnection: \
                 close\r\n\r\n{master}",
                master.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        let (_proxy, url) = start(&url).await.unwrap();
        let client = reqwest::Client::new();
        let master = client.get(url).send().await.unwrap().text().await.unwrap();
        assert!(!master.contains("data:") && !master.contains(&STANDARD.encode(key)));
        let pattern = regex::Regex::new(r#"URI="([^"]+)""#).unwrap();
        let captures = pattern.captures(&master).unwrap();
        let key_url = &captures[1];
        assert!(key_url.ends_with(".key"));
        let response = client.get(key_url).send().await.unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(response.content_length(), Some(16));
        assert_eq!(response.bytes().await.unwrap().as_ref(), key);
        let response = client.head(key_url).send().await.unwrap();
        assert_eq!(response.headers()[reqwest::header::CONTENT_LENGTH], "16");
        assert!(response.bytes().await.unwrap().is_empty());
        server.await.unwrap();
        for invalid in [
            "data:text/plain,0123456789abcdef",
            "data:text/plain;base64,AA==",
            "data:text/plain;base64,!!!!!!!!!!!!!!!!!!!!!!!!",
            "data:text/plain;base64,MDEyMzQ1Njc4OWFiY2RlZg==#fragment",
        ] {
            assert!(inline_key(&Url::parse(invalid).unwrap()).is_err());
        }
    }

    #[rocket::async_test]
    async fn external_keys_ignore_playlist_mime_types_and_signatures() {
        let key = b"#EXTM3U\x00\xff0123456";
        assert_eq!(key.len(), 16);
        let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/master.m3u8", upstream.local_addr().unwrap());
        let server = tokio::spawn(async move {
            for _ in 0..3 {
                let (mut socket, _) = upstream.accept().await.unwrap();
                let (path, _, _) = request(&mut socket).await.unwrap();
                let playlist = b"#EXTM3U\n#EXT-X-SESSION-KEY:METHOD=AES-128,URI=\"key.m3u8\"\n\
                                 #EXT-X-KEY:METHOD=AES-128,URI=\"key.m3u8\"\n\
                                 #EXTINF:2,\nsegment.ts\n";
                let body = if path == "/master.m3u8" { &playlist[..] } else { &key[..] };
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: \
                     application/vnd.apple.mpegurl\r\nContent-Length: {}\r\nConnection: \
                     close\r\n\r\n",
                    body.len()
                );
                socket.write_all(header.as_bytes()).await.unwrap();
                socket.write_all(body).await.unwrap();
            }
        });
        let (_relay, input) = start(&url).await.unwrap();
        let client = reqwest::Client::new();
        let playlist = client
            .get(input)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        let pattern = regex::Regex::new(r#"URI="([^"]+)""#).unwrap();
        let urls = pattern
            .captures_iter(&playlist)
            .map(|captures| captures[1].to_owned())
            .collect::<Vec<_>>();
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0], urls[1]);
        assert!(urls[0].ends_with(".key"));
        for head in [false, true] {
            let builder = if head { client.head(&urls[0]) } else { client.get(&urls[0]) };
            let response = builder.send().await.unwrap();
            assert_eq!(response.status(), 200);
            assert_eq!(response.headers()[reqwest::header::CONTENT_LENGTH], "16");
            assert_eq!(
                response.headers()[reqwest::header::CONTENT_TYPE],
                "application/octet-stream"
            );
            assert_eq!(
                response.bytes().await.unwrap().as_ref(),
                if head { &[][..] } else { &key[..] }
            );
        }
        server.await.unwrap();
    }

    #[rocket::async_test]
    async fn binary_resources_keep_the_full_length_after_playlist_sniffing() {
        for (size, head, range) in [
            (4, false, false),
            (1024, false, false),
            (256 * 1024, false, false),
            (1024, false, true),
            (1024, true, false),
        ] {
            let data = vec![0xa5; size];
            let expected = data.clone();
            let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}/init.mp4?v=1", upstream.local_addr().unwrap());
            let server = tokio::spawn(async move {
                let (mut socket, _) = upstream.accept().await.unwrap();
                let (_, requested_range, _) = request(&mut socket).await.unwrap();
                assert_eq!(
                    requested_range.as_deref(),
                    range.then_some("bytes=1024-2047")
                );
                let status = if range { "206 Partial Content" } else { "200 OK" };
                let content_range =
                    if range { "Content-Range: bytes 1024-2047/4096\r\n" } else { "" };
                socket
                    .write_all(
                        format!(
                            "HTTP/1.1 {status}\r\nContent-Type: \
                             application/vnd.apple.mpegurl\r\nContent-Length: \
                             {size}\r\n{content_range}Connection: close\r\n\r\n"
                        )
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
                socket.write_all(&data).await.unwrap();
            });
            let (_proxy, url) = start(&url).await.unwrap();
            let client = reqwest::Client::new();
            let mut builder = if head { client.head(url) } else { client.get(url) };
            if range {
                builder = builder.header(reqwest::header::RANGE, "bytes=1024-2047");
            }
            let response = builder.send().await.unwrap();
            assert_eq!(response.status().as_u16(), if range { 206 } else { 200 });
            assert_eq!(
                response.headers()[reqwest::header::CONTENT_LENGTH],
                size.to_string(),
                "The relay must include the bytes already read while detecting playlists"
            );
            if range {
                assert_eq!(
                    response.headers()[reqwest::header::CONTENT_RANGE],
                    "bytes 1024-2047/4096"
                );
            }
            let body = response.bytes().await.unwrap();
            assert_eq!(body.as_ref(), if head { &[][..] } else { &expected });
            server.await.unwrap();
        }
    }

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
        let inline_key = "data:text/plain;base64,ABEiM0RVZneImaq7zN3u/w==";
        let original = format!(
            "#EXTM3U\n#EXT-X-SESSION-KEY:METHOD=AES-128,URI=\"{inline_key}\"\n#EXT-X-KEY:\
             METHOD=AES-128,URI=\"key?secret=hidden\"\n#EXT-X-MAP:URI=\"init.mp4\"\n../segment.ts?\
             token=secret\n"
        );
        let result = rewrite(
            &original,
            &Url::parse("https://provider.example/live/index.m3u8").unwrap(),
            &state,
        )
        .unwrap();
        assert!(!result.contains("provider.example"));
        assert!(!result.contains("hidden"));
        assert!(!result.contains("token=secret"));
        assert!(!result.contains(inline_key));
        assert_eq!(result.matches("http://127.0.0.1:1234/opaque/").count(), 4);
        assert!(
            rewrite(
                "#EXTM3U\nfile:///private/secret",
                &Url::parse("https://example.com/").unwrap(),
                &state
            )
            .is_err()
        );
        assert!(
            rewrite(
                "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"file:///private/secret\"",
                &Url::parse("https://example.com/").unwrap(),
                &state
            )
            .is_err()
        );
    }
}
