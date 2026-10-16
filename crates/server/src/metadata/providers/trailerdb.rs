use serde::Deserialize;
use std::collections::BTreeMap;

use crate::config::MetadataProviderId;
use crate::metadata::{
    METADATA_EXTRA_TYPE_TRAILER,
    MediaLibraryKind,
    MetadataProviderDescriptor,
    MetadataProviderRole,
    ProviderMetadataDetails,
    ProviderMetadataExtra,
    normalize_locale_key,
    normalize_metadata_extra_type,
    youtube_watch_url,
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
        logo_light_url: Some("/assets/providers/trailerdb-light.svg".into()),
        logo_dark_url: Some("/assets/providers/trailerdb-dark.svg".into()),
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
    item_type: &str,
    database_id: &str,
    external_id: &str,
    locale_key: &str,
) -> Result<Option<ProviderMetadataDetails>, String> {
    let Some(path) = trailerdb_path(item_type, database_id, external_id) else {
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
    Ok(parse_youtube_trailer(&payload, locale_key, item_type))
}

fn trailerdb_path(
    item_type: &str,
    database_id: &str,
    external_id: &str,
) -> Option<String> {
    let external_id = external_id.trim();
    if external_id.is_empty() {
        return None;
    }

    match (
        item_type.trim().to_ascii_lowercase().as_str(),
        database_id.trim().to_ascii_lowercase().as_str(),
    ) {
        ("movie", "imdb") => Some(format!("movie/{external_id}")),
        ("show", "tmdb") => Some(format!("series/{external_id}")),
        _ => None,
    }
}

// Movie and series detail responses have different documented video fields.
// https://trailerdb.org/data/movie/tt0133093.json
// https://trailerdb.org/data/series/1399.json
#[derive(Deserialize)]
struct MovieResponse {
    trailer_groups: Option<Vec<TrailerGroup>>,
    trailers: Option<Vec<MovieTrailer>>,
}

#[derive(Deserialize)]
struct SeriesResponse {
    trailers: Option<Vec<SeriesTrailer>>,
}

#[derive(Deserialize)]
struct TrailerGroup {
    #[serde(rename = "type")]
    extra_type: String,
    title: Option<String>,
    languages: BTreeMap<String, LocalizedTrailer>,
}

#[derive(Deserialize)]
struct LocalizedTrailer {
    youtube_id: String,
    title: Option<String>,
}

#[derive(Deserialize)]
struct MovieTrailer {
    youtube_id: String,
    title: Option<String>,
    #[serde(rename = "type")]
    extra_type: String,
    language: String,
    is_official: Option<bool>,
    duration: Option<i32>,
}

#[derive(Deserialize)]
struct SeriesTrailer {
    youtube_id: String,
    title: Option<String>,
    trailer_type: String,
    language: String,
    is_official: Option<bool>,
}

fn parse_youtube_trailer(
    payload_json: &str,
    locale_key: &str,
    item_type: &str,
) -> Option<ProviderMetadataDetails> {
    let language = provider_locale_key(locale_key);
    let extras = match item_type {
        "movie" => {
            let response: MovieResponse = serde_json::from_str(payload_json).ok()?;
            let groups = extras_from_groups(response.trailer_groups.unwrap_or_default(), &language);
            if groups.is_empty() {
                extras_from_entries(response.trailers.unwrap_or_default(), &language)
            } else {
                groups
            }
        }
        "show" => {
            let response: SeriesResponse = serde_json::from_str(payload_json).ok()?;
            let entries = response
                .trailers
                .unwrap_or_default()
                .into_iter()
                .map(|entry| MovieTrailer {
                    youtube_id: entry.youtube_id,
                    title: entry.title,
                    extra_type: entry.trailer_type,
                    language: entry.language,
                    is_official: entry.is_official,
                    duration: None,
                })
                .collect();
            extras_from_entries(entries, &language)
        }
        _ => return None,
    };
    let trailer = extras
        .iter()
        .find(|extra| extra.extra_type == METADATA_EXTRA_TYPE_TRAILER)
        .or_else(|| extras.first())?;
    Some(ProviderMetadataDetails {
        trailer_title: trailer.title.clone(),
        trailer_url: Some(trailer.url.clone()),
        extras,
        ..ProviderMetadataDetails::default()
    })
}

fn extras_from_groups(
    groups: Vec<TrailerGroup>,
    language: &str,
) -> Vec<ProviderMetadataExtra> {
    groups
        .into_iter()
        .enumerate()
        .filter_map(|(index, group)| {
            let translation = group
                .languages
                .into_iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(language))?
                .1;
            Some(ProviderMetadataExtra {
                extra_type: normalize_metadata_extra_type(&group.extra_type)?,
                title: translation.title.or(group.title),
                url: youtube_watch_url(&translation.youtube_id)?,
                duration_seconds: None,
                thumbnail_url: None,
                sort_order: index as i32,
            })
        })
        .collect()
}

fn extras_from_entries(
    entries: Vec<MovieTrailer>,
    language: &str,
) -> Vec<ProviderMetadataExtra> {
    let mut entries = entries
        .into_iter()
        .filter(|entry| entry.language.eq_ignore_ascii_case(language))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| {
        std::cmp::Reverse((
            entry.is_official.unwrap_or(false),
            entry.extra_type.eq_ignore_ascii_case("trailer"),
        ))
    });
    entries
        .into_iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            Some(ProviderMetadataExtra {
                extra_type: normalize_metadata_extra_type(&entry.extra_type)?,
                title: entry.title,
                url: youtube_watch_url(&entry.youtube_id)?,
                duration_seconds: entry.duration.filter(|value| *value > 0),
                thumbnail_url: None,
                sort_order: index as i32,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn series_response_uses_trailer_type_and_rejects_movie_alias() {
        let payload = serde_json::json!({
            "trailers": [
                {
                    "youtube_id": "abcdefghijk",
                    "title": "Trailer",
                    "trailer_type": "trailer",
                    "language": "en"
                }
            ]
        })
        .to_string();
        assert!(super::parse_youtube_trailer(&payload, "en-US", "show").is_some());
        let payload = serde_json::json!({
            "trailers": [
                {
                    "youtube_id": "abcdefghijk",
                    "title": "Trailer",
                    "type": "trailer",
                    "language": "en"
                }
            ]
        })
        .to_string();
        assert!(super::parse_youtube_trailer(&payload, "en-US", "show").is_none());
    }
    use super::{
        parse_youtube_trailer,
        trailerdb_path,
    };

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
            trailerdb_path("show", "tmdb", "1399").as_deref(),
            Some("series/1399")
        );
        assert_eq!(trailerdb_path("tv", "tmdb", "1399"), None);
        assert_eq!(trailerdb_path("series", "tmdb", "1399"), None);
        assert_eq!(trailerdb_path("show", "themoviedb", "1399"), None);
        assert_eq!(trailerdb_path("show", "imdb", "tt0944947"), None);
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

        let trailer = parse_youtube_trailer(&payload, "es-ES", "movie").expect("Expected trailer");

        assert_eq!(trailer.trailer_title.as_deref(), Some("Trailer oficial"));
        assert_eq!(
            trailer.trailer_url.as_deref(),
            Some("https://www.youtube.com/watch?v=ZYXWVUT9876")
        );
        assert_eq!(parse_youtube_trailer(&payload, "fr-FR", "movie"), None);
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
                    "is_official": false,
                    "duration": 52
                },
                {
                    "youtube_id": "bbbbbbbbbbb",
                    "title": "Trailer oficial",
                    "type": "Trailer",
                    "language": "es",
                    "is_official": true,
                    "duration": 148
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

        let trailer = parse_youtube_trailer(&payload, "es", "movie").expect("Expected trailer");

        assert_eq!(trailer.trailer_title.as_deref(), Some("Trailer oficial"));
        assert_eq!(
            trailer.trailer_url.as_deref(),
            Some("https://www.youtube.com/watch?v=bbbbbbbbbbb")
        );
        assert_eq!(
            trailer
                .extras
                .iter()
                .map(|extra| extra.extra_type.as_str())
                .collect::<Vec<_>>(),
            vec!["trailer", "clip"]
        );
        assert_eq!(trailer.extras[0].duration_seconds, Some(148));
    }
}
