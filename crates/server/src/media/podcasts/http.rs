//! Public-network-only HTTP with DNS pinning, bounded redirects, deadlines, and response limits.
use reqwest::{
    Client,
    Response,
    Url,
};
use std::{
    net::{
        IpAddr,
        Ipv4Addr,
    },
    time::Duration,
};

pub(crate) fn normalize_url(value: &str) -> Result<String, String> {
    let mut url = Url::parse(value.trim()).map_err(|_| "Enter a valid podcast feed URL")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Use an HTTP or HTTPS feed URL without embedded login credentials".into());
    }
    url.set_fragment(None);
    Ok(url.to_string())
}

fn public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !ip.is_private()
        && !ip.is_loopback()
        && !ip.is_link_local()
        && !ip.is_broadcast()
        && !ip.is_documentation()
        && !ip.is_multicast()
        && a != 0
        && a < 240
        && !(a == 100 && (64..=127).contains(&b))
        && !(a == 198 && (18..=19).contains(&b))
        && !(a == 192 && b == 0 && c == 0)
}

pub(super) fn public_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => public_v4(ip),
        IpAddr::V6(ip) => {
            if let Some(v4) = ip.to_ipv4_mapped() {
                return public_v4(v4);
            }
            let segments = ip.segments();
            // Permit global unicast; exclude transition, documentation and special-use ranges.
            (segments[0] & 0xe000) == 0x2000
                && segments[0] != 0x2002
                && !(segments[0] == 0x2001 && (segments[1] < 0x0200 || segments[1] == 0x0db8))
        }
    }
}

/// Resolve and validate every redirect separately, pinning the connection to those addresses.
pub(crate) async fn request(
    value: &str,
    range: Option<&str>,
) -> Result<Response, String> {
    tokio::time::timeout(
        Duration::from_secs(30),
        request_with_policy(value, range, public_address),
    )
    .await
    .map_err(|_| "Podcast request timed out".to_string())?
}

async fn request_with_policy(
    value: &str,
    range: Option<&str>,
    address_allowed: fn(IpAddr) -> bool,
) -> Result<Response, String> {
    let mut url = Url::parse(&normalize_url(value)?).map_err(|_| "Invalid podcast URL")?;
    for _ in 0..=10 {
        let host = url
            .host_str()
            .ok_or("Invalid podcast host")?
            .trim_matches(['[', ']']);
        let port = url.port_or_known_default().ok_or("Invalid podcast port")?;
        let addresses = tokio::time::timeout(
            Duration::from_secs(10),
            tokio::net::lookup_host((host, port)),
        )
        .await
        .map_err(|_| "Podcast host lookup timed out")?
        .map_err(|_| "Podcast host could not be resolved")?
        .collect::<Vec<_>>();
        if addresses.is_empty() || addresses.iter().any(|addr| !address_allowed(addr.ip())) {
            return Err("Podcast URLs must use a public internet address".into());
        }
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(30))
            .user_agent(concat!("Koko/", env!("CARGO_PKG_VERSION")))
            .resolve_to_addrs(host, &addresses)
            .build()
            .map_err(|_| "Unable to create podcast client")?;
        let mut request = client.get(url.clone());
        if let Some(range) = range {
            request = request.header(reqwest::header::RANGE, range);
        }
        let response = request
            .send()
            .await
            .map_err(|_| "Podcast publisher could not be reached")?;
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or("Invalid podcast redirect")?;
            let next = url.join(location).map_err(|_| "Invalid podcast redirect")?;
            if url.scheme() == "https" && next.scheme() != "https" {
                return Err("Podcast publisher redirected to an insecure connection".into());
            }
            url = Url::parse(&normalize_url(next.as_str())?)
                .map_err(|_| "Invalid podcast redirect")?;
            continue;
        }
        if !response.status().is_success()
            && response.status() != reqwest::StatusCode::RANGE_NOT_SATISFIABLE
        {
            return Err(format!(
                "Podcast publisher returned HTTP {}",
                response.status().as_u16()
            ));
        }
        return Ok(response);
    }
    Err("Podcast publisher redirected too many times".into())
}

pub(super) async fn fetch_text(
    url: &str,
    limit: usize,
) -> Result<(String, String), String> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let mut response = request(url, None).await?;
        let canonical = response.url().to_string();
        if response.content_length().is_some_and(|n| n > limit as u64) {
            return Err("Podcast response is too large".into());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "Unable to read podcast response")?
        {
            if bytes.len() + chunk.len() > limit {
                return Err("Podcast response is too large".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let text = String::from_utf8(bytes).map_err(|_| "Podcast feed must use UTF-8")?;
        Ok((canonical, text))
    })
    .await
    .map_err(|_| "Podcast request timed out".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{
        AsyncReadExt,
        AsyncWriteExt,
    };

    #[tokio::test]
    async fn publisher_tracking_redirects_preserve_ranges_through_six_hops() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let publisher = tokio::spawn(async move {
            for hop in 0..=6 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buffer = [0; 4096];
                let count = socket.read(&mut buffer).await.unwrap();
                let request = String::from_utf8_lossy(&buffer[..count]).to_lowercase();
                assert!(request.contains("range: bytes=0-3"));
                let response = if hop < 6 {
                    format!(
                        "HTTP/1.1 302 Found\r\nLocation: /{}\r\nContent-Length: 0\r\n\r\n",
                        hop + 1
                    )
                } else {
                    "HTTP/1.1 206 Partial Content\r\nContent-Type: audio/mpeg\r\nContent-Length: \
                     4\r\nContent-Range: bytes 0-3/100\r\n\r\nID3!"
                        .into()
                };
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });
        // Only this test transport permits loopback; production always uses public_address.
        let response =
            request_with_policy(&format!("http://{address}/0"), Some("bytes=0-3"), |_| true)
                .await
                .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::PARTIAL_CONTENT);
        assert_eq!(response.headers()["content-range"], "bytes 0-3/100");
        assert_eq!(response.bytes().await.unwrap().as_ref(), b"ID3!");
        publisher.await.unwrap();
    }
}
