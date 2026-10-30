//! Bounded LAN discovery using HDHomeRun UDP, NextPVR UDP, and Tvheadend DNS-SD advertisements.

use rocket_okapi::okapi::schemars::{
    self,
    JsonSchema,
};
use serde::Serialize;
use std::{
    collections::{
        HashMap,
        HashSet,
    },
    net::{
        Ipv4Addr,
        SocketAddr,
    },
    time::{
        Duration,
        Instant,
    },
};
use tokio::net::UdpSocket;

#[derive(Clone, Serialize, JsonSchema)]
pub(crate) struct DiscoveredSource {
    pub name: String,
    pub url: String,
    pub capacity: i32,
}

fn packet(
    payload: &[u8],
    kind: u16,
) -> Vec<u8> {
    let mut bytes = Vec::from(kind.to_be_bytes());
    bytes.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    bytes.extend_from_slice(payload);
    bytes.extend_from_slice(&crc32fast::hash(&bytes).to_le_bytes());
    bytes
}

fn hdhomerun(
    bytes: &[u8],
    peer: SocketAddr,
) -> Option<DiscoveredSource> {
    if bytes.len() < 8 || bytes[..2] != [0, 3] {
        return None;
    }
    let length = u16::from_be_bytes([bytes[2], bytes[3]]) as usize;
    if bytes.len() != length + 8
        || crc32fast::hash(&bytes[..length + 4]).to_le_bytes() != bytes[length + 4..]
    {
        return None;
    }
    let mut payload = &bytes[4..length + 4];
    let mut id = None;
    let mut capacity = 1;
    let mut base = None;
    while payload.len() >= 2 {
        let tag = payload[0];
        let mut size = usize::from(payload[1] & 0x7f);
        let mut offset = 2;
        if payload[1] & 0x80 != 0 {
            if payload.len() < 3 {
                return None;
            }
            size |= usize::from(payload[2]) << 7;
            offset = 3;
        }
        let data = payload.get(offset..offset + size)?;
        match tag {
            2 if data.len() == 4 => id = Some(u32::from_be_bytes(data.try_into().ok()?)),
            0x10 if data.len() == 1 => capacity = i32::from(data[0]).clamp(1, 64),
            0x2a => {
                base = std::str::from_utf8(data)
                    .ok()
                    .map(|s| s.trim_end_matches('\0').to_owned())
            }
            _ => {}
        }
        payload = &payload[offset + size..];
    }
    let id = id?;
    let url = base
        .and_then(|s| super::formats::http_url(&s).ok())
        .filter(|u| {
            u.host_str()
                .is_some_and(|s| s.parse::<std::net::IpAddr>().ok() == Some(peer.ip()))
        })
        .map(|u| u.to_string())
        .unwrap_or_else(|| format!("http://{}/", peer.ip()));
    Some(DiscoveredSource {
        name: format!("HDHomeRun {id:08X}"),
        url,
        capacity,
    })
}

fn nextpvr(
    bytes: &[u8],
    peer: SocketAddr,
) -> Option<DiscoveredSource> {
    let value = std::str::from_utf8(bytes).ok()?.trim_matches('\0').trim();
    if value.is_empty() {
        return None;
    }
    // Replies from older servers vary; the sender address remains authoritative for Docker/NAT.
    let mut name = format!("NextPVR ({})", peer.ip());
    let mut port = 8866;
    let mut scheme = "http".to_owned();
    if let Ok(doc) = roxmltree::Document::parse(value) {
        for node in doc.descendants().filter(roxmltree::Node::is_element) {
            match node.tag_name().name().to_ascii_lowercase().as_str() {
                "name" | "server_name" | "servername" => {
                    if let Some(text) = node.text().filter(|s| s.len() <= 200) {
                        name = text.into();
                    }
                }
                "port" | "web_port" | "webport" => {
                    if let Some(number) = node
                        .text()
                        .and_then(|s| s.parse::<u16>().ok())
                        .filter(|p| *p > 0)
                    {
                        port = number;
                    }
                }
                _ => {}
            }
        }
    }
    static URL: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| regex::Regex::new(r#"https?://[^\s<>\"']+"#).unwrap());
    if let Some(address) = URL
        .find(value)
        .and_then(|m| reqwest::Url::parse(m.as_str()).ok())
    {
        port = address.port_or_known_default().unwrap_or(port);
        scheme = address.scheme().to_owned();
    }
    Some(DiscoveredSource {
        name,
        url: format!("{scheme}://{}:{port}/", peer.ip()),
        capacity: 1,
    })
}

async fn udp(kind: &str) -> Result<Vec<DiscoveredSource>, String> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .await
        .map_err(|_| "Unable to start network discovery; enter the source address manually")?;
    socket
        .set_broadcast(true)
        .map_err(|_| "Network broadcasts are unavailable; enter the source address manually")?;
    let (port, request) = if kind == "hdhomerun" {
        (
            65001,
            packet(
                &[
                    1, 4, 0, 0, 0, 1, 2, 4, 255, 255, 255, 255,
                ],
                2,
            ),
        )
    } else {
        (16891, b"DISCOVER_REQUEST".to_vec())
    };
    let mut found = HashMap::new();
    let mut buffer = vec![0; 65536];
    for _ in 0..2 {
        socket
            .send_to(&request, (Ipv4Addr::BROADCAST, port))
            .await
            .map_err(|_| {
                "Discovery broadcast failed; check the network or enter the address manually"
            })?;
        // A PVR server on this computer may not receive its own subnet broadcast.
        let _ = socket.send_to(&request, (Ipv4Addr::LOCALHOST, port)).await;
        let end = tokio::time::Instant::now() + Duration::from_millis(1200);
        while let Ok(Ok((size, peer))) =
            tokio::time::timeout_at(end, socket.recv_from(&mut buffer)).await
        {
            let item = if kind == "hdhomerun" {
                hdhomerun(&buffer[..size], peer)
            } else {
                nextpvr(&buffer[..size], peer)
            };
            if let Some(item) = item {
                found.insert(item.url.clone(), item);
            }
            if found.len() >= 128 {
                break;
            }
        }
    }
    let mut result = found.into_values().collect::<Vec<_>>();
    result.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(result)
}

fn tvheadend() -> Result<Vec<DiscoveredSource>, String> {
    use mdns_sd::{
        ServiceDaemon,
        ServiceEvent,
    };
    struct Daemon(ServiceDaemon);
    impl Drop for Daemon {
        fn drop(&mut self) {
            let _ = self.0.shutdown();
        }
    }
    let daemon = Daemon(ServiceDaemon::new().map_err(|_| {
        "Local DNS-SD discovery is unavailable; enter the Tvheadend address manually"
    })?);
    let http = daemon
        .0
        .browse("_http._tcp.local.")
        .map_err(|_| "Unable to search for Tvheadend HTTP services")?;
    let htsp = daemon
        .0
        .browse("_htsp._tcp.local.")
        .map_err(|_| "Unable to search for Tvheadend services")?;
    let mut web = HashMap::new();
    let mut names = HashSet::new();
    let end = Instant::now() + Duration::from_secs(4);
    while Instant::now() < end {
        for (receiver, is_http) in [(&http, true), (&htsp, false)] {
            for _ in 0..256 {
                let Ok(event) = receiver.try_recv() else {
                    break;
                };
                if let ServiceEvent::ServiceResolved(info) = event {
                    let key = info
                        .fullname
                        .split("._")
                        .next()
                        .unwrap_or("")
                        .to_lowercase();
                    if !is_http {
                        names.insert(key);
                        continue;
                    }
                    let Some(address) = info.addresses.iter().find(|ip| ip.is_ipv4()) else {
                        continue;
                    };
                    let mut url = reqwest::Url::parse(&format!("http://{address}:{}/", info.port))
                        .map_err(|_| "Invalid Tvheadend advertisement")?;
                    if let Some(path) = info.txt_properties.get_property_val_str("path") {
                        url.set_path(path);
                    }
                    web.insert(
                        key,
                        DiscoveredSource {
                            name: info
                                .fullname
                                .split("._")
                                .next()
                                .unwrap_or("Tvheadend")
                                .into(),
                            url: url.to_string(),
                            capacity: 1,
                        },
                    );
                }
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let mut result = web
        .into_iter()
        .filter(|(key, _)| names.contains(key) || key.contains("tvheadend"))
        .map(|(_, value)| value)
        .collect::<Vec<_>>();
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result.truncate(128);
    Ok(result)
}

pub(crate) async fn discover(kind: &str) -> Result<Vec<DiscoveredSource>, String> {
    match kind {
        "hdhomerun" | "nextpvr" => udp(kind).await,
        "tvheadend" => tokio::task::spawn_blocking(tvheadend)
            .await
            .map_err(|_| "Tvheadend discovery stopped")?,
        _ => Err(
            "Discovery supports HDHomeRun, NextPVR, and Tvheadend; enter an IPTV playlist URL \
             manually"
                .into(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hdhr_discovery_checks_crc_and_retains_device_capacity() {
        let reply = packet(
            &[
                2, 4, 0x10, 0x12, 0x34, 0x56, 0x10, 1, 4,
            ],
            3,
        );
        let peer = "192.168.1.20:65001".parse().unwrap();
        let found = hdhomerun(&reply, peer).unwrap();
        assert_eq!(found.capacity, 4);
        assert_eq!(found.url, "http://192.168.1.20/");
        let mut broken = reply;
        broken[8] ^= 1;
        assert!(hdhomerun(&broken, peer).is_none());
    }
    #[test]
    fn nextpvr_uses_the_reachable_sender_and_advertised_port() {
        let found = nextpvr(
            b"<server><name>Living room</name><port>8867</port></server>",
            "192.168.1.30:16891".parse().unwrap(),
        )
        .unwrap();
        assert_eq!(found.name, "Living room");
        assert_eq!(found.url, "http://192.168.1.30:8867/");
    }
}
