//! Large and compressed feed transfers, independent of an external TV provider.

use super::*;
use std::io::{
    Read,
    Write,
};

fn serve(
    mut file: std::fs::File,
    encoding: &str,
    chunked: bool,
) -> (reqwest::Url, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/feed", listener.local_addr().unwrap())
        .parse()
        .unwrap();
    let encoding = encoding.to_owned();
    let task = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .unwrap();
        let mut request = [0; 8192];
        assert!(socket.read(&mut request).unwrap() > 0);
        write!(socket, "HTTP/1.1 200 OK\r\nConnection: close\r\n{encoding}").unwrap();
        if chunked {
            write!(socket, "Transfer-Encoding: chunked\r\n\r\n").unwrap();
        } else {
            write!(
                socket,
                "Content-Length: {}\r\n\r\n",
                file.metadata().unwrap().len()
            )
            .unwrap();
        }
        file.rewind().unwrap();
        let mut buffer = [0; 64 * 1024];
        loop {
            let count = file.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            if chunked {
                write!(socket, "{count:x}\r\n").unwrap();
            }
            socket.write_all(&buffer[..count]).unwrap();
            if chunked {
                socket.write_all(b"\r\n").unwrap();
            }
        }
        if chunked {
            socket.write_all(b"0\r\n\r\n").unwrap();
        }
    });
    (url, task)
}

#[rocket::async_test]
async fn xmltv_downloads_exceed_64_mib_with_plain_and_both_gzip_transports() {
    let mut file = tempfile::tempfile().unwrap();
    let now = chrono::Utc::now();
    let stop = now + chrono::Duration::hours(1);
    write!(
        file,
        "<tv><channel id='station'><display-name>TV</display-name></channel><programme \
         channel='station' start='{} +0000' stop='{} +0000'><title>Large feed</title><desc>News \
         &amp; weather</desc></programme>",
        now.format("%Y%m%d%H%M%S"),
        stop.format("%Y%m%d%H%M%S")
    )
    .unwrap();
    // Repeated, modest-sized entries exercise streaming without a huge XML token.
    let padding = format!("<!--{}-->", "x".repeat(1017)).repeat(64);
    for _ in 0..1040 {
        file.write_all(padding.as_bytes()).unwrap();
    }
    file.write_all(b"</tv>").unwrap();
    assert!(file.metadata().unwrap().len() > 64 * 1024 * 1024);
    let compressed = tempfile::tempfile().unwrap();
    let mut encoder = flate2::write::GzEncoder::new(compressed, flate2::Compression::fast());
    file.rewind().unwrap();
    std::io::copy(&mut file, &mut encoder).unwrap();
    let compressed = encoder.finish().unwrap();
    for (input, encoding, chunked) in [
        (&file, "", false),
        (&compressed, "", true),
        (&compressed, "Content-Encoding: gzip\r\n", true),
    ] {
        let (url, task) = serve(input.try_clone().unwrap(), encoding, chunked);
        let guide = guide(url).await.unwrap();
        task.join().unwrap();
        assert_eq!(guide.programs.len(), 1);
        assert_eq!(
            guide.programs[0].metadata.description.as_deref(),
            Some("News & weather")
        );
        assert_eq!(guide.stations[0].guide_key, "station");
    }
}

#[rocket::async_test]
async fn gzip_upload_reader_handles_members_and_rejects_a_truncated_trailer() {
    let mut file = tempfile::tempfile().unwrap();
    for part in [
        b"<tv><channel id='station'>".as_slice(),
        b"<display-name>TV</display-name></channel></tv>",
    ] {
        let mut encoder = flate2::write::GzEncoder::new(&mut file, flate2::Compression::fast());
        encoder.write_all(part).unwrap();
        encoder.finish().unwrap();
    }
    let guide = read_guide(file.try_clone().unwrap()).await.unwrap();
    assert_eq!(guide.stations.len(), 1);
    file.set_len(file.metadata().unwrap().len() - 4).unwrap();
    assert!(read_guide(file).await.is_err());
}

#[test]
fn xmltv_has_no_program_or_document_node_count_cap() {
    let mut file = std::io::BufWriter::new(tempfile::tempfile().unwrap());
    file.write_all(b"<tv>").unwrap();
    let now = chrono::Utc::now();
    for index in 0..250_001 {
        let start = now + chrono::Duration::seconds(index);
        let stop = start + chrono::Duration::seconds(1);
        write!(
            file,
            "<programme channel='station' start='{} +0000' stop='{} +0000'><title>Program \
             {index}</title><category>Sports</category><category>TV</category><category>News</\
             category><category>Movie</category><category>Kids</category><desc>Guide \
             entry</desc></programme>",
            start.format("%Y%m%d%H%M%S"),
            stop.format("%Y%m%d%H%M%S")
        )
        .unwrap();
    }
    file.write_all(b"</tv>").unwrap();
    let file = file.into_inner().unwrap();
    let guide = formats::read_guide(reader(file).unwrap(), Some(now.timestamp())).unwrap();
    assert_eq!(guide.programs.len(), 250_001);
    assert_eq!(guide.programs.last().unwrap().title, "Program 250000");
}

#[test]
fn playlists_have_no_20_000_channel_cap_and_keep_duplicate_detection() {
    let mut file = std::io::BufWriter::new(tempfile::tempfile().unwrap());
    file.write_all(b"\xef\xbb\xbf\n\n#EXTM3U\n").unwrap();
    for index in 0..20_001 {
        writeln!(
            file,
            "#EXTINF:-1 tvg-id=\"{index}\",TV {index}\n/{index}.ts"
        )
        .unwrap();
    }
    file.write_all(b"#EXTINF:-1 tvg-id=\"0\",TV 0\n/0.ts\n")
        .unwrap();
    let channels = formats::read_m3u(
        reader(file.into_inner().unwrap()).unwrap(),
        "https://example.com/tv/list.m3u",
    )
    .unwrap();
    assert_eq!(channels.len(), 20_001);
    assert_eq!(
        channels.last().unwrap().stream_url,
        "https://example.com/20000.ts"
    );
}

#[test]
fn xmltv_rejects_incomplete_documents_custom_entities_and_multiple_roots() {
    for xml in [
        "<tv><channel id='a'/>",
        "<tv><channel id='a'></tv>",
        "<tv/><tv/>",
        "<html/>",
        "<tv><channel id='a' id='b'/></tv>",
        "<!DOCTYPE tv [<!ENTITY x SYSTEM 'file:///private'>]><tv/>",
        "<tv><ignored>&custom;</ignored></tv>",
        "<tv/><?xml version='1.0'?>",
        "<tv/>trailing text",
    ] {
        assert!(formats::read_guide(xml.as_bytes(), None).is_err(), "{xml}");
    }
    let xml = concat!(
        "<!DOCTYPE tv SYSTEM 'https://example.invalid/xmltv.dtd'>",
        "<tv><channel id='a'><display-name><![CDATA[TV & news]]></display-name></channel></tv>"
    )
    .as_bytes();
    assert_eq!(
        formats::read_guide(xml, None).unwrap().stations[0].name,
        "TV & news"
    );
}

#[test]
fn streamed_entries_keep_root_namespaces_and_infer_stops_across_retention_boundaries() {
    let now = chrono::Utc::now();
    let start = now + chrono::Duration::seconds(crate::live_tv::retention::FUTURE - 60);
    let next = start + chrono::Duration::seconds(120);
    let old = now - chrono::Duration::seconds(crate::live_tv::retention::HISTORY + 3600);
    let stamp = |time: chrono::DateTime<chrono::Utc>| time.format("%Y%m%d%H%M%S +0000").to_string();
    let xml = format!(
        "<tv xmlns:extra='https://example.com/guide'><programme channel='s' start='{}' \
         stop='{}'><title>Outside</title></programme><programme channel='s' \
         start='{}'><title>Retained</title><desc extra:format='text'>A &amp; \
         B</desc></programme><programme channel='s' start='{}' \
         stop='{}'><title>Expired</title></programme></tv>",
        stamp(next),
        stamp(next + chrono::Duration::hours(1)),
        stamp(start),
        stamp(old),
        stamp(old + chrono::Duration::minutes(30))
    );
    let guide = formats::read_guide(xml.as_bytes(), Some(now.timestamp())).unwrap();
    assert_eq!(guide.programs.len(), 1);
    assert_eq!(guide.programs[0].title, "Retained");
    assert_eq!(guide.programs[0].ends_at, next.timestamp());
    assert_eq!(
        guide.programs[0].metadata.description.as_deref(),
        Some("A & B")
    );
}

#[rocket::async_test]
#[ignore = "Requires KOKO_TEST_M3U_URL and KOKO_TEST_XMLTV_URL for a real provider"]
async fn provider_feed_import_round_trip() {
    use crate::db::schema::live_tv_programs as programs;
    use diesel::prelude::*;
    let started = std::time::Instant::now();
    let channels = playlist(std::env::var("KOKO_TEST_M3U_URL").unwrap().parse().unwrap())
        .await
        .unwrap();
    let guide = guide(
        std::env::var("KOKO_TEST_XMLTV_URL")
            .unwrap()
            .parse()
            .unwrap(),
    )
    .await
    .unwrap();
    assert!(!channels.is_empty());
    assert!(!guide.programs.is_empty());
    let mut conn = crate::live_tv::tests::database();
    crate::live_tv::storage::save_channels(&mut conn, 1, &channels).unwrap();
    crate::live_tv::storage::save_guide(&mut conn, 1, &guide).unwrap();
    let count = programs::table
        .filter(programs::source_id.eq(1))
        .count()
        .get_result::<i64>(&mut conn)
        .unwrap();
    assert_eq!(count as usize, guide.programs.len());
    eprintln!(
        "Imported {} channels, {} stations, {count} programs in {:.1}s",
        channels.len(),
        guide.stations.len(),
        started.elapsed().as_secs_f64()
    );
}
