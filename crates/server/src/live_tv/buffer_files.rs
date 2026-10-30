//! Loopback-only HLS file delivery without holding C-runtime locks on a live playlist.
use std::{
    path::PathBuf,
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
    let target = first.next().unwrap_or("").split('?').next().unwrap_or("");
    let prefix = format!("/{token}/");
    let name = target.strip_prefix(&prefix).unwrap_or("");
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

impl FileServer {
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
        let task = tokio::spawn(async move {
            let mut clients = JoinSet::new();
            loop {
                tokio::select! {
                    result=listener.accept()=> {
                        let Ok((socket,_))=result else {break;};
                        if clients.len() < 16 {
                            clients.spawn(respond(socket, directory.clone(), token.clone()));
                        }
                    },
                    _=clients.join_next(), if !clients.is_empty()=>{}
                }
            }
        });
        Ok(Self { url, task })
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
