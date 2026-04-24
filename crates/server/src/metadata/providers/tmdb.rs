use serde::Deserialize;
use serde_json::Value;
use strsim::normalized_levenshtein;

use crate::config::{MetadataProviderId, MetadataSettings};
use crate::metadata::{
    MediaLibraryKind, MetadataProviderDescriptor, MetadataSearchResult, StoredMetadataSnapshot,
    cleanup_movie_title, extract_release_year, movie_match_score, parse_movie_name,
    show_search_query, tmdb_episode_external_id, tmdb_get_json, tmdb_get_text, tmdb_image_url,
    tmdb_provider_settings, tmdb_season_external_id,
};

#[derive(Debug, Deserialize)]
struct TmdbSearchResponse {
    results: Vec<TmdbSearchItem>,
}

#[derive(Debug, Deserialize)]
struct TmdbSearchItem {
    id: i64,
    media_type: Option<String>,
    title: Option<String>,
    name: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    release_date: Option<String>,
    first_air_date: Option<String>,
}

pub(crate) fn descriptor() -> MetadataProviderDescriptor {
    MetadataProviderDescriptor {
        id: MetadataProviderId::Tmdb,
        display_name: "TheMovieDB".into(),
        description: "Primary movie and television metadata provider for Koko.".into(),
        supported_kinds: vec![
            MediaLibraryKind::Movies,
            MediaLibraryKind::Shows,
        ],
        requires_api_key: true,
        implemented: true,
    }
}

pub(crate) async fn search(
    settings: &MetadataSettings,
    query: &str,
) -> Result<Vec<MetadataSearchResult>, String> {
    let provider = tmdb_provider_settings(settings)?;
    let payload = tmdb_get_json::<TmdbSearchResponse>(
        &provider,
        "search/multi",
        vec![("query", query.to_string())],
        &format!("search query {:?}", query),
    )
    .await?;
    Ok(payload
        .results
        .into_iter()
        .filter_map(|item| {
            let media_type = item.media_type.unwrap_or_default();
            if media_type != "movie" && media_type != "tv" {
                return None;
            }

            let title = item.title.or(item.name)?;
            Some(MetadataSearchResult {
                provider_id: MetadataProviderId::Tmdb,
                external_id: item.id.to_string(),
                media_type,
                title,
                overview: item.overview,
                artwork_url: item.poster_path.map(|path| tmdb_image_url(&path, "w500")),
                backdrop_url: item
                    .backdrop_path
                    .map(|path| tmdb_image_url(&path, "w1280")),
                release_year: extract_release_year(item.release_date.or(item.first_air_date)),
            })
        })
        .collect())
}

pub(crate) async fn fetch_snapshot(
    settings: &MetadataSettings,
    external_id: &str,
    media_type: &str,
) -> Result<StoredMetadataSnapshot, String> {
    let provider = tmdb_provider_settings(settings)?;
    let payload = tmdb_get_text(
        &provider,
        &format!("{}/{}", media_type, external_id),
        vec![("append_to_response", "videos".to_string())],
        &format!("details lookup for {media_type}:{external_id}"),
    )
    .await?;
    let parsed: Value = serde_json::from_str(&payload).map_err(|error| error.to_string())?;
    let title = parsed
        .get("title")
        .or_else(|| parsed.get("name"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let overview = parsed
        .get("overview")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .filter(|value| !value.trim().is_empty());
    let artwork_url = parsed
        .get("poster_path")
        .and_then(Value::as_str)
        .map(|path| tmdb_image_url(path, "w500"));
    let backdrop_url = parsed
        .get("backdrop_path")
        .and_then(Value::as_str)
        .map(|path| tmdb_image_url(path, "w1280"));
    let release_year = parsed
        .get("release_date")
        .or_else(|| parsed.get("first_air_date"))
        .and_then(Value::as_str)
        .map(|value| value.to_string())
        .and_then(|value| extract_release_year(Some(value)));

    Ok(StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tmdb,
        external_id: external_id.to_string(),
        media_type: Some(media_type.to_string()),
        title,
        overview,
        artwork_url,
        backdrop_url,
        release_year,
        provider_payload_json: Some(payload),
    })
}

pub(crate) async fn guess_movie_match(
    settings: &MetadataSettings,
    relative_path: &str,
    display_title: &str,
) -> Result<Option<MetadataSearchResult>, String> {
    let parsed = parse_movie_name(relative_path, display_title);
    if parsed.title.trim().is_empty() {
        return Ok(None);
    }

    if let Some(tmdb_id) = parsed.tmdb_id.clone() {
        let snapshot = fetch_snapshot(settings, &tmdb_id, "movie").await?;
        return Ok(Some(MetadataSearchResult {
            provider_id: MetadataProviderId::Tmdb,
            external_id: tmdb_id,
            media_type: "movie".into(),
            title: snapshot.title.unwrap_or(parsed.title),
            overview: snapshot.overview,
            artwork_url: snapshot.artwork_url,
            backdrop_url: snapshot.backdrop_url,
            release_year: snapshot.release_year,
        }));
    }

    let mut best_result = None;
    let mut best_score = 0.0;
    for result in search(settings, &parsed.title).await? {
        if result.media_type != "movie" {
            continue;
        }

        let score = movie_match_score(&parsed, &result);
        if score > best_score {
            best_score = score;
            best_result = Some(result);
        }
    }

    Ok((best_score >= 0.78).then_some(best_result).flatten())
}

pub(crate) async fn guess_show_match(
    settings: &MetadataSettings,
    relative_path: &str,
    display_title: &str,
) -> Result<Option<MetadataSearchResult>, String> {
    let query = show_search_query(relative_path, display_title);
    if query.trim().is_empty() {
        return Ok(None);
    }

    let mut best_result = None;
    let mut best_score = 0.0;
    for result in search(settings, &query).await? {
        if result.media_type != "tv" {
            continue;
        }

        let score = normalized_levenshtein(
            &cleanup_movie_title(&query).to_ascii_lowercase(),
            &cleanup_movie_title(&result.title).to_ascii_lowercase(),
        );
        if score > best_score {
            best_score = score;
            best_result = Some(result);
        }
    }

    Ok((best_score >= 0.78).then_some(best_result).flatten())
}

pub(crate) async fn fetch_season_snapshot(
    settings: &MetadataSettings,
    show_external_id: &str,
    season_number: i32,
) -> Result<StoredMetadataSnapshot, String> {
    let provider = tmdb_provider_settings(settings)?;
    let payload = tmdb_get_text(
        &provider,
        &format!("tv/{}/season/{}", show_external_id, season_number),
        Vec::new(),
        &format!("season lookup for tv:{show_external_id}:season:{season_number}"),
    )
    .await?;
    let parsed: Value = serde_json::from_str(&payload).map_err(|error| error.to_string())?;

    Ok(StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tmdb,
        external_id: tmdb_season_external_id(show_external_id, season_number),
        media_type: Some("tv_season".into()),
        title: parsed
            .get("name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        overview: parsed
            .get("overview")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        artwork_url: parsed
            .get("poster_path")
            .and_then(Value::as_str)
            .map(|path| tmdb_image_url(path, "w500")),
        backdrop_url: None,
        release_year: parsed
            .get("air_date")
            .and_then(Value::as_str)
            .map(|value| value.to_string())
            .and_then(|value| extract_release_year(Some(value))),
        provider_payload_json: Some(payload),
    })
}

pub(crate) async fn fetch_episode_snapshot(
    settings: &MetadataSettings,
    show_external_id: &str,
    season_number: i32,
    episode_number: i32,
) -> Result<StoredMetadataSnapshot, String> {
    let provider = tmdb_provider_settings(settings)?;
    let payload = tmdb_get_text(
        &provider,
        &format!(
            "tv/{}/season/{}/episode/{}",
            show_external_id, season_number, episode_number
        ),
        Vec::new(),
        &format!(
            "episode lookup for tv:{show_external_id}:season:{season_number}:episode:{episode_number}"
        ),
    )
    .await?;
    let parsed: Value = serde_json::from_str(&payload).map_err(|error| error.to_string())?;

    Ok(StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tmdb,
        external_id: tmdb_episode_external_id(show_external_id, season_number, episode_number),
        media_type: Some("tv_episode".into()),
        title: parsed
            .get("name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        overview: parsed
            .get("overview")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        artwork_url: parsed
            .get("still_path")
            .and_then(Value::as_str)
            .map(|path| tmdb_image_url(path, "w780")),
        backdrop_url: None,
        release_year: parsed
            .get("air_date")
            .and_then(Value::as_str)
            .map(|value| value.to_string())
            .and_then(|value| extract_release_year(Some(value))),
        provider_payload_json: Some(payload),
    })
}
