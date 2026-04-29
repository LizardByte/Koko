use serde_json::Value;

use crate::config::MetadataProviderId;
use crate::metadata::{
    MediaLibraryKind, MetadataProviderDescriptor, MetadataProviderRole, ProviderMetadataDetails,
    normalize_locale_key, youtube_watch_url,
};

const TRAILERDB_DATA_BASE: &str = "https://trailerdb.org/data";

pub(crate) fn descriptor() -> MetadataProviderDescriptor {
    MetadataProviderDescriptor {
        id: MetadataProviderId::TrailerDb,
        display_name: "TrailerDB".into(),
        description: "Secondary provider for localized movie and show trailer metadata.".into(),
        supported_kinds: vec![
            MediaLibraryKind::Movies,
            MediaLibraryKind::Shows,
        ],
        requires_api_key: false,
        implemented: true,
        role: MetadataProviderRole::Secondary,
        extends_provider_ids: vec![MetadataProviderId::Tmdb],
        attribution_text: "Trailer metadata provided by The Trailer Database.".into(),
        attribution_url: "https://trailerdb.org/".into(),
        logo_light_url: None,
        logo_dark_url: None,
    }
}

pub(crate) fn provider_locale_key(locale_key: &str) -> String {
    normalize_locale_key(locale_key)
        .split('-')
        .next()
        .unwrap_or("en")
        .to_ascii_lowercase()
}

pub(crate) async fn fetch_secondary_metadata(
    media_type: &str,
    database_id: &str,
    external_id: &str,
    locale_key: &str,
) -> Result<Option<ProviderMetadataDetails>, String> {
    let Some(path) = trailerdb_path(media_type, database_id, external_id) else {
        return Ok(None);
    };

    let response = reqwest::Client::new()
        .get(format!("{TRAILERDB_DATA_BASE}/{path}.json"))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(format!(
            "TrailerDB lookup failed with status {}",
            response.status()
        ));
    }

    let payload = response.text().await.map_err(|error| error.to_string())?;
    Ok(parse_youtube_trailer(&payload, locale_key))
}

fn trailerdb_path(
    media_type: &str,
    database_id: &str,
    external_id: &str,
) -> Option<String> {
    let external_id = external_id.trim();
    if external_id.is_empty() {
        return None;
    }

    match (
        media_type.trim().to_ascii_lowercase().as_str(),
        database_id.trim().to_ascii_lowercase().as_str(),
    ) {
        ("movie", "imdb") => Some(format!("movie/{external_id}")),
        ("tv" | "series" | "show", "tmdb" | "themoviedb") => Some(format!("series/{external_id}")),
        _ => None,
    }
}

fn parse_youtube_trailer(
    payload_json: &str,
    locale_key: &str,
) -> Option<ProviderMetadataDetails> {
    let payload = serde_json::from_str::<Value>(payload_json).ok()?;
    let language = provider_locale_key(locale_key);

    trailer_from_groups(payload.get("trailer_groups"), &language)
        .or_else(|| trailer_from_entries(payload.get("trailers"), &language))
}

fn trailer_from_groups(
    groups: Option<&Value>,
    language: &str,
) -> Option<ProviderMetadataDetails> {
    groups?.as_array()?.iter().find_map(|group| {
        let languages = group.get("languages")?.as_object()?;
        let translation = languages
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(language))
            .map(|(_, value)| value)?;
        let youtube_id = text_field(translation, &["youtube_id"])?;
        let title = text_field(translation, &["title"]).or_else(|| text_field(group, &["title"]));
        youtube_watch_url(&youtube_id).map(|url| ProviderMetadataDetails {
            trailer_title: title,
            trailer_url: Some(url),
            ..ProviderMetadataDetails::default()
        })
    })
}

fn trailer_from_entries(
    trailers: Option<&Value>,
    language: &str,
) -> Option<ProviderMetadataDetails> {
    trailers?
        .as_array()?
        .iter()
        .filter(|entry| {
            text_field(entry, &["language"])
                .as_deref()
                .is_some_and(|entry_language| entry_language.eq_ignore_ascii_case(language))
        })
        .fold(None, |best, entry| {
            let score = trailer_entry_score(entry);
            match best {
                Some((best_score, best_entry)) if best_score >= score => {
                    Some((best_score, best_entry))
                }
                _ => Some((score, entry)),
            }
        })
        .and_then(|(_, entry)| trailer_from_entry(entry))
}

fn trailer_from_entry(entry: &Value) -> Option<ProviderMetadataDetails> {
    let youtube_id = text_field(entry, &["youtube_id"])?;
    youtube_watch_url(&youtube_id).map(|url| ProviderMetadataDetails {
        trailer_title: text_field(entry, &["title"]),
        trailer_url: Some(url),
        ..ProviderMetadataDetails::default()
    })
}

fn trailer_entry_score(entry: &Value) -> (i32, i32) {
    let official_score = entry
        .get("is_official")
        .and_then(Value::as_bool)
        .map(i32::from)
        .unwrap_or(0);
    let trailer_type_score = text_field(entry, &["type", "trailer_type"])
        .map(|value| value.eq_ignore_ascii_case("trailer") as i32)
        .unwrap_or(0);
    (official_score, trailer_type_score)
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
    use super::{parse_youtube_trailer, trailerdb_path};

    #[test]
    fn movie_lookup_uses_imdb_detail_endpoint() {
        assert_eq!(
            trailerdb_path("movie", "imdb", "tt0133093").as_deref(),
            Some("movie/tt0133093")
        );
        assert_eq!(trailerdb_path("movie", "tmdb", "603"), None);
    }

    #[test]
    fn show_lookup_uses_tmdb_series_detail_endpoint() {
        assert_eq!(
            trailerdb_path("tv", "tmdb", "1399").as_deref(),
            Some("series/1399")
        );
        assert_eq!(
            trailerdb_path("show", "themoviedb", "1399").as_deref(),
            Some("series/1399")
        );
        assert_eq!(trailerdb_path("tv", "imdb", "tt0944947"), None);
    }

    #[test]
    fn trailer_groups_return_requested_language_only() {
        let payload = serde_json::json!({
            "trailer_groups": [
                {
                    "group_id": "official",
                    "type": "Trailer",
                    "title": "Official Trailer",
                    "languages": {
                        "en": {
                            "youtube_id": "abcdefghijk",
                            "title": "Official Trailer"
                        },
                        "es": {
                            "youtube_id": "ZYXWVUT9876",
                            "title": "Trailer oficial"
                        }
                    }
                }
            ]
        })
        .to_string();

        let trailer = parse_youtube_trailer(&payload, "es-ES").expect("Expected trailer");

        assert_eq!(trailer.trailer_title.as_deref(), Some("Trailer oficial"));
        assert_eq!(
            trailer.trailer_url.as_deref(),
            Some("https://www.youtube.com/watch?v=ZYXWVUT9876")
        );
        assert_eq!(parse_youtube_trailer(&payload, "fr-FR"), None);
    }

    #[test]
    fn trailers_return_requested_official_trailer() {
        let payload = serde_json::json!({
            "trailers": [
                {
                    "youtube_id": "aaaaaaaaaaa",
                    "title": "Spanish clip",
                    "type": "Clip",
                    "language": "es",
                    "is_official": false
                },
                {
                    "youtube_id": "bbbbbbbbbbb",
                    "title": "Trailer oficial",
                    "type": "Trailer",
                    "language": "es",
                    "is_official": true
                },
                {
                    "youtube_id": "ccccccccccc",
                    "title": "Official Trailer",
                    "type": "Trailer",
                    "language": "en",
                    "is_official": true
                }
            ]
        })
        .to_string();

        let trailer = parse_youtube_trailer(&payload, "es").expect("Expected trailer");

        assert_eq!(trailer.trailer_title.as_deref(), Some("Trailer oficial"));
        assert_eq!(
            trailer.trailer_url.as_deref(),
            Some("https://www.youtube.com/watch?v=bbbbbbbbbbb")
        );
    }
}
