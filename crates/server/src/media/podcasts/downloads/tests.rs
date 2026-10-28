use super::*;
use tokio::io::{
    AsyncReadExt,
    AsyncWriteExt,
};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir()
            .join(format!(
                "koko-podcast-download-{}-{}",
                std::process::id(),
                crate::utils::current_timestamp()
            ))
            .join(rand::random::<u64>().to_string());
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn publisher(response: &'static [u8]) -> reqwest::Response {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        let _ = socket.read(&mut request).await.unwrap();
        socket.write_all(response).await.unwrap();
    });
    reqwest::get(format!("http://{address}/episode"))
        .await
        .unwrap()
}

#[tokio::test]
async fn complete_downloads_are_published_atomically_with_the_original_bytes() {
    let scratch = Scratch::new();
    let path = scratch.0.join("episode.mp3");
    let response = publisher(
        b"HTTP/1.1 200 OK\r\n\
Content-Type: audio/mpeg\r\n\
Content-Length: 4\r\nConnection: close\r\n\r\nID3!",
    )
    .await;
    assert_eq!(save_response(response, &path).await.unwrap(), 4);
    assert_eq!(std::fs::read(&path).unwrap(), b"ID3!");
    assert!(!path.with_extension("part").exists());
}

#[tokio::test]
async fn interrupted_downloads_never_become_playable_files() {
    let scratch = Scratch::new();
    let path = scratch.0.join("episode.mp3");
    let response = publisher(
        b"HTTP/1.1 200 OK\r\n\
Content-Type: audio/mpeg\r\n\
Content-Length: 100\r\nConnection: close\r\n\r\nID3!",
    )
    .await;
    assert!(save_response(response, &path).await.is_err());
    assert!(!path.exists());
    assert!(!path.with_extension("part").exists());
}

#[tokio::test]
async fn publisher_error_pages_and_partial_responses_are_not_saved_as_episodes() {
    let scratch = Scratch::new();
    let path = scratch.0.join("episode.mp3");
    for bytes in [
        b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 4\r\n\r\noops".as_slice(),
        b"HTTP/1.1 206 Partial Content\r\n\
Content-Type: audio/mpeg\r\n\
Content-Length: 4\r\n\r\nID3!"
            .as_slice(),
    ] {
        assert!(save_response(publisher(bytes).await, &path).await.is_err());
        assert!(!path.exists());
    }
}

#[test]
fn download_locks_are_shared_only_for_the_same_episode() {
    let first = episode_lock(1);
    let second = episode_lock(1);
    let another = episode_lock(2);
    assert!(Arc::ptr_eq(&first, &second));
    assert!(!Arc::ptr_eq(&first, &another));
}

#[test]
fn measured_runtime_uses_the_downloaded_media_and_rejects_invalid_probe_output() {
    assert_eq!(
        measured_duration(br#"{"format":{"duration":"1900.25"}}"#),
        Some(1_900_250)
    );
    assert_eq!(
        measured_duration(br#"{"streams":[{"duration":"10.5"},{"duration":"12"}]}"#),
        Some(12_000)
    );
    for bytes in [
        br#"{"format":{"duration":"NaN"}}"#.as_slice(),
        b"invalid",
        br#"{"format":{"duration":"-1"}}"#,
    ] {
        assert_eq!(measured_duration(bytes), None);
    }
}
