use serde_json::Value;
use strsim::normalized_levenshtein;
use tmdb_client::apis::client::APIClient as TmdbApiClient;
use tmdb_client::models::{EpisodeDetails, MovieDetails, MovieObject, SeasonDetails, TvDetails};

use crate::config::{MetadataProviderId, MetadataSettings};
use crate::metadata::{
    MediaLibraryKind, MetadataProviderDescriptor, MetadataSearchResult, StoredMetadataSnapshot,
    cleanup_movie_title, extract_release_year, movie_match_score, parse_movie_name,
    show_search_query, tmdb_episode_external_id, tmdb_image_url, tmdb_provider_settings,
    tmdb_season_external_id,
};

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
        attribution_text: "Metadata and artwork provided by The Movie Database (TMDB).".into(),
        attribution_url: "https://www.themoviedb.org/".into(),
        logo_light_url: Some("https://www.themoviedb.org/assets/2/v4/logos/v2/blue_square_1-5bdc75aaebeb75dc7ae79426ddd9be3b2be1e342510f8202baf6bffa71d7f5c4.svg".into()),
        logo_dark_url: Some("https://www.themoviedb.org/assets/2/v4/logos/v2/blue_square_1-5bdc75aaebeb75dc7ae79426ddd9be3b2be1e342510f8202baf6bffa71d7f5c4.svg".into()),
    }
}

pub(crate) async fn search(
    settings: &MetadataSettings,
    query: &str,
) -> Result<Vec<MetadataSearchResult>, String> {
    let provider = tmdb_provider_settings(settings)?;
    let api_key = tmdb_api_key_from_provider(&provider)?;
    let query = query.to_string();
    let language = provider.language;
    run_tmdb_blocking(move || {
        let client = TmdbApiClient::new_with_api_key(api_key);
        let payload = client
            .search_api()
            .get_search_multi_paginated(&query, Some(&language), Some(1), Some(false), None)
            .map_err(|error| format!("TMDB search query {:?} failed: {}", query, error))?;
        Ok(payload
            .results
            .unwrap_or_default()
            .into_iter()
            .filter_map(search_result_from_value)
            .collect())
    })
    .await
}

pub(crate) async fn fetch_snapshot(
    settings: &MetadataSettings,
    external_id: &str,
    media_type: &str,
) -> Result<StoredMetadataSnapshot, String> {
    let provider = tmdb_provider_settings(settings)?;
    let api_key = tmdb_api_key_from_provider(&provider)?;
    let language = provider.language;
    let external_id_number = parse_external_id(external_id, media_type)?;
    let external_id_string = external_id.to_string();
    let normalized_media_type = match media_type {
        "series" => "tv".to_string(),
        other => other.to_string(),
    };

    run_tmdb_blocking(move || match normalized_media_type.as_str() {
        "movie" => {
            let client = TmdbApiClient::new_with_api_key(api_key.clone());
            let details = client
                .movies_api()
                .get_movie_details(
                    external_id_number,
                    Some(&language),
                    None,
                    Some("videos,images,release_dates,external_ids"),
                )
                .map_err(|error| {
                    format!(
                        "TMDB details lookup for movie:{} failed: {}",
                        external_id_string, error
                    )
                })?;
            Ok(movie_snapshot_from_details(&external_id_string, &details))
        }
        "tv" => {
            let client = TmdbApiClient::new_with_api_key(api_key);
            let details = client
                .tv_api()
                .get_tv_details(
                    external_id_number,
                    Some(&language),
                    None,
                    Some("videos,images,content_ratings,external_ids"),
                )
                .map_err(|error| {
                    format!(
                        "TMDB details lookup for tv:{} failed: {}",
                        external_id_string, error
                    )
                })?;
            Ok(tv_snapshot_from_details(&external_id_string, &details))
        }
        other => Err(format!("Unsupported TMDB media type: {}", other)),
    })
    .await
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
            score: Some(1.0),
        }));
    }
    if let Some(tvdb_id) = parsed.tvdb_id.clone() {
        if let Some(result) = find_tmdb_movie_by_external_id(settings, &tvdb_id, "tvdb_id").await? {
            return Ok(Some(result));
        }
    }
    if let Some(imdb_id) = parsed.imdb_id.clone() {
        if let Some(result) = find_tmdb_movie_by_external_id(settings, &imdb_id, "imdb_id").await? {
            return Ok(Some(result));
        }
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

async fn find_tmdb_movie_by_external_id(
    settings: &MetadataSettings,
    external_id: &str,
    external_source: &str,
) -> Result<Option<MetadataSearchResult>, String> {
    let provider = tmdb_provider_settings(settings)?;
    let api_key = tmdb_api_key_from_provider(&provider)?;
    let language = provider.language;
    let external_id = external_id.to_string();
    let external_source = external_source.to_string();
    run_tmdb_blocking(move || {
        let client = TmdbApiClient::new_with_api_key(api_key);
        let payload = client
            .find_api()
            .get_find_external_id(&external_id, &external_source, Some(&language))
            .map_err(|error| {
                format!(
                    "TMDB external id lookup for {}:{} failed: {}",
                    external_source, external_id, error
                )
            })?;
        Ok(payload
            .movie_results
            .unwrap_or_default()
            .into_iter()
            .find_map(movie_search_result_from_object))
    })
    .await
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
    let api_key = tmdb_api_key_from_provider(&provider)?;
    let language = provider.language;
    let show_id = parse_external_id(show_external_id, "tv")?;
    let show_external_id = show_external_id.to_string();
    run_tmdb_blocking(move || {
        let client = TmdbApiClient::new_with_api_key(api_key);
        let details = client
            .tv_seasons_api()
            .get_tv_season_details(show_id, season_number, Some(&language), None, None)
            .map_err(|error| {
                format!(
                    "TMDB season lookup for tv:{}:season:{} failed: {}",
                    show_external_id, season_number, error
                )
            })?;
        Ok(season_snapshot_from_details(
            &show_external_id,
            season_number,
            &details,
        ))
    })
    .await
}

pub(crate) async fn fetch_episode_snapshot(
    settings: &MetadataSettings,
    show_external_id: &str,
    season_number: i32,
    episode_number: i32,
) -> Result<StoredMetadataSnapshot, String> {
    let provider = tmdb_provider_settings(settings)?;
    let api_key = tmdb_api_key_from_provider(&provider)?;
    let language = provider.language;
    let show_id = parse_external_id(show_external_id, "tv")?;
    let show_external_id = show_external_id.to_string();
    run_tmdb_blocking(move || {
        let client = TmdbApiClient::new_with_api_key(api_key);
        let details = client
            .tv_episodes_api()
            .get_tv_season_episode_details(
                show_id,
                season_number,
                episode_number,
                Some(&language),
                None,
                None,
            )
            .map_err(|error| {
                format!(
                    "TMDB episode lookup for tv:{}:season:{}:episode:{} failed: {}",
                    show_external_id, season_number, episode_number, error
                )
            })?;
        Ok(episode_snapshot_from_details(
            &show_external_id,
            season_number,
            episode_number,
            &details,
        ))
    })
    .await
}

fn parse_external_id(
    external_id: &str,
    media_type: &str,
) -> Result<i32, String> {
    external_id.parse::<i32>().map_err(|_| {
        format!(
            "TMDB {} external id must be numeric, got {:?}",
            media_type, external_id
        )
    })
}

fn tmdb_api_key_from_provider(
    provider: &crate::config::MetadataProviderSettings
) -> Result<String, String> {
    let api_key = provider.api_key.clone().unwrap_or_default();
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err("TMDB is enabled but no API key is configured.".into());
    }

    Ok(api_key.to_string())
}

async fn run_tmdb_blocking<T, F>(operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| format!("TMDB request task failed: {}", error))?
}

fn search_result_from_value(item: Value) -> Option<MetadataSearchResult> {
    let media_type = item.get("media_type")?.as_str()?.to_ascii_lowercase();
    if media_type != "movie" && media_type != "tv" {
        return None;
    }

    let external_id = item.get("id")?.as_i64()?.to_string();
    let title = item
        .get("title")
        .or_else(|| item.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)?;
    let overview = item
        .get("overview")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let artwork_url = item
        .get("poster_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|path| tmdb_image_url(path, "w500"));
    let backdrop_url = item
        .get("backdrop_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|path| tmdb_image_url(path, "w1280"));
    let release_year = item
        .get("release_date")
        .or_else(|| item.get("first_air_date"))
        .and_then(Value::as_str)
        .map(|value| value.to_string())
        .and_then(|value| extract_release_year(Some(value)));

    Some(MetadataSearchResult {
        provider_id: MetadataProviderId::Tmdb,
        external_id,
        media_type,
        title,
        overview,
        artwork_url,
        backdrop_url,
        release_year,
        score: None,
    })
}

fn movie_search_result_from_object(item: MovieObject) -> Option<MetadataSearchResult> {
    let external_id = item.id?.to_string();
    let title = item
        .title
        .as_deref()
        .or(item.original_title.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)?;
    Some(MetadataSearchResult {
        provider_id: MetadataProviderId::Tmdb,
        external_id,
        media_type: "movie".into(),
        title,
        overview: item
            .overview
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        artwork_url: item
            .poster_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|path| tmdb_image_url(path, "w500")),
        backdrop_url: item
            .backdrop_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|path| tmdb_image_url(path, "w1280")),
        release_year: item
            .release_date
            .and_then(|value| extract_release_year(Some(value))),
        score: None,
    })
}

fn movie_snapshot_from_details(
    external_id: &str,
    details: &MovieDetails,
) -> StoredMetadataSnapshot {
    StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tmdb,
        external_id: external_id.to_string(),
        media_type: Some("movie".into()),
        title: details.title.clone(),
        overview: details
            .overview
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        artwork_url: details
            .poster_path
            .as_deref()
            .map(|path| tmdb_image_url(path, "w500")),
        backdrop_url: details
            .backdrop_path
            .as_deref()
            .map(|path| tmdb_image_url(path, "w1280")),
        release_year: details
            .release_date
            .clone()
            .and_then(|value| extract_release_year(Some(value))),
        locale_key: crate::metadata::DEFAULT_METADATA_LOCALE.to_string(),
        provider_locale_key: None,
        provider_payload_json: serde_json::to_string(details).ok(),
    }
}

fn tv_snapshot_from_details(
    external_id: &str,
    details: &TvDetails,
) -> StoredMetadataSnapshot {
    StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tmdb,
        external_id: external_id.to_string(),
        media_type: Some("tv".into()),
        title: details.name.clone(),
        overview: details
            .overview
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        artwork_url: details
            .poster_path
            .as_deref()
            .map(|path| tmdb_image_url(path, "w500")),
        backdrop_url: details
            .backdrop_path
            .as_deref()
            .map(|path| tmdb_image_url(path, "w1280")),
        release_year: details
            .first_air_date
            .clone()
            .and_then(|value| extract_release_year(Some(value))),
        locale_key: crate::metadata::DEFAULT_METADATA_LOCALE.to_string(),
        provider_locale_key: None,
        provider_payload_json: serde_json::to_string(details).ok(),
    }
}

fn season_snapshot_from_details(
    show_external_id: &str,
    season_number: i32,
    details: &SeasonDetails,
) -> StoredMetadataSnapshot {
    StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tmdb,
        external_id: tmdb_season_external_id(show_external_id, season_number),
        media_type: Some("tv_season".into()),
        title: details.name.clone(),
        overview: details
            .overview
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        artwork_url: details
            .poster_path
            .as_deref()
            .map(|path| tmdb_image_url(path, "w500")),
        backdrop_url: None,
        release_year: details
            .air_date
            .clone()
            .and_then(|value| extract_release_year(Some(value))),
        locale_key: crate::metadata::DEFAULT_METADATA_LOCALE.to_string(),
        provider_locale_key: None,
        provider_payload_json: serde_json::to_string(details).ok(),
    }
}

fn episode_snapshot_from_details(
    show_external_id: &str,
    season_number: i32,
    episode_number: i32,
    details: &EpisodeDetails,
) -> StoredMetadataSnapshot {
    StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tmdb,
        external_id: tmdb_episode_external_id(show_external_id, season_number, episode_number),
        media_type: Some("tv_episode".into()),
        title: details.name.clone(),
        overview: details
            .overview
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        artwork_url: details
            .still_path
            .as_deref()
            .map(|path| tmdb_image_url(path, "w780")),
        backdrop_url: None,
        release_year: details
            .air_date
            .clone()
            .and_then(|value| extract_release_year(Some(value))),
        locale_key: crate::metadata::DEFAULT_METADATA_LOCALE.to_string(),
        provider_locale_key: None,
        provider_payload_json: serde_json::to_string(details).ok(),
    }
}
