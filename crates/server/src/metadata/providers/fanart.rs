//! Music artwork from <https://api.fanart.tv/> using matched MusicBrainz identifiers.
use serde::Deserialize;
use std::time::Duration;

use super::{
    MetadataProvider,
    MetadataProviderFuture,
};
use crate::config::{
    MediaLibraryKind,
    MetadataProviderId,
    MetadataSettings,
};
use crate::media::valid_mbid;
use crate::metadata::{
    MetadataProviderDescriptor,
    MetadataProviderRole,
    ProviderMetadataDetails,
};

const API_BASE: &str = "https://webservice.fanart.tv/v3.2";

pub(crate) fn project_api_key() -> Option<&'static str> {
    option_env!("KOKO_FANART_PROJECT_API_KEY")
        .map(str::trim)
        .filter(|key| !key.is_empty())
}

pub(super) struct FanartMetadataProvider;

impl MetadataProvider for FanartMetadataProvider {
    fn descriptor(&self) -> MetadataProviderDescriptor {
        MetadataProviderDescriptor {
            id: MetadataProviderId::Fanart,
            display_name: "Fanart.tv".into(),
            description: "Artist portraits, backgrounds, logos, and album covers for MusicBrainz \
                          music libraries."
                .into(),
            supported_kinds: vec![MediaLibraryKind::Music],
            requires_api_key: true,
            implemented: true,
            role: MetadataProviderRole::Secondary,
            extends_provider_ids: vec![MetadataProviderId::MusicBrainz],
            attribution_text: "Music artwork provided by Fanart.tv.".into(),
            attribution_url: "https://fanart.tv/".into(),
            logo_light_url: Some("/assets/providers/fanart.png".into()),
            logo_dark_url: Some("/assets/providers/fanart.png".into()),
        }
    }

    fn secondary_metadata_reference_priority(
        &self,
        source_provider_id: &MetadataProviderId,
        item_type: &str,
        database_id: &str,
    ) -> Option<usize> {
        (*source_provider_id == MetadataProviderId::MusicBrainz
            && matches!(
                (item_type, database_id),
                ("artist", "musicbrainz") | ("album", "musicbrainz_release_group")
            ))
        .then_some(0)
    }

    fn fetch_secondary_metadata<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        item_type: &'a str,
        database_id: &'a str,
        external_id: &'a str,
        locale_key: &'a str,
    ) -> MetadataProviderFuture<'a, Option<ProviderMetadataDetails>> {
        Box::pin(async move {
            let Some(path) = lookup_path(item_type, database_id, external_id) else {
                return Ok(None);
            };
            let provider =
                crate::metadata::provider_settings(settings, MetadataProviderId::Fanart)?;
            let cache_key = crate::metadata::metadata_response_cache_key(
                &MetadataProviderId::Fanart,
                "artwork",
                &[
                    &path,
                    project_api_key().unwrap_or_default(),
                    provider.api_key.as_deref().unwrap_or_default(),
                ],
            );
            if let Some(payload) = crate::metadata::read_metadata_response_cache_text(&cache_key) {
                if let Ok(details) = parse_artwork(&payload, item_type, external_id, locale_key) {
                    return Ok(details);
                }
            }
            let payload = request(
                &format!("{API_BASE}/{path}"),
                project_api_key(),
                provider.api_key.as_deref(),
            )
            .await?;
            let Some(payload) = payload else {
                return Ok(None);
            };
            let details = parse_artwork(&payload, item_type, external_id, locale_key)?;
            crate::metadata::write_metadata_response_cache_text(&cache_key, &payload);
            Ok(details)
        })
    }
}

fn lookup_path(
    item_type: &str,
    database_id: &str,
    external_id: &str,
) -> Option<String> {
    if !valid_mbid(external_id) {
        return None;
    }
    match (item_type, database_id) {
        ("artist", "musicbrainz") => Some(format!("music/{external_id}")),
        ("album", "musicbrainz_release_group") => Some(format!("music/albums/{external_id}")),
        _ => None,
    }
}

fn artwork_request(
    url: &str,
    project_key: Option<&str>,
    personal_key: Option<&str>,
) -> Result<reqwest::RequestBuilder, String> {
    let mut request = crate::metadata::http::client().get(url);
    for (name, key) in [
        ("api-key", project_key),
        ("client-key", personal_key),
    ] {
        if let Some(key) = key.filter(|key| !key.is_empty()) {
            let mut secret = reqwest::header::HeaderValue::from_str(key)
                .map_err(|_| "Fanart.tv API key is not a valid header value".to_string())?;
            secret.set_sensitive(true);
            request = request.header(name, secret);
        }
    }
    Ok(request)
}

async fn request(
    url: &str,
    project_key: Option<&str>,
    personal_key: Option<&str>,
) -> Result<Option<String>, String> {
    for attempt in 0..3 {
        // Personal keys take priority, and neither key appears in URLs or errors.
        let response = artwork_request(url, project_key, personal_key)?
            .send()
            .await
            .map_err(|error| format!("Fanart.tv request failed: {error}"))?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if attempt < 2 && matches!(status.as_u16(), 429 | 503) {
            let delay = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(1 << attempt)
                .min(60);
            tokio::time::sleep(Duration::from_secs(delay)).await;
            continue;
        }
        if !status.is_success() {
            return Err(format!(
                "Fanart.tv artwork lookup failed with status {status}"
            ));
        }
        return response
            .text()
            .await
            .map(Some)
            .map_err(|error| format!("Invalid Fanart.tv response: {error}"));
    }
    unreachable!()
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct MusicResponse {
    mbid_id: Option<String>,
    artistthumb: Vec<Artwork>,
    artistbackground: Vec<Artwork>,
    hdmusiclogo: Vec<Artwork>,
    musiclogo: Vec<Artwork>,
    albums: Vec<Album>,
}

#[derive(Deserialize)]
struct Artwork {
    url: String,
    lang: Option<String>,
    likes: Option<String>,
}

#[derive(Deserialize)]
struct Album {
    release_group_id: String,
    #[serde(default)]
    albumcover: Vec<Artwork>,
}

fn parse_artwork(
    payload: &str,
    item_type: &str,
    external_id: &str,
    locale_key: &str,
) -> Result<Option<ProviderMetadataDetails>, String> {
    let response: MusicResponse = serde_json::from_str(payload)
        .map_err(|error| format!("Invalid Fanart.tv artwork response: {error}"))?;
    if item_type == "artist" && response.mbid_id.as_deref() != Some(external_id) {
        return Err("Fanart.tv returned a different or missing artist identifier".into());
    }
    let covers = match item_type {
        "artist" => &response.artistthumb,
        "album" => {
            let Some(album) = response
                .albums
                .iter()
                .find(|album| album.release_group_id == external_id)
            else {
                return Ok(None);
            };
            &album.albumcover
        }
        _ => return Ok(None),
    };
    let details = ProviderMetadataDetails {
        artwork_url: select_artwork(covers, locale_key),
        backdrop_url: select_artwork(&response.artistbackground, locale_key),
        logo_url: select_artwork(&response.hdmusiclogo, locale_key)
            .or_else(|| select_artwork(&response.musiclogo, locale_key)),
        ..Default::default()
    };
    Ok((details.artwork_url.is_some()
        || details.backdrop_url.is_some()
        || details.logo_url.is_some())
    .then_some(details))
}

fn select_artwork(
    images: &[Artwork],
    locale_key: &str,
) -> Option<String> {
    let language = locale_key.split('-').next().unwrap_or("en");
    images
        .iter()
        .filter_map(|image| {
            let mut url = reqwest::Url::parse(image.url.trim()).ok()?;
            // Older records still contain HTTP CDN URLs; use the HTTPS equivalent.
            if url.host_str() != Some("assets.fanart.tv")
                || !matches!(url.scheme(), "http" | "https")
            {
                return None;
            }
            url.set_scheme("https").ok()?;
            let language_rank = match image.lang.as_deref().unwrap_or("") {
                value if value.eq_ignore_ascii_case(language) => 0,
                "" | "00" => 1,
                "en" => 2,
                _ => 3,
            };
            let likes = image
                .likes
                .as_deref()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0);
            Some(((language_rank, std::cmp::Reverse(likes)), url.to_string()))
        })
        .min_by(|left, right| left.0.cmp(&right.0))
        .map(|(_, url)| url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{
        Read,
        Write,
    };

    const ARTIST: &str = "46510d46-ce20-4dde-b9ea-207e474353f4";
    const ALBUM: &str = "29551412-b9fc-4519-9c65-37f8cf14711e";

    #[test]
    fn credentials_use_sensitive_headers_and_never_urls_or_settings() {
        for (project, personal) in [
            (Some("project-secret"), Some("personal-secret")),
            (Some("project-secret"), None),
            (None, Some("personal-secret")),
        ] {
            let request = artwork_request(API_BASE, project, personal)
                .unwrap()
                .build()
                .unwrap();
            assert!(request.url().query().is_none());
            for (name, expected) in [
                ("api-key", project),
                ("client-key", personal),
            ] {
                assert_eq!(
                    request
                        .headers()
                        .get(name)
                        .map(|value| value.to_str().unwrap()),
                    expected
                );
                if let Some(header) = request.headers().get(name) {
                    assert!(header.is_sensitive());
                }
            }
            let debug = format!("{request:?}");
            assert!(!debug.contains("project-secret"));
            assert!(!debug.contains("personal-secret"));
        }
        let error = artwork_request(API_BASE, Some("bad\nsecret"), None)
            .err()
            .unwrap();
        assert!(!error.contains("bad"));
        let settings = crate::config::settings_for_persistence(&crate::config::Settings::default());
        let serialized = serde_json::to_string(&settings).unwrap();
        if let Some(key) = project_api_key() {
            assert!(!serialized.contains(key));
            let resolved =
                crate::metadata::provider_settings(&settings.metadata, MetadataProviderId::Fanart)
                    .unwrap();
            assert!(resolved.api_key.is_none());
            assert!(
                crate::metadata::list_provider_statuses(&settings.metadata)
                    .iter()
                    .any(
                        |provider| provider.id == MetadataProviderId::Fanart && provider.configured
                    )
            );
        }
    }

    #[test]
    fn artists_use_portraits_backgrounds_and_preferred_language_logos() {
        let payload = serde_json::json!({
            "mbid_id": ARTIST,
            "artistthumb": [
                {"url": "http://assets.fanart.tv/portrait.jpg", "likes": "3"},
                {"url": "https://assets.fanart.tv/popular.jpg", "likes": "14"},
                {"url": "https://example.com/wrong.jpg", "likes": "999"}
            ],
            "artistbackground": [{"url": "https://assets.fanart.tv/background.jpg"}],
            "hdmusiclogo": [
                {"url": "https://assets.fanart.tv/english.png", "lang": "en", "likes": "99"},
                {"url": "https://assets.fanart.tv/spanish.png", "lang": "es"}
            ]
        })
        .to_string();
        let details = parse_artwork(&payload, "artist", ARTIST, "es-ES")
            .unwrap()
            .unwrap();
        assert_eq!(
            details.artwork_url.as_deref(),
            Some("https://assets.fanart.tv/popular.jpg")
        );
        assert_eq!(
            details.backdrop_url.as_deref(),
            Some("https://assets.fanart.tv/background.jpg")
        );
        assert_eq!(
            details.logo_url.as_deref(),
            Some("https://assets.fanart.tv/spanish.png")
        );
        assert!(parse_artwork(&payload, "artist", ALBUM, "en-US").is_err());
        let portrait: Vec<Artwork> = serde_json::from_value(serde_json::json!([
            {"url": "http://assets.fanart.tv/portrait.jpg"}
        ]))
        .unwrap();
        assert_eq!(
            select_artwork(&portrait, "en-US").as_deref(),
            Some("https://assets.fanart.tv/portrait.jpg")
        );
    }

    #[test]
    fn albums_use_the_requested_release_group_and_never_the_artist_portrait() {
        let payload = serde_json::json!({
            "mbid_id": ARTIST,
            "artistthumb": [{"url": "https://assets.fanart.tv/artist.jpg"}],
            "albums": [
                {"release_group_id": ARTIST,
                 "albumcover": [{"url": "https://assets.fanart.tv/wrong.jpg"}]},
                {"release_group_id": ALBUM,
                 "albumcover": [{"url": "https://assets.fanart.tv/album.jpg"}]}
            ]
        })
        .to_string();
        let details = parse_artwork(&payload, "album", ALBUM, "en-US")
            .unwrap()
            .unwrap();
        assert_eq!(
            details.artwork_url.as_deref(),
            Some("https://assets.fanart.tv/album.jpg")
        );
        assert!(
            parse_artwork("{}", "album", ALBUM, "en-US")
                .unwrap()
                .is_none()
        );
        assert!(parse_artwork("<html>error</html>", "artist", ARTIST, "en-US").is_err());
        assert!(parse_artwork(r#"{"albums":false}"#, "album", ALBUM, "en-US").is_err());
        assert!(lookup_path("track", "musicbrainz", ARTIST).is_none());
        assert!(lookup_path("album", "musicbrainz", ALBUM).is_none());
        assert!(lookup_path("artist", "musicbrainz", "../invalid").is_none());
    }

    #[tokio::test]
    async fn http_retries_busy_responses_and_distinguishes_missing_artwork_from_errors() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/music/{ARTIST}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            for status in [
                "429 Too Many Requests",
                "503 Service Unavailable",
                "200 OK",
                "404 Not Found",
                "401 Unauthorized",
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut buffer = [0; 4096];
                let size = stream.read(&mut buffer).unwrap();
                let headers = String::from_utf8_lossy(&buffer[..size]).to_ascii_lowercase();
                assert!(headers.contains("api-key: project-test"));
                assert!(headers.contains("client-key: personal-test"));
                assert!(!headers.lines().next().unwrap().contains("test"));
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: 2\r\nRetry-After: 0\r\nConnection: \
                     close\r\n\r\n{{}}"
                )
                .unwrap();
            }
        });
        assert_eq!(
            request(&url, Some("project-test"), Some("personal-test"))
                .await
                .unwrap(),
            Some("{}".into())
        );
        assert!(
            request(&url, Some("project-test"), Some("personal-test"))
                .await
                .unwrap()
                .is_none()
        );
        let error = request(&url, Some("project-test"), Some("personal-test"))
            .await
            .unwrap_err();
        assert!(error.contains("401"));
        assert!(!error.contains("personal-test") && !error.contains("project-test"));
        server.join().unwrap();
    }

    #[test]
    fn persisted_artwork_uses_matched_ids_and_respects_library_selection() {
        use crate::media::{
            get_item_secondary_provider_references,
            get_media_item,
            list_media_items,
        };
        use crate::metadata::{
            StoredMetadataSnapshot,
            upsert_item_metadata_link,
            upsert_item_metadata_snapshot,
        };
        use diesel::connection::SimpleConnection;
        use diesel::{
            Connection,
            ExpressionMethods,
            QueryDsl,
            RunQueryDsl,
            SqliteConnection,
        };
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        crate::db::run_pending_sqlite_migrations(&mut conn).unwrap();
        conn.batch_execute(
            "INSERT INTO media_libraries (id,name,path,kind,metadata_providers_json)
             VALUES (1,'Music','music','music','[\"musicbrainz\",\"fanart\"]');
             INSERT INTO media_items (id,library_id,parent_id,identity_key,item_type,display_title)
             VALUES (1,1,NULL,'artist','artist','Local Artist'),
                    (2,1,1,'album','album','Local Album'),
                    (3,1,1,'edition','album','Local Edition'),
                    (4,1,2,'track','track','Local Track');",
        )
        .unwrap();
        let mut primary = StoredMetadataSnapshot {
            provider_id: MetadataProviderId::MusicBrainz,
            external_id: ARTIST.into(),
            media_type: Some("artist".into()),
            title: Some("Matched Artist".into()),
            overview: Some("Artist description".into()),
            artwork_url: None,
            backdrop_url: None,
            release_year: None,
            locale_key: "en-US".into(),
            provider_locale_key: None,
            provider_payload_json: None,
        };
        upsert_item_metadata_snapshot(&mut conn, 1, &primary).unwrap();
        primary.external_id = ALBUM.into();
        primary.media_type = Some("release-group".into());
        primary.title = Some("Matched Album".into());
        primary.artwork_url = Some("https://coverartarchive.org/album/front-500".into());
        upsert_item_metadata_snapshot(&mut conn, 2, &primary).unwrap();
        primary.external_id = ARTIST.into();
        primary.media_type = Some("release".into());
        primary.provider_payload_json =
            Some(serde_json::json!({"release-group": {"id": ALBUM}}).to_string());
        upsert_item_metadata_snapshot(&mut conn, 3, &primary).unwrap();
        for (id, kind, database, external_id) in [
            (1, "artist", "musicbrainz", ARTIST),
            (2, "album", "musicbrainz_release_group", ALBUM),
            (3, "album", "musicbrainz_release_group", ALBUM),
        ] {
            assert_eq!(
                get_item_secondary_provider_references(&mut conn, id, MetadataProviderId::Fanart)
                    .unwrap(),
                vec![(kind.into(), database.into(), external_id.into())]
            );
        }
        assert!(
            get_item_secondary_provider_references(&mut conn, 4, MetadataProviderId::Fanart)
                .unwrap()
                .is_empty()
        );

        let details = parse_artwork(
            &serde_json::json!({
                "mbid_id": ARTIST,
                "artistthumb": [{"url": "https://assets.fanart.tv/artist.jpg"}],
                "artistbackground": [{"url": "https://assets.fanart.tv/background.jpg"}],
                "hdmusiclogo": [{"url": "https://assets.fanart.tv/logo.png"}]
            })
            .to_string(),
            "artist",
            ARTIST,
            "en-US",
        )
        .unwrap()
        .unwrap();
        let secondary = StoredMetadataSnapshot {
            provider_id: MetadataProviderId::Fanart,
            external_id: format!("artist:musicbrainz:{ARTIST}"),
            media_type: Some("artist".into()),
            title: None,
            overview: None,
            artwork_url: details.artwork_url.clone(),
            backdrop_url: details.backdrop_url.clone(),
            release_year: None,
            locale_key: "en-US".into(),
            provider_locale_key: None,
            provider_payload_json: None,
        };
        upsert_item_metadata_link(&mut conn, 1, &secondary, &details, "secondary", None).unwrap();
        let item = get_media_item(&mut conn, 1, "missing-data-directory")
            .unwrap()
            .unwrap();
        assert_eq!(item.display_title, "Matched Artist");
        assert_eq!(item.overview.as_deref(), Some("Artist description"));
        assert!(
            item.poster_url.is_some() && item.backdrop_url.is_some() && item.logo_url.is_some()
        );
        let cards = list_media_items(&mut conn, Some(1)).unwrap();
        let card = cards.iter().find(|item| item.id == 1).unwrap();
        assert!(card.backdrop_url.is_some() && card.logo_url.is_some());
        let poster = crate::media::get_preferred_item_artwork_metadata_link_for_languages(
            &mut conn,
            4,
            &[],
            crate::metadata::ArtworkKind::Poster,
        )
        .unwrap()
        .unwrap();
        assert_eq!(poster.provider_id, "musicbrainz");
        assert_eq!(poster.media_item_id, 2);
        use crate::db::schema::item_metadata_links::dsl as links;
        let json = links::item_metadata_links
            .filter(links::provider_id.eq("fanart"))
            .select(links::genres_json)
            .first::<Option<String>>(&mut conn)
            .unwrap();
        assert!(json.is_none());
        let album_artwork = StoredMetadataSnapshot {
            external_id: format!("album:musicbrainz_release_group:{ALBUM}"),
            media_type: Some("album".into()),
            artwork_url: Some("https://assets.fanart.tv/album.jpg".into()),
            ..secondary.clone()
        };
        upsert_item_metadata_link(&mut conn, 2, &album_artwork, &details, "secondary", None)
            .unwrap();
        conn.batch_execute(
            "UPDATE media_libraries SET metadata_providers_json='[\"fanart\",\"musicbrainz\"]'",
        )
        .unwrap();
        let poster = crate::media::get_preferred_item_artwork_metadata_link_for_languages(
            &mut conn,
            4,
            &[],
            crate::metadata::ArtworkKind::Poster,
        )
        .unwrap()
        .unwrap();
        assert_eq!(poster.provider_id, "fanart");
        assert_eq!(poster.media_item_id, 2);
        assert_eq!(
            get_media_item(&mut conn, 2, "missing-data-directory")
                .unwrap()
                .unwrap()
                .display_title,
            "Matched Album"
        );
        conn.batch_execute(
            "UPDATE media_libraries SET metadata_providers_json='[\"musicbrainz\"]'",
        )
        .unwrap();
        let item = get_media_item(&mut conn, 1, "missing-data-directory")
            .unwrap()
            .unwrap();
        assert!(
            item.poster_url.is_none() && item.backdrop_url.is_none() && item.logo_url.is_none()
        );
        let cards = list_media_items(&mut conn, Some(1)).unwrap();
        let card = cards.iter().find(|item| item.id == 1).unwrap();
        assert!(card.backdrop_url.is_none() && card.logo_url.is_none());
    }
}
