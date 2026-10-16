use diesel::connection::SimpleConnection;
use diesel::{
    Connection,
    SqliteConnection,
};
use koko::config::MetadataProviderId;
use koko::metadata::{
    StoredMetadataSnapshot,
    get_item_metadata_summaries,
    set_item_metadata_refresh_state_for_locale,
    upsert_item_metadata_snapshot,
};
use std::io::{
    Read,
    Write,
};

fn connection() -> SqliteConnection {
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    koko::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    conn.batch_execute(
        "INSERT INTO media_libraries (id,name,path,kind) VALUES (1,'Review','review','movies'); \
         INSERT INTO media_items (id,library_id,identity_key,item_type,display_title) VALUES \
         (1,1,'review','movie','Movie');",
    )
    .unwrap();
    conn
}

fn snapshot(locale: &str) -> StoredMetadataSnapshot {
    StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tmdb,
        external_id: "603".into(),
        media_type: Some("movie".into()),
        title: Some("Movie".into()),
        overview: None,
        artwork_url: None,
        backdrop_url: None,
        release_year: Some(1999),
        locale_key: locale.into(),
        provider_locale_key: Some(locale.into()),
        provider_payload_json: None,
    }
}

fn artwork_dir() -> std::path::PathBuf {
    let path =
        std::env::temp_dir().join(format!("koko-metadata-artwork-{}", rand::random::<u64>()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn png() -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::RgbImage::new(1, 1)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

fn serve_artwork(
    status: u16,
    body: Vec<u8>,
) -> (String, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/poster.png", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0; 2048];
        stream.read(&mut buffer).unwrap();
        write!(
            stream,
            "HTTP/1.1 {status} Result\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(&body).unwrap();
    });
    (url, server)
}

#[tokio::test]
async fn metadata_artwork_rejects_http_errors_and_invalid_images() {
    let dir = artwork_dir();
    for status in [404, 503, 200] {
        let (url, server) = serve_artwork(status, b"<html>unavailable</html>".to_vec());
        assert!(
            koko::metadata::cache_artwork(&url, &dir, "poster")
                .await
                .is_err()
        );
        server.join().unwrap();
        assert!(!koko::metadata::expected_artwork_cache_path(&url, &dir, "poster").exists());
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn metadata_artwork_refetches_previously_corrupted_cache_entries() {
    let dir = artwork_dir();
    let bytes = png();
    let (url, server) = serve_artwork(200, bytes.clone());
    let path = koko::metadata::expected_artwork_cache_path(&url, &dir, "poster");
    std::fs::write(&path, b"old cached 404").unwrap();
    assert_eq!(
        koko::metadata::cache_artwork(&url, &dir, "poster")
            .await
            .unwrap(),
        path
    );
    server.join().unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn metadata_artwork_keeps_working_images_until_a_replacement_is_ready() {
    let dir = artwork_dir();
    let old = koko::metadata::expected_artwork_cache_path(
        "http://old.example/poster.png",
        &dir,
        "poster",
    );
    std::fs::write(&old, png()).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let unavailable = format!("http://{}/poster.png", listener.local_addr().unwrap());
    drop(listener);
    assert!(
        koko::metadata::cache_artwork(&unavailable, &dir, "poster")
            .await
            .is_err()
    );
    assert!(old.exists());
    let (url, server) = serve_artwork(200, png());
    let replacement = koko::metadata::cache_artwork(&url, &dir, "poster")
        .await
        .unwrap();
    server.join().unwrap();
    assert!(replacement.is_file());
    assert!(!old.exists());
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn metadata_refresh_state_changes_only_the_requested_translation() {
    let mut conn = connection();
    for locale in ["en-US", "es-ES"] {
        upsert_item_metadata_snapshot(&mut conn, 1, &snapshot(locale)).unwrap();
    }
    set_item_metadata_refresh_state_for_locale(
        &mut conn,
        1,
        MetadataProviderId::Tmdb,
        "603",
        Some("movie"),
        "es-ES",
        "error",
        Some("failed"),
    )
    .unwrap();
    let rows = get_item_metadata_summaries(&mut conn, 1).unwrap();
    assert_eq!(
        rows.iter()
            .find(|row| row.locale_key == "en-US")
            .unwrap()
            .refresh_state,
        "fresh"
    );
    assert_eq!(
        rows.iter()
            .find(|row| row.locale_key == "es-ES")
            .unwrap()
            .refresh_state,
        "error"
    );
    set_item_metadata_refresh_state_for_locale(
        &mut conn,
        1,
        MetadataProviderId::Tmdb,
        "603",
        Some("movie"),
        "fr-FR",
        "pending",
        None,
    )
    .unwrap();
    let rows = get_item_metadata_summaries(&mut conn, 1).unwrap();
    assert_eq!(
        rows.iter()
            .find(|row| row.locale_key == "fr-FR")
            .unwrap()
            .refresh_state,
        "pending"
    );
}

#[test]
fn failed_metadata_refreshes_do_not_starve_later_due_items() {
    let mut conn = connection();
    for id in 1..=9 {
        if id > 1 {
            conn.batch_execute(&format!(
                "INSERT INTO media_items (id,library_id,identity_key,item_type,display_title) \
                 VALUES ({id},1,'movie-{id}','movie','Movie');"
            ))
            .unwrap();
        }
        upsert_item_metadata_snapshot(&mut conn, id, &snapshot("en-US")).unwrap();
    }
    conn.batch_execute("UPDATE item_metadata_links SET next_refresh_at = 0")
        .unwrap();
    let now = koko::utils::current_timestamp();
    for row in koko::metadata::list_due_item_metadata_links(&mut conn, now, 8).unwrap() {
        set_item_metadata_refresh_state_for_locale(
            &mut conn,
            row.media_item_id,
            MetadataProviderId::Tmdb,
            "603",
            Some("movie"),
            "en-US",
            "error",
            Some("404"),
        )
        .unwrap();
    }
    let next = koko::metadata::list_due_item_metadata_links(&mut conn, now, 8).unwrap();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].media_item_id, 9);
    assert_eq!(
        koko::metadata::list_due_item_metadata_links(&mut conn, now + 301, 9)
            .unwrap()
            .len(),
        9
    );
}
