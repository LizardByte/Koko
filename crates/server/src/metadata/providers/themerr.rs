use serde_json::Value;

use crate::config::MetadataProviderId;
use crate::metadata::{
    METADATA_EXTRA_TYPE_THEME_SONG, MediaLibraryKind, MetadataProviderDescriptor,
    MetadataProviderRole, ProviderMetadataDetails, ProviderMetadataExtra, youtube_watch_url,
};

const THEMERR_API_BASE: &str = "https://app.lizardbyte.dev/ThemerrDB";

pub(crate) fn descriptor() -> MetadataProviderDescriptor {
    MetadataProviderDescriptor {
        id: MetadataProviderId::Themerr,
        display_name: "ThemerrDB".into(),
        description:
            "Secondary provider for theme-song metadata linked to movie, show, and collection metadata.".into(),
        supported_kinds: vec![
            MediaLibraryKind::Movies,
            MediaLibraryKind::Shows,
        ],
        requires_api_key: false,
        implemented: true,
        role: MetadataProviderRole::Secondary,
        extends_provider_ids: vec![MetadataProviderId::Tmdb],
        attribution_text: "Theme metadata provided by ThemerrDB.".into(),
        attribution_url: "https://app.lizardbyte.dev/ThemerrDB".into(),
        logo_light_url: Some(
            "https://app.lizardbyte.dev/ThemerrDB/assets/img/navbar-avatar.png".into(),
        ),
        logo_dark_url: Some(
            "https://app.lizardbyte.dev/ThemerrDB/assets/img/navbar-avatar.png".into(),
        ),
    }
}

pub(crate) async fn fetch_youtube_theme_url(
    media_type: &str,
    database_id: &str,
    external_id: &str,
) -> Result<Option<String>, String> {
    let Some(database_path) = database_path_for_media_type(media_type) else {
        return Ok(None);
    };
    let Some(database_id) = normalize_database_id(media_type, database_id) else {
        return Ok(None);
    };
    let normalized_external_id = external_id.trim();
    if normalized_external_id.is_empty() {
        return Ok(None);
    }

    let response = reqwest::Client::new()
        .get(format!(
            "{}/{}/{}/{}.json",
            THEMERR_API_BASE, database_path, database_id, normalized_external_id
        ))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(format!(
            "ThemerrDB lookup failed with status {}",
            response.status()
        ));
    }

    let payload = response.text().await.map_err(|error| error.to_string())?;
    Ok(parse_youtube_theme_url(&payload))
}

pub(crate) async fn fetch_youtube_theme_metadata(
    media_type: &str,
    database_id: &str,
    external_id: &str,
) -> Result<Option<ProviderMetadataDetails>, String> {
    let Some(theme_song_url) =
        fetch_youtube_theme_url(media_type, database_id, external_id).await?
    else {
        return Ok(None);
    };
    let oembed = fetch_youtube_oembed_metadata(&theme_song_url).await;

    Ok(Some(ProviderMetadataDetails {
        theme_song_url: Some(theme_song_url.clone()),
        extras: vec![ProviderMetadataExtra {
            extra_type: METADATA_EXTRA_TYPE_THEME_SONG.to_string(),
            title: oembed.as_ref().and_then(|metadata| metadata.title.clone()),
            url: theme_song_url,
            duration_seconds: None,
            thumbnail_url: oembed.and_then(|metadata| metadata.thumbnail_url),
            sort_order: 0,
        }],
        ..ProviderMetadataDetails::default()
    }))
}

#[derive(Debug, Clone)]
struct YoutubeOEmbedMetadata {
    title: Option<String>,
    thumbnail_url: Option<String>,
}

async fn fetch_youtube_oembed_metadata(url: &str) -> Option<YoutubeOEmbedMetadata> {
    let response = reqwest::Client::new()
        .get("https://www.youtube.com/oembed")
        .query(&[
            ("format", "json"),
            ("url", url),
        ])
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let payload = response.text().await.ok()?;
    let payload = serde_json::from_str::<Value>(&payload).ok()?;
    Some(YoutubeOEmbedMetadata {
        title: text_field(&payload, &["title"]),
        thumbnail_url: text_field(&payload, &["thumbnail_url"]),
    })
}

fn database_path_for_media_type(media_type: &str) -> Option<&'static str> {
    match media_type.trim() {
        "movie" => Some("movies"),
        "tv" | "series" | "show" => Some("tv_shows"),
        "collection" | "movie_collection" => Some("movie_collections"),
        _ => None,
    }
}

fn normalize_database_id(
    media_type: &str,
    database_id: &str,
) -> Option<&'static str> {
    let normalized_media_type = media_type.trim().to_ascii_lowercase();
    match database_id.trim().to_ascii_lowercase().as_str() {
        "themoviedb" | "tmdb" => Some("themoviedb"),
        "imdb" if normalized_media_type == "movie" => Some("imdb"),
        _ => None,
    }
}

fn parse_youtube_theme_url(payload_json: &str) -> Option<String> {
    serde_json::from_str::<Value>(payload_json)
        .ok()?
        .get("youtube_theme_url")?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(youtube_watch_url)
}

fn text_field(
    value: &Value,
    keys: &[&str],
) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

#[cfg(test)]
mod tests {
    use super::{database_path_for_media_type, normalize_database_id, parse_youtube_theme_url};

    #[test]
    fn parse_youtube_theme_url_extracts_watch_url() {
        let payload = serde_json::json!({
            "id": 603,
            "title": "The Matrix",
            "youtube_theme_url": "https://www.youtube.com/watch?v=SLBACEP6LsI"
        })
        .to_string();

        assert_eq!(
            parse_youtube_theme_url(&payload).as_deref(),
            Some("https://www.youtube.com/watch?v=SLBACEP6LsI")
        );
    }

    #[test]
    fn parse_youtube_theme_url_rejects_missing_url() {
        let payload = serde_json::json!({
            "id": 1399,
            "name": "Game of Thrones"
        })
        .to_string();

        assert_eq!(parse_youtube_theme_url(&payload), None);
    }

    #[test]
    fn collection_theme_lookup_uses_movie_collection_database() {
        assert_eq!(
            database_path_for_media_type("collection"),
            Some("movie_collections")
        );
        assert_eq!(
            normalize_database_id("collection", "tmdb"),
            Some("themoviedb")
        );
        assert_eq!(normalize_database_id("collection", "imdb"), None);
    }
}
