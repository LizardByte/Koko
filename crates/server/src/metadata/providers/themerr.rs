use serde_json::Value;

use crate::config::MetadataProviderId;
use crate::metadata::{MediaLibraryKind, MetadataProviderDescriptor, MetadataProviderRole};

const THEMERR_API_BASE: &str = "https://app.lizardbyte.dev/ThemerrDB";

pub(crate) fn descriptor() -> MetadataProviderDescriptor {
    MetadataProviderDescriptor {
        id: MetadataProviderId::Themerr,
        display_name: "ThemerrDB".into(),
        description:
            "Secondary provider for theme-song metadata linked to movie and show metadata.".into(),
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
    let Some(database_id) = normalize_database_id(database_id) else {
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

fn database_path_for_media_type(media_type: &str) -> Option<&'static str> {
    match media_type.trim() {
        "movie" => Some("movies"),
        "tv" | "series" | "show" => Some("tv"),
        _ => None,
    }
}

fn normalize_database_id(database_id: &str) -> Option<&'static str> {
    match database_id.trim().to_ascii_lowercase().as_str() {
        "themoviedb" | "tmdb" => Some("themoviedb"),
        "imdb" => Some("imdb"),
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
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::parse_youtube_theme_url;

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
}
