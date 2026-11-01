//! Loopback delivery of completed capture segments, including a continuous transport stream.
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{
            AtomicBool,
            Ordering,
        },
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
    task::{
        JoinHandle,
        JoinSet,
    },
};

pub(super) struct FileServer {
    pub url: String,
    task: JoinHandle<()>,
    finished: Arc<AtomicBool>,
}
impl Drop for FileServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(super) fn valid_segment_name(value: &str) -> bool {
    value
        .strip_prefix("segment-")
        .and_then(|s| s.strip_suffix(".ts"))
        .is_some_and(|number| number.len() == 10 && number.bytes().all(|b| b.is_ascii_digit()))
}

async fn respond(
    mut socket: TcpStream,
    directory: PathBuf,
    token: String,
    finished: Arc<AtomicBool>,
) -> std::io::Result<()> {
    let mut header = Vec::new();
    let mut bytes = [0; 1024];
    while header.len() < 8192 && !header.windows(4).any(|v| v == b"\r\n\r\n") {
        let count = tokio::time::timeout(Duration::from_secs(5), socket.read(&mut bytes)).await??;
        if count == 0 {
            return Ok(());
        }
        header.extend_from_slice(&bytes[..count]);
    }
    let request = String::from_utf8_lossy(&header);
    let mut first = request.lines().next().unwrap_or("").split_whitespace();
    let method = first.next().unwrap_or("");
    let target = first.next().unwrap_or("");
    let (target, query) = target.split_once('?').unwrap_or((target, ""));
    let prefix = format!("/{token}/");
    let name = target.strip_prefix(&prefix).unwrap_or("");
    if method == "GET" && name == "buffer.ts" {
        if let Some(index) = query
            .strip_prefix("segment=")
            .and_then(|v| v.parse::<u64>().ok())
        {
            if index < 10_000_000_000 {
                return continuous(socket, directory, index, finished).await;
            }
        }
    }
    if !matches!(method, "GET" | "HEAD") || (name != "buffer.m3u8" && !valid_segment_name(name)) {
        socket
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await?;
        return Ok(());
    }
    let Ok(file) = tokio::fs::File::open(directory.join(name)).await else {
        socket
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await?;
        return Ok(());
    };
    let length = file.metadata().await?.len();
    let mime = if name == "buffer.m3u8" { "application/vnd.apple.mpegurl" } else { "video/mp2t" };
    socket
        .write_all(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: \
                 {length}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n"
            )
            .as_bytes(),
        )
        .await?;
    if method == "GET" {
        tokio::io::copy(&mut file.take(length), &mut socket).await?;
    }
    socket.shutdown().await
}

/// Follow completed files immediately, avoiding a second HLS demuxer's playlist reload delay.
async fn continuous(
    mut socket: TcpStream,
    directory: PathBuf,
    mut index: u64,
    finished: Arc<AtomicBool>,
) -> std::io::Result<()> {
    socket
        .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: video/mp2t\r\n\
        Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        )
        .await?;
    loop {
        let path = directory.join(format!("segment-{index:010}.ts"));
        match tokio::fs::File::open(&path).await {
            Ok(mut file) => {
                tokio::io::copy(&mut file, &mut socket).await?;
                index += 1;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if finished.load(Ordering::Acquire) {
                    // Capture publishes its final file before marking completion.
                    if path.is_file() {
                        continue;
                    }
                    return socket.shutdown().await;
                }
                let mut disconnected = [0; 1];
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(25)) => {},
                    _ = socket.read(&mut disconnected) => return Ok(()),
                }
            }
            Err(error) => return Err(error),
        }
    }
}

impl FileServer {
    /// Keep segment sequence numbers stable when older entries leave the rolling playlist.
    pub fn stream_url(
        &self,
        segment: &str,
    ) -> Result<String, String> {
        if !valid_segment_name(segment) {
            return Err("Invalid TV buffer segment".into());
        }
        let index = &segment[8..18];
        Ok(self
            .url
            .replace("buffer.m3u8", &format!("buffer.ts?segment={index}")))
    }

    /// End readers after every completed segment has been delivered.
    pub fn finish(&self) {
        self.finished.store(true, Ordering::Release);
    }

    pub async fn start(
        directory: PathBuf,
        token: &str,
    ) -> Result<Self, String> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_| "Unable to open local TV buffer delivery")?;
        let address = listener
            .local_addr()
            .map_err(|_| "Unable to locate local TV buffer delivery")?;
        let url = format!("http://{address}/{token}/buffer.m3u8");
        let token = token.to_owned();
        let finished = Arc::new(AtomicBool::new(false));
        let capture_finished = finished.clone();
        let task = tokio::spawn(async move {
            let mut clients = JoinSet::new();
            loop {
                tokio::select! {
                    result=listener.accept()=> {
                        let Ok((socket,_))=result else {break;};
                        if clients.len() < 16 {
                            clients.spawn(respond(
                                socket,
                                directory.clone(),
                                token.clone(),
                                capture_finished.clone(),
                            ));
                        }
                    },
                    _=clients.join_next(), if !clients.is_empty()=>{}
                }
            }
        });
        Ok(Self {
            url,
            task,
            finished,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_names_are_limited_to_the_generated_segment_namespace() {
        assert!(valid_segment_name("segment-0000000001.ts"));
        for name in [
            "../segment-0000000001.ts",
            "segment-..ts",
            "capture.ts",
            "C:/private.ts",
            "segment-0000000001.ts/../private",
        ] {
            assert!(!valid_segment_name(name));
        }
    }

    #[rocket::async_test]
    async fn continuous_reads_follow_completed_segments_and_finish_without_playlist_polling() {
        let root =
            std::env::temp_dir().join(format!("koko-buffer-tail-{:032x}", rand::random::<u128>()));
        tokio::fs::create_dir_all(&root).await.unwrap();
        tokio::fs::write(root.join("segment-0000000012.ts"), b"first")
            .await
            .unwrap();
        tokio::fs::write(root.join("segment-0000000013.ts.tmp"), b"second")
            .await
            .unwrap();
        let files = FileServer::start(root.clone(), "tail").await.unwrap();
        let url = files.stream_url("segment-0000000012.ts").unwrap();
        assert!(files.stream_url("../../private.ts").is_err());
        let mut response = reqwest::get(url).await.unwrap();
        let mut received = Vec::new();
        while received.len() < 5 {
            received.extend(response.chunk().await.unwrap().unwrap());
        }
        assert_eq!(received, b"first");
        // Partial capture files must never be read before the atomic rename completes.
        assert!(
            tokio::time::timeout(Duration::from_millis(100), response.chunk())
                .await
                .is_err()
        );
        tokio::fs::rename(
            root.join("segment-0000000013.ts.tmp"),
            root.join("segment-0000000013.ts"),
        )
        .await
        .unwrap();
        files.finish();
        let rest = tokio::time::timeout(Duration::from_secs(2), response.bytes())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(rest.as_ref(), b"second");
        drop(files);
        tokio::fs::remove_dir_all(root).await.unwrap();
    }

    #[rocket::async_test]
    async fn disconnected_readers_release_their_slot_while_waiting_for_another_segment() {
        let root = std::env::temp_dir().join(format!(
            "koko-buffer-disconnect-{:032x}",
            rand::random::<u128>()
        ));
        tokio::fs::create_dir_all(&root).await.unwrap();
        tokio::fs::write(root.join("segment-0000000000.ts"), b"video")
            .await
            .unwrap();
        let files = FileServer::start(root.clone(), "disconnect").await.unwrap();
        let url = files.stream_url("segment-0000000000.ts").unwrap();
        // Exceed the listener's simultaneous-reader limit without producing a second file.
        for _ in 0..20 {
            let mut response = reqwest::get(&url).await.unwrap();
            let chunk = tokio::time::timeout(Duration::from_secs(2), response.chunk())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert!(!chunk.is_empty());
            drop(response);
        }
        drop(files);
        tokio::fs::remove_dir_all(root).await.unwrap();
    }

    #[rocket::async_test]
    async fn playlist_reads_see_updates_and_cannot_open_other_files() {
        let root =
            std::env::temp_dir().join(format!("koko-buffer-files-{:032x}", rand::random::<u128>()));
        tokio::fs::create_dir_all(&root).await.unwrap();
        tokio::fs::write(root.join("buffer.m3u8"), "first playlist")
            .await
            .unwrap();
        tokio::fs::write(root.join("private.txt"), "private")
            .await
            .unwrap();
        let files = FileServer::start(root.clone(), "test-buffer")
            .await
            .unwrap();
        let client = reqwest::Client::new();
        assert_eq!(
            client
                .get(&files.url)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap(),
            "first playlist"
        );
        tokio::fs::write(root.join("buffer.m3u8"), "updated playlist")
            .await
            .unwrap();
        assert_eq!(
            client
                .get(&files.url)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap(),
            "updated playlist"
        );
        for url in [
            files.url.replace("buffer.m3u8", "private.txt"),
            files.url.replace("test-buffer", "wrong-token"),
        ] {
            assert_eq!(
                client.get(url).send().await.unwrap().status(),
                reqwest::StatusCode::NOT_FOUND
            );
        }
        drop(files);
        tokio::fs::remove_dir_all(root).await.unwrap();
    }
}
