//! Shared, bounded HTTP policy for metadata services and artwork downloads.
use once_cell::sync::Lazy;
use std::time::Duration;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| build_client(REQUEST_TIMEOUT));

fn build_client(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(timeout.min(Duration::from_secs(10)))
        .timeout(timeout)
        .user_agent(concat!("Koko/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("Failed to build metadata HTTP client")
}

pub(crate) fn client() -> &'static reqwest::Client {
    &CLIENT
}

#[cfg(test)]
mod tests {
    use std::io::{
        Read,
        Write,
    };

    #[tokio::test]
    async fn request_deadline_includes_a_stalled_response_body() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (release, wait) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0; 2048];
            stream.read(&mut buffer).unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\n")
                .unwrap();
            wait.recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
        });
        let client = super::build_client(std::time::Duration::from_millis(100));
        let response = client
            .get(format!("http://{address}/"))
            .send()
            .await
            .unwrap();
        let result = response.bytes().await;
        release.send(()).unwrap();
        server.join().unwrap();
        assert!(result.unwrap_err().is_timeout());
    }
}
