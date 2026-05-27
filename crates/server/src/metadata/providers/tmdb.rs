use std::collections::{
    HashMap,
    HashSet,
};
use std::sync::Mutex;

use once_cell::sync::Lazy;
use serde_json::Value;
use strsim::normalized_levenshtein;
use tmdb_client::apis::client::APIClient as TmdbApiClient;
use tmdb_client::models::{
    EpisodeDetails,
    MovieDetails,
    MovieObject,
    SeasonDetails,
    TvDetails,
};

use crate::config::{
    MetadataProviderId,
    MetadataProviderSettings,
    MetadataSettings,
};
use crate::metadata::{
    MediaLibraryKind,
    MetadataItemKind,
    MetadataProviderDescriptor,
    MetadataProviderRole,
    MetadataSearchResult,
    ProviderDescendantTarget,
    ProviderExternalId,
    ProviderMetadataCollection,
    ProviderMetadataDetails,
    ProviderMetadataPerson,
    StoredMetadataSnapshot,
    cleanup_movie_title,
    extract_release_year,
    managed_metadata_asset_dir,
    metadata_asset_db_path,
    metadata_response_cache_key,
    movie_match_score,
    normalize_external_id_source,
    parse_movie_name,
    preferred_image_url_by_format,
    provider_settings,
    read_metadata_response_cache_text,
    show_search_query,
    try_cache_item_artwork,
    write_metadata_response_cache_text,
};

const TMDB_IMAGE_BASE: &str = "https://image.tmdb.org/t/p";
const TMDB_LOGO_URL: &str = concat!(
    "https://www.themoviedb.org/assets/2/v4/logos/v2/",
    "blue_square_1-5bdc75aaebeb75dc7ae79426ddd9be3b2be1e342510f8202baf6b",
    "ffa71d7f5c4.svg",
);

static TMDB_PERSON_DETAIL_CACHE: Lazy<Mutex<HashMap<String, Value>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static TMDB_COLLECTION_DETAIL_CACHE: Lazy<Mutex<HashMap<String, Value>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

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
        role: MetadataProviderRole::Primary,
        extends_provider_ids: Vec::new(),
        attribution_text: "Metadata and artwork provided by The Movie Database (TMDB).".into(),
        attribution_url: "https://www.themoviedb.org/".into(),
        logo_light_url: Some(TMDB_LOGO_URL.into()),
        logo_dark_url: Some(TMDB_LOGO_URL.into()),
    }
}

pub(crate) fn metadata_item_kind(media_type: Option<&str>) -> MetadataItemKind {
    match media_type
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "movie" => MetadataItemKind::Movie,
        "tv" => MetadataItemKind::Show,
        "tv_season" => MetadataItemKind::Season,
        "tv_episode" => MetadataItemKind::Episode,
        "collection" => MetadataItemKind::Collection,
        "person" | "people" => MetadataItemKind::Person,
        "company" => MetadataItemKind::Company,
        _ => MetadataItemKind::Item,
    }
}

pub(crate) async fn search(
    settings: &MetadataSettings,
    query: &str,
    media_type: Option<&str>,
) -> Result<Vec<MetadataSearchResult>, String> {
    let provider = tmdb_provider_settings(settings)?;
    let api_key = tmdb_api_key_from_provider(&provider)?;
    let query = query.to_string();
    let language = provider.language;
    let expected_media_type = media_type.map(normalize_tmdb_search_media_type);
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
            .filter(|result| {
                expected_media_type
                    .as_deref()
                    .map(|expected| result.media_type == expected)
                    .unwrap_or(true)
            })
            .collect())
    })
    .await
}

fn normalize_tmdb_search_media_type(media_type: &str) -> String {
    match media_type {
        "series" => "tv".into(),
        other => other.into(),
    }
}

pub(crate) async fn fetch_snapshot(
    settings: &MetadataSettings,
    external_id: &str,
    media_type: &str,
    include_person_details: bool,
) -> Result<StoredMetadataSnapshot, String> {
    let provider = tmdb_provider_settings(settings)?;
    let api_key = tmdb_api_key_from_provider(&provider)?;
    let language = provider.language;
    let image_languages = tmdb_include_image_languages(&language);
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
                    Some(&image_languages),
                    Some("videos,images,release_dates,external_ids,credits"),
                )
                .map_err(|error| {
                    format!(
                        "TMDB details lookup for movie:{} failed: {}",
                        external_id_string, error
                    )
                })?;
            let payload_json = enriched_tmdb_payload_json(
                &client,
                &details,
                &language,
                &image_languages,
                include_person_details,
            );
            let mut snapshot = movie_snapshot_from_details(&external_id_string, &details);
            snapshot.provider_payload_json = payload_json;
            Ok(snapshot)
        }
        "tv" => {
            let client = TmdbApiClient::new_with_api_key(api_key);
            let details = client
                .tv_api()
                .get_tv_details(
                    external_id_number,
                    Some(&language),
                    Some(&image_languages),
                    Some("videos,images,content_ratings,external_ids,credits"),
                )
                .map_err(|error| {
                    format!(
                        "TMDB details lookup for tv:{} failed: {}",
                        external_id_string, error
                    )
                })?;
            let payload_json = enriched_tmdb_payload_json(
                &client,
                &details,
                &language,
                &image_languages,
                include_person_details,
            );
            let mut snapshot = tv_snapshot_from_details(&external_id_string, &details);
            snapshot.provider_payload_json = payload_json;
            Ok(snapshot)
        }
        other => Err(format!("Unsupported TMDB media type: {}", other)),
    })
    .await
}

fn tmdb_include_image_languages(language: &str) -> String {
    let base_language = language
        .split(['-', '_'])
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("en");

    if base_language.eq_ignore_ascii_case("null") {
        "null".into()
    } else {
        format!("{base_language},null")
    }
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

    if let Some(tmdb_id) = parsed.provider_id("tmdb").map(str::to_string) {
        let snapshot = fetch_snapshot(settings, &tmdb_id, "movie", false).await?;
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
    if let Some(tvdb_id) = parsed.provider_id("tvdb").map(str::to_string) {
        if let Some(result) = find_tmdb_movie_by_external_id(settings, &tvdb_id, "tvdb_id").await? {
            return Ok(Some(result));
        }
    }
    if let Some(imdb_id) = parsed.provider_id("imdb").map(str::to_string) {
        if let Some(result) = find_tmdb_movie_by_external_id(settings, &imdb_id, "imdb_id").await? {
            return Ok(Some(result));
        }
    }

    let mut best_result = None;
    let mut best_score = 0.0;
    for result in search(settings, &parsed.title, Some("movie")).await? {
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
    for result in search(settings, &query, Some("tv")).await? {
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

pub(crate) async fn load_show_descendant_targets(
    settings: &MetadataSettings,
    show_external_id: &str,
) -> Result<Vec<ProviderDescendantTarget>, String> {
    let provider = tmdb_provider_settings(settings)?;
    let api_key = tmdb_api_key_from_provider(&provider)?;
    let language = provider.language;
    let show_id = parse_external_id(show_external_id, "tv")?;
    let show_external_id = show_external_id.to_string();
    run_tmdb_blocking(move || {
        let client = TmdbApiClient::new_with_api_key(api_key);
        let details = client
            .tv_api()
            .get_tv_details(show_id, Some(&language), None, None)
            .map_err(|error| {
                format!(
                    "TMDB descendant lookup for tv:{} failed: {}",
                    show_external_id, error
                )
            })?;

        let mut season_numbers = details
            .seasons
            .unwrap_or_default()
            .into_iter()
            .filter_map(|season| season.season_number)
            .filter(|season_number| *season_number > 0)
            .collect::<Vec<_>>();
        season_numbers.sort_unstable();
        season_numbers.dedup();

        let mut targets = Vec::new();
        for season_number in season_numbers {
            let season_details = client
                .tv_seasons_api()
                .get_tv_season_details(show_id, season_number, Some(&language), None, None)
                .map_err(|error| {
                    format!(
                        "TMDB descendant lookup for tv:{} season {} failed: {}",
                        show_external_id, season_number, error
                    )
                })?;
            for episode in season_details.episodes.unwrap_or_default() {
                let Some(episode_number) = episode.episode_number.filter(|number| *number > 0)
                else {
                    continue;
                };
                targets.push(ProviderDescendantTarget {
                    season_number,
                    episode_number,
                    season_external_id: season_number.to_string(),
                    episode_external_id: episode_number.to_string(),
                });
            }
        }

        Ok(targets)
    })
    .await
}

pub(crate) async fn fetch_season_snapshot(
    settings: &MetadataSettings,
    show_external_id: &str,
    season_number: i32,
    include_person_details: bool,
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
            .get_tv_season_details(
                show_id,
                season_number,
                Some(&language),
                None,
                Some("credits"),
            )
            .map_err(|error| {
                format!(
                    "TMDB season lookup for tv:{}:season:{} failed: {}",
                    show_external_id, season_number, error
                )
            })?;
        let payload_json = enriched_tmdb_payload_json(
            &client,
            &details,
            &language,
            "null",
            include_person_details,
        );
        let mut snapshot = season_snapshot_from_details(&show_external_id, season_number, &details);
        snapshot.provider_payload_json = payload_json;
        Ok(snapshot)
    })
    .await
}

pub(crate) async fn fetch_episode_snapshot(
    settings: &MetadataSettings,
    show_external_id: &str,
    season_number: i32,
    episode_number: i32,
    include_person_details: bool,
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
                Some("credits"),
            )
            .map_err(|error| {
                format!(
                    "TMDB episode lookup for tv:{}:season:{}:episode:{} failed: {}",
                    show_external_id, season_number, episode_number, error
                )
            })?;
        let payload_json = enriched_tmdb_payload_json(
            &client,
            &details,
            &language,
            "null",
            include_person_details,
        );
        let mut snapshot = episode_snapshot_from_details(
            &show_external_id,
            season_number,
            episode_number,
            &details,
        );
        snapshot.provider_payload_json = payload_json;
        Ok(snapshot)
    })
    .await
}

pub(crate) async fn fetch_person_metadata(
    settings: &MetadataSettings,
    external_id: &str,
) -> Result<Option<ProviderMetadataPerson>, String> {
    let provider = tmdb_provider_settings(settings)?;
    let api_key = tmdb_api_key_from_provider(&provider)?;
    let language = provider.language.clone();
    let image_languages = tmdb_include_image_languages(&language);
    let person_id = external_id.trim().parse::<i32>().map_err(|_| {
        format!(
            "TMDB person external id must be numeric, got {}",
            external_id
        )
    })?;

    run_tmdb_blocking(move || {
        let client = TmdbApiClient::new_with_api_key(api_key);
        Ok(
            tmdb_cached_person_detail(&client, person_id, &language, &image_languages)
                .and_then(|person| tmdb_person_from_detail(&person, person_id)),
        )
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

fn tmdb_provider_settings(settings: &MetadataSettings) -> Result<MetadataProviderSettings, String> {
    provider_settings(settings, MetadataProviderId::Tmdb).map_err(|error| format!("TMDB {}", error))
}

fn tmdb_image_url(
    path: &str,
    size: &str,
) -> String {
    format!(
        "{}/{}/{}",
        TMDB_IMAGE_BASE,
        size,
        path.trim_start_matches('/')
    )
}

fn tmdb_season_external_id(
    show_external_id: &str,
    season_number: i32,
) -> String {
    format!("tv:{show_external_id}:season:{season_number}")
}

fn tmdb_episode_external_id(
    show_external_id: &str,
    season_number: i32,
    episode_number: i32,
) -> String {
    format!("tv:{show_external_id}:season:{season_number}:episode:{episode_number}")
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

fn enriched_tmdb_payload_json<T: serde::Serialize>(
    client: &TmdbApiClient,
    details: &T,
    language: &str,
    image_languages: &str,
    include_person_details: bool,
) -> Option<String> {
    let mut payload = serde_json::to_value(details).ok()?;
    enrich_tmdb_collection_payload(client, &mut payload, language);
    if include_person_details {
        enrich_tmdb_people_payload(client, &mut payload, language, image_languages);
    }
    serde_json::to_string(&payload).ok()
}

fn enrich_tmdb_collection_payload(
    client: &TmdbApiClient,
    payload: &mut Value,
    language: &str,
) {
    let Some(collection_id) = payload
        .get("belongs_to_collection")
        .and_then(|collection| collection.get("id"))
        .and_then(Value::as_i64)
        .and_then(|id| i32::try_from(id).ok())
    else {
        return;
    };
    let Some(mut collection_details) =
        tmdb_cached_collection_detail(client, collection_id, language)
    else {
        return;
    };

    if let (Some(original), Some(detailed)) = (
        payload
            .get("belongs_to_collection")
            .and_then(Value::as_object),
        collection_details.as_object_mut(),
    ) {
        for (key, value) in original {
            let detailed_value_missing = detailed.get(key).map(Value::is_null).unwrap_or(true);
            if detailed_value_missing {
                detailed.insert(key.clone(), value.clone());
            }
        }
    }

    if let Some(map) = payload.as_object_mut() {
        map.insert("belongs_to_collection".into(), collection_details);
    }
}

fn enrich_tmdb_people_payload(
    client: &TmdbApiClient,
    payload: &mut Value,
    language: &str,
    image_languages: &str,
) {
    let mut person_ids = Vec::new();
    if let Some(cast) = payload
        .get("credits")
        .and_then(|credits| credits.get("cast"))
        .and_then(Value::as_array)
    {
        person_ids.extend(
            cast.iter()
                .filter_map(|entry| entry.get("id").and_then(Value::as_i64)),
        );
    }
    for guest_stars in tmdb_guest_star_collections(payload) {
        person_ids.extend(
            guest_stars
                .iter()
                .filter_map(|entry| entry.get("id").and_then(Value::as_i64)),
        );
    }
    if let Some(crew) = payload
        .get("credits")
        .and_then(|credits| credits.get("crew"))
        .and_then(Value::as_array)
    {
        person_ids.extend(crew.iter().filter_map(|entry| {
            let job = entry.get("job").and_then(Value::as_str)?;
            matches_important_tmdb_crew_role(job)
                .then(|| entry.get("id").and_then(Value::as_i64))
                .flatten()
        }));
    }

    let mut seen = HashSet::new();
    let people = person_ids
        .into_iter()
        .filter(|id| seen.insert(*id))
        .take(40)
        .filter_map(|id| {
            let person_id = i32::try_from(id).ok()?;
            tmdb_cached_person_detail(client, person_id, language, image_languages)
                .map(|details| (id, details))
        })
        .collect::<Vec<_>>();

    if people.is_empty() {
        return;
    }

    if let Some(credits) = payload.get_mut("credits") {
        for collection_key in ["cast", "crew", "guest_stars"] {
            if let Some(entries) = credits
                .get_mut(collection_key)
                .and_then(Value::as_array_mut)
            {
                for entry in entries {
                    let Some(id) = entry.get("id").and_then(Value::as_i64) else {
                        continue;
                    };
                    let Some((_, person)) = people.iter().find(|(person_id, _)| *person_id == id)
                    else {
                        continue;
                    };
                    if let Some(map) = entry.as_object_mut() {
                        map.insert("koko_person".into(), person.clone());
                    }
                }
            }
        }
    }
    if let Some(entries) = payload.get_mut("guest_stars").and_then(Value::as_array_mut) {
        for entry in entries {
            let Some(id) = entry.get("id").and_then(Value::as_i64) else {
                continue;
            };
            let Some((_, person)) = people.iter().find(|(person_id, _)| *person_id == id) else {
                continue;
            };
            if let Some(map) = entry.as_object_mut() {
                map.insert("koko_person".into(), person.clone());
            }
        }
    }
}

fn tmdb_cached_person_detail(
    client: &TmdbApiClient,
    person_id: i32,
    language: &str,
    image_languages: &str,
) -> Option<Value> {
    let cache_key = metadata_response_cache_key(
        &MetadataProviderId::Tmdb,
        "person",
        &[
            &person_id.to_string(),
            language,
            image_languages,
        ],
    );
    if let Some(cached) = TMDB_PERSON_DETAIL_CACHE
        .lock()
        .ok()
        .and_then(|cache| cache.get(&cache_key).cloned())
    {
        return Some(cached);
    }
    if let Some(contents) = read_metadata_response_cache_text(&cache_key) {
        if let Ok(value) = serde_json::from_str::<Value>(&contents) {
            if let Ok(mut cache) = TMDB_PERSON_DETAIL_CACHE.lock() {
                cache.insert(cache_key.clone(), value.clone());
            }
            return Some(value);
        }
    }

    let details = client
        .people_api()
        .get_person_details(
            person_id,
            Some(language),
            Some(image_languages),
            Some("combined_credits,external_ids,images"),
        )
        .ok()?;
    let mut value = serde_json::to_value(details).ok()?;
    let known_for = tmdb_known_for_from_person_payload(&value);
    if let Some(map) = value.as_object_mut() {
        map.insert(
            "koko_known_for".into(),
            Value::Array(known_for.into_iter().map(Value::String).collect()),
        );
    }

    if let Ok(mut cache) = TMDB_PERSON_DETAIL_CACHE.lock() {
        if cache.len() > 5000 {
            cache.clear();
        }
        cache.insert(cache_key.clone(), value.clone());
    }
    write_metadata_response_cache_text(&cache_key, &value.to_string());
    Some(value)
}

fn tmdb_cached_collection_detail(
    client: &TmdbApiClient,
    collection_id: i32,
    language: &str,
) -> Option<Value> {
    let cache_key = metadata_response_cache_key(
        &MetadataProviderId::Tmdb,
        "collection",
        &[
            &collection_id.to_string(),
            language,
        ],
    );
    if let Some(cached) = TMDB_COLLECTION_DETAIL_CACHE
        .lock()
        .ok()
        .and_then(|cache| cache.get(&cache_key).cloned())
    {
        return Some(cached);
    }
    if let Some(contents) = read_metadata_response_cache_text(&cache_key) {
        if let Ok(value) = serde_json::from_str::<Value>(&contents) {
            if let Ok(mut cache) = TMDB_COLLECTION_DETAIL_CACHE.lock() {
                cache.insert(cache_key.clone(), value.clone());
            }
            return Some(value);
        }
    }

    let details = client
        .collections_api()
        .get_collection_details(collection_id, Some(language))
        .ok()?;
    let value = serde_json::to_value(details).ok()?;
    if let Ok(mut cache) = TMDB_COLLECTION_DETAIL_CACHE.lock() {
        if cache.len() > 5000 {
            cache.clear();
        }
        cache.insert(cache_key.clone(), value.clone());
    }
    write_metadata_response_cache_text(&cache_key, &value.to_string());
    Some(value)
}

fn tmdb_known_for_from_person_payload(payload: &Value) -> Vec<String> {
    let Some(combined_credits) = payload.get("combined_credits") else {
        return Vec::new();
    };
    let mut titles = Vec::new();
    for key in ["cast", "crew"] {
        if let Some(entries) = combined_credits.get(key).and_then(Value::as_array) {
            for entry in entries {
                if let Some(title) = entry
                    .get("title")
                    .or_else(|| entry.get("name"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|title| !title.is_empty())
                {
                    if !titles.iter().any(|existing| existing == title) {
                        titles.push(title.to_string());
                    }
                }
                if titles.len() >= 8 {
                    return titles;
                }
            }
        }
    }
    titles
}

fn matches_important_tmdb_crew_role(role: &str) -> bool {
    matches!(
        role,
        "Director"
            | "Writer"
            | "Screenplay"
            | "Story"
            | "Creator"
            | "Executive Producer"
            | "Producer"
            | "Original Music Composer"
            | "Composer"
            | "Director of Photography"
    )
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

pub(crate) fn metadata_details(snapshot: &StoredMetadataSnapshot) -> ProviderMetadataDetails {
    let Some(payload) = snapshot
        .provider_payload_json
        .as_deref()
        .and_then(|payload| serde_json::from_str::<Value>(payload).ok())
    else {
        return ProviderMetadataDetails::default();
    };

    let trailer = tmdb_trailer_entry(&payload);
    ProviderMetadataDetails {
        external_ids: tmdb_external_ids(&payload, snapshot),
        tagline: text_field(&payload, &["tagline"]),
        logo_url: tmdb_logo_url(&payload),
        genres: tmdb_genres(&payload),
        rating: payload
            .get("vote_average")
            .and_then(Value::as_f64)
            .map(|value| value as f32),
        content_rating: tmdb_content_rating(&payload),
        trailer_title: trailer
            .and_then(|entry| entry.get("name"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        trailer_url: trailer
            .and_then(|entry| {
                entry
                    .get("site")
                    .and_then(Value::as_str)
                    .zip(entry.get("key").and_then(Value::as_str))
            })
            .and_then(|(site, key)| youtube_watch_url(site, key)),
        collections: tmdb_collections(&payload),
        people: tmdb_people(&payload),
        ..ProviderMetadataDetails::default()
    }
}

fn tmdb_external_ids(
    payload: &Value,
    snapshot: &StoredMetadataSnapshot,
) -> Vec<ProviderExternalId> {
    let mut external_ids = Vec::new();
    push_external_id(&mut external_ids, "tmdb", Some(&snapshot.external_id));
    extend_tmdb_external_ids_from_value(&mut external_ids, payload);
    if let Some(ids) = payload.get("external_ids") {
        extend_tmdb_external_ids_from_value(&mut external_ids, ids);
    }
    external_ids
}

fn extend_tmdb_external_ids_from_value(
    external_ids: &mut Vec<ProviderExternalId>,
    value: &Value,
) {
    let Some(map) = value.as_object() else {
        return;
    };
    for (key, value) in map {
        let Some(source) = tmdb_external_id_source_from_key(key) else {
            continue;
        };
        push_external_id_value(external_ids, &source, value);
    }
}

fn tmdb_external_id_source_from_key(key: &str) -> Option<String> {
    let key = key.trim();
    if key.ends_with("_mid") {
        return normalize_external_id_source(key);
    }
    normalize_external_id_source(key.strip_suffix("_id")?)
}

fn push_external_id_value(
    external_ids: &mut Vec<ProviderExternalId>,
    source: &str,
    value: &Value,
) {
    let external_id = value
        .as_str()
        .map(str::to_string)
        .or_else(|| value.as_i64().map(|id| id.to_string()))
        .or_else(|| value.as_u64().map(|id| id.to_string()));
    push_external_id(external_ids, source, external_id.as_deref());
}

fn push_external_id(
    external_ids: &mut Vec<ProviderExternalId>,
    source: &str,
    external_id: Option<&str>,
) {
    let Some(external_id) = external_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    let Some(source) = normalize_external_id_source(source) else {
        return;
    };
    if external_ids
        .iter()
        .any(|existing| existing.source == source && existing.external_id == external_id)
    {
        return;
    }
    external_ids.push(ProviderExternalId {
        source,
        external_id: external_id.to_string(),
    });
}

pub(crate) async fn cache_person_assets(
    snapshot: &StoredMetadataSnapshot,
    data_dir: &str,
) -> Result<StoredMetadataSnapshot, String> {
    let Some(payload_json) = snapshot.provider_payload_json.as_deref() else {
        return Ok(snapshot.clone());
    };
    let mut payload =
        serde_json::from_str::<Value>(payload_json).map_err(|error| error.to_string())?;
    cache_tmdb_people_payload_images(&mut payload, snapshot, data_dir).await?;

    let mut next_snapshot = snapshot.clone();
    next_snapshot.provider_payload_json = Some(payload.to_string());
    Ok(next_snapshot)
}

async fn cache_tmdb_people_payload_images(
    payload: &mut Value,
    snapshot: &StoredMetadataSnapshot,
    data_dir: &str,
) -> Result<(), String> {
    if let Some(credits) = payload.get_mut("credits") {
        for collection_key in ["cast", "crew", "guest_stars"] {
            let Some(entries) = credits
                .get_mut(collection_key)
                .and_then(Value::as_array_mut)
            else {
                continue;
            };
            cache_tmdb_people_entries_images(entries, snapshot, data_dir).await;
        }
    }
    if let Some(entries) = payload.get_mut("guest_stars").and_then(Value::as_array_mut) {
        cache_tmdb_people_entries_images(entries, snapshot, data_dir).await;
    }
    Ok(())
}

async fn cache_tmdb_people_entries_images(
    entries: &mut [Value],
    snapshot: &StoredMetadataSnapshot,
    data_dir: &str,
) {
    for entry in entries {
        let Some(external_id) = person_external_id(entry) else {
            continue;
        };
        let image_url = entry
            .get("koko_person")
            .and_then(|person| person.get("profile_path"))
            .or_else(|| entry.get("profile_path"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(|path| {
                if path.starts_with("http://") || path.starts_with("https://") {
                    path.to_string()
                } else {
                    tmdb_image_url(path, "w185")
                }
            });
        let Some(image_url) = image_url else {
            continue;
        };
        let person_dir = managed_metadata_asset_dir(
            data_dir,
            snapshot.provider_id.clone(),
            &external_id,
            Some("person"),
            &snapshot.locale_key,
        );
        let cache_key = format!("{}_profile", snapshot.provider_id.as_storage_value());
        let Some(path) = try_cache_item_artwork(&image_url, &person_dir, &cache_key).await else {
            continue;
        };
        let cached_path = metadata_asset_db_path(data_dir, &path);
        if let Some(map) = entry.as_object_mut() {
            map.insert(
                "koko_cached_image_path".into(),
                Value::String(cached_path.clone()),
            );
            if let Some(person) = map.get_mut("koko_person").and_then(Value::as_object_mut) {
                person.insert("koko_cached_image_path".into(), Value::String(cached_path));
            }
        }
    }
}

fn tmdb_trailer_entry(payload: &Value) -> Option<&Value> {
    payload
        .get("videos")
        .and_then(|videos| videos.get("results"))
        .and_then(Value::as_array)
        .and_then(|videos| {
            videos
                .iter()
                .find(|video| {
                    video.get("site").and_then(Value::as_str) == Some("YouTube")
                        && video.get("type").and_then(Value::as_str) == Some("Trailer")
                        && video
                            .get("official")
                            .and_then(Value::as_bool)
                            .unwrap_or(false)
                })
                .or_else(|| {
                    videos.iter().find(|video| {
                        video.get("site").and_then(Value::as_str) == Some("YouTube")
                            && video.get("type").and_then(Value::as_str) == Some("Trailer")
                    })
                })
                .or_else(|| {
                    videos
                        .iter()
                        .find(|video| video.get("site").and_then(Value::as_str) == Some("YouTube"))
                })
        })
}

fn youtube_watch_url(
    site: &str,
    key: &str,
) -> Option<String> {
    site.eq_ignore_ascii_case("YouTube")
        .then(|| key.trim())
        .filter(|key| !key.is_empty())
        .and_then(crate::metadata::youtube_watch_url)
}

fn tmdb_logo_url(payload: &Value) -> Option<String> {
    payload
        .get("images")
        .and_then(|images| images.get("logos"))
        .and_then(Value::as_array)
        .and_then(|logos| {
            preferred_image_url_by_format(logos.iter().filter_map(|logo| {
                logo.get("file_path")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|path| !path.is_empty())
                    .map(|path| tmdb_image_url(path, "w500"))
            }))
        })
}

fn tmdb_genres(payload: &Value) -> Vec<String> {
    payload
        .get("genres")
        .and_then(Value::as_array)
        .map(|genres| {
            genres
                .iter()
                .filter_map(|genre| genre.get("name").and_then(Value::as_str))
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn tmdb_content_rating(payload: &Value) -> Option<String> {
    payload
        .get("release_dates")
        .and_then(|release_dates| release_dates.get("results"))
        .and_then(Value::as_array)
        .and_then(|countries| {
            countries
                .iter()
                .find(|country| country.get("iso_3166_1").and_then(Value::as_str) == Some("US"))
                .or_else(|| countries.first())
        })
        .and_then(|country| country.get("release_dates"))
        .and_then(Value::as_array)
        .and_then(|dates| {
            dates.iter().find_map(|date| {
                date.get("certification")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
            })
        })
        .or_else(|| {
            payload
                .get("content_ratings")
                .and_then(|ratings| ratings.get("results"))
                .and_then(Value::as_array)
                .and_then(|ratings| {
                    ratings
                        .iter()
                        .find(|rating| {
                            rating.get("iso_3166_1").and_then(Value::as_str) == Some("US")
                        })
                        .or_else(|| ratings.first())
                })
                .and_then(|rating| rating.get("rating"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
}

fn tmdb_collections(payload: &Value) -> Vec<ProviderMetadataCollection> {
    let Some(collection) = payload.get("belongs_to_collection") else {
        return Vec::new();
    };
    let Some(external_id) = collection.get("id").and_then(Value::as_i64) else {
        return Vec::new();
    };
    let Some(name) = collection
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
    else {
        return Vec::new();
    };

    vec![ProviderMetadataCollection {
        external_id: external_id.to_string(),
        name: Some(name.to_string()),
        overview: text_field(collection, &["overview"]),
        artwork_url: collection
            .get("poster_path")
            .and_then(Value::as_str)
            .map(|path| tmdb_image_url(path, "w500")),
        backdrop_url: collection
            .get("backdrop_path")
            .and_then(Value::as_str)
            .map(|path| tmdb_image_url(path, "w1280")),
        theme_song_url: None,
    }]
}

fn tmdb_guest_star_collections(payload: &Value) -> Vec<&Vec<Value>> {
    let mut collections = Vec::new();
    if let Some(guest_stars) = payload.get("guest_stars").and_then(Value::as_array) {
        collections.push(guest_stars);
    }
    if let Some(guest_stars) = payload
        .get("credits")
        .and_then(|credits| credits.get("guest_stars"))
        .and_then(Value::as_array)
    {
        collections.push(guest_stars);
    }
    collections
}

fn tmdb_people(payload: &Value) -> Vec<ProviderMetadataPerson> {
    let Some(credits) = payload.get("credits") else {
        let mut people = Vec::new();
        extend_tmdb_guest_stars(&mut people, payload);
        return sort_and_dedupe_people(people);
    };

    let mut people = Vec::new();
    if let Some(cast) = credits.get("cast").and_then(Value::as_array) {
        people.extend(cast.iter().enumerate().filter_map(|(index, entry)| {
            let name = person_name(entry)?;
            Some(ProviderMetadataPerson {
                external_id: person_external_id(entry),
                external_ids: tmdb_person_external_ids(entry, None),
                name,
                known_for: tmdb_person_known_for(entry),
                biography: tmdb_person_detail(entry, "biography"),
                gender: tmdb_person_gender(entry),
                birthday: tmdb_person_detail(entry, "birthday"),
                deathday: tmdb_person_detail(entry, "deathday"),
                birth_place: tmdb_person_detail(entry, "place_of_birth"),
                role: Some("Actor".into()),
                department: Some("Cast".into()),
                character_name: text_field(entry, &["character"]),
                profile_url: person_external_id(entry)
                    .map(|id| format!("https://www.themoviedb.org/person/{id}")),
                image_url: entry
                    .get("profile_path")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|path| tmdb_image_url(path, "w185")),
                cached_image_path: text_field(entry, &["koko_cached_image_path"]),
                sort_order: entry
                    .get("order")
                    .and_then(Value::as_i64)
                    .and_then(|order| i32::try_from(order).ok())
                    .unwrap_or_else(|| i32::try_from(index).unwrap_or(i32::MAX)),
            })
        }));
    }

    extend_tmdb_guest_stars(&mut people, payload);

    if let Some(crew) = credits.get("crew").and_then(Value::as_array) {
        let mut crew_order = 10_000;
        people.extend(crew.iter().filter_map(|entry| {
            let job = text_field(entry, &["job"])?;
            if !matches_important_tmdb_crew_role(&job) {
                return None;
            }
            let name = person_name(entry)?;
            let sort_order = crew_order;
            crew_order += 1;
            Some(ProviderMetadataPerson {
                external_id: person_external_id(entry),
                external_ids: tmdb_person_external_ids(entry, None),
                name,
                known_for: tmdb_person_known_for(entry),
                biography: tmdb_person_detail(entry, "biography"),
                gender: tmdb_person_gender(entry),
                birthday: tmdb_person_detail(entry, "birthday"),
                deathday: tmdb_person_detail(entry, "deathday"),
                birth_place: tmdb_person_detail(entry, "place_of_birth"),
                role: Some(job),
                department: text_field(entry, &["department"]).or_else(|| Some("Crew".into())),
                character_name: None,
                profile_url: person_external_id(entry)
                    .map(|id| format!("https://www.themoviedb.org/person/{id}")),
                image_url: entry
                    .get("profile_path")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|path| tmdb_image_url(path, "w185")),
                cached_image_path: text_field(entry, &["koko_cached_image_path"]),
                sort_order,
            })
        }));
    }

    sort_and_dedupe_people(people)
}

fn extend_tmdb_guest_stars(
    people: &mut Vec<ProviderMetadataPerson>,
    payload: &Value,
) {
    let mut guest_order = 5_000;
    for guest_stars in tmdb_guest_star_collections(payload) {
        people.extend(guest_stars.iter().filter_map(|entry| {
            let name = person_name(entry)?;
            let sort_order = entry
                .get("order")
                .and_then(Value::as_i64)
                .and_then(|order| i32::try_from(order).ok())
                .map(|order| 5_000 + order)
                .unwrap_or_else(|| {
                    let sort_order = guest_order;
                    guest_order += 1;
                    sort_order
                });
            Some(ProviderMetadataPerson {
                external_id: person_external_id(entry),
                external_ids: tmdb_person_external_ids(entry, None),
                name,
                known_for: tmdb_person_known_for(entry),
                biography: tmdb_person_detail(entry, "biography"),
                gender: tmdb_person_gender(entry),
                birthday: tmdb_person_detail(entry, "birthday"),
                deathday: tmdb_person_detail(entry, "deathday"),
                birth_place: tmdb_person_detail(entry, "place_of_birth"),
                role: Some("Guest Star".into()),
                department: Some("Cast".into()),
                character_name: text_field(entry, &["character"]),
                profile_url: person_external_id(entry)
                    .map(|id| format!("https://www.themoviedb.org/person/{id}")),
                image_url: entry
                    .get("profile_path")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|path| tmdb_image_url(path, "w185")),
                cached_image_path: text_field(entry, &["koko_cached_image_path"]),
                sort_order,
            })
        }));
    }
}

fn tmdb_person_from_detail(
    person: &Value,
    fallback_id: i32,
) -> Option<ProviderMetadataPerson> {
    let name = person_name(person)?;
    let fallback_id_string = fallback_id.to_string();
    Some(ProviderMetadataPerson {
        external_id: person_external_id(person).or_else(|| Some(fallback_id_string.clone())),
        external_ids: tmdb_person_external_ids(person, Some(&fallback_id_string)),
        name,
        known_for: person
            .get("koko_known_for")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        biography: text_field(person, &["biography"]),
        gender: tmdb_person_gender_from_value(person),
        birthday: text_field(person, &["birthday"]),
        deathday: text_field(person, &["deathday"]),
        birth_place: text_field(person, &["place_of_birth"]),
        role: None,
        department: None,
        character_name: None,
        profile_url: person_external_id(person)
            .or_else(|| Some(fallback_id.to_string()))
            .map(|id| format!("https://www.themoviedb.org/person/{id}")),
        image_url: person
            .get("profile_path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|path| tmdb_image_url(path, "w185")),
        cached_image_path: text_field(person, &["koko_cached_image_path"]),
        sort_order: 0,
    })
}

fn tmdb_person_external_ids(
    value: &Value,
    fallback_external_id: Option<&str>,
) -> Vec<ProviderExternalId> {
    let details = value.get("koko_person");
    let mut external_ids = Vec::new();
    let primary_external_id = person_external_id(value)
        .or_else(|| details.and_then(person_external_id))
        .or_else(|| fallback_external_id.map(ToOwned::to_owned));
    push_external_id(&mut external_ids, "tmdb", primary_external_id.as_deref());

    if fallback_external_id.is_some() {
        extend_tmdb_external_ids_from_value(&mut external_ids, value);
    }
    if let Some(ids) = value.get("external_ids") {
        extend_tmdb_external_ids_from_value(&mut external_ids, ids);
    }
    if let Some(details) = details {
        extend_tmdb_external_ids_from_value(&mut external_ids, details);
        if let Some(ids) = details.get("external_ids") {
            extend_tmdb_external_ids_from_value(&mut external_ids, ids);
        }
    }
    external_ids
}

fn tmdb_person_detail(
    credit: &Value,
    key: &str,
) -> Option<String> {
    credit
        .get("koko_person")
        .and_then(|person| person.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn tmdb_person_gender(credit: &Value) -> Option<String> {
    let gender = credit
        .get("koko_person")
        .and_then(|person| person.get("gender"))
        .and_then(Value::as_i64)?;
    tmdb_gender_label(gender)
}

fn tmdb_person_gender_from_value(person: &Value) -> Option<String> {
    let gender = person.get("gender").and_then(Value::as_i64)?;
    tmdb_gender_label(gender)
}

fn tmdb_gender_label(gender: i64) -> Option<String> {
    match gender {
        1 => Some("Female".into()),
        2 => Some("Male".into()),
        3 => Some("Non-binary".into()),
        _ => None,
    }
}

fn tmdb_person_known_for(credit: &Value) -> Vec<String> {
    credit
        .get("koko_person")
        .and_then(|person| person.get("koko_known_for"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .take(8)
                .collect()
        })
        .unwrap_or_default()
}

fn person_name(value: &Value) -> Option<String> {
    text_field(
        value,
        &[
            "name",
            "original_name",
            "fullName",
        ],
    )
}

fn person_external_id(value: &Value) -> Option<String> {
    value
        .get("id")
        .or_else(|| value.get("peopleId"))
        .or_else(|| value.get("personId"))
        .and_then(|id| {
            id.as_i64()
                .map(|id| id.to_string())
                .or_else(|| id.as_str().map(str::to_string))
        })
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
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

fn sort_and_dedupe_people(people: Vec<ProviderMetadataPerson>) -> Vec<ProviderMetadataPerson> {
    let mut seen = HashSet::new();
    let mut people = people
        .into_iter()
        .filter(|person| {
            let key = format!(
                "{}:{}:{}",
                person.external_id.as_deref().unwrap_or(""),
                person.name.to_ascii_lowercase(),
                person.role.as_deref().unwrap_or("")
            );
            seen.insert(key)
        })
        .collect::<Vec<_>>();
    people.sort_by(|left, right| {
        left.sort_order
            .cmp(&right.sort_order)
            .then_with(|| left.department.cmp(&right.department))
            .then_with(|| left.name.cmp(&right.name))
    });
    people.truncate(80);
    people
}

#[cfg(test)]
mod tests {
    use super::{
        metadata_details,
        tmdb_logo_url,
    };
    use crate::config::MetadataProviderId;
    use crate::metadata::StoredMetadataSnapshot;
    use serde_json::json;

    #[test]
    fn tmdb_logo_url_prefers_svg_over_png() {
        let payload = json!({
            "images": {
                "logos": [
                    { "file_path": "/logo.png" },
                    { "file_path": "/logo.svg" }
                ]
            }
        });

        assert_eq!(
            tmdb_logo_url(&payload).as_deref(),
            Some("https://image.tmdb.org/t/p/w500/logo.svg")
        );
    }

    #[test]
    fn tmdb_logo_url_falls_back_to_png_when_svg_missing() {
        let payload = json!({
            "images": {
                "logos": [
                    { "file_path": "/logo.jpg" },
                    { "file_path": "/logo.png" }
                ]
            }
        });

        assert_eq!(
            tmdb_logo_url(&payload).as_deref(),
            Some("https://image.tmdb.org/t/p/w500/logo.png")
        );
    }

    #[test]
    fn tmdb_metadata_details_collects_known_external_ids() {
        let snapshot = StoredMetadataSnapshot {
            provider_id: MetadataProviderId::Tmdb,
            external_id: "603".into(),
            media_type: Some("movie".into()),
            title: Some("The Matrix".into()),
            overview: None,
            artwork_url: None,
            backdrop_url: None,
            release_year: Some(1999),
            locale_key: crate::metadata::DEFAULT_METADATA_LOCALE.into(),
            provider_locale_key: Some("en-US".into()),
            provider_payload_json: Some(
                json!({
                    "imdb_id": "tt0133093",
                    "external_ids": {
                        "wikidata_id": "Q83495",
                        "facebook_id": "thematrixmovie",
                        "freebase_mid": "/m/0f2y0"
                    }
                })
                .to_string(),
            ),
        };

        let details = metadata_details(&snapshot);

        assert!(
            details
                .external_ids
                .iter()
                .any(|id| { id.source == "tmdb" && id.external_id == "603" })
        );
        assert!(
            details
                .external_ids
                .iter()
                .any(|id| { id.source == "imdb" && id.external_id == "tt0133093" })
        );
        assert!(
            details
                .external_ids
                .iter()
                .any(|id| { id.source == "wikidata" && id.external_id == "Q83495" })
        );
        assert!(
            details
                .external_ids
                .iter()
                .any(|id| { id.source == "facebook" && id.external_id == "thematrixmovie" })
        );
        assert!(
            details
                .external_ids
                .iter()
                .any(|id| { id.source == "freebase_mid" && id.external_id == "/m/0f2y0" })
        );
    }

    #[test]
    fn tmdb_metadata_details_use_shallow_credit_people_without_person_detail_payload() {
        let snapshot = StoredMetadataSnapshot {
            provider_id: MetadataProviderId::Tmdb,
            external_id: "603".into(),
            media_type: Some("movie".into()),
            title: Some("The Matrix".into()),
            overview: None,
            artwork_url: None,
            backdrop_url: None,
            release_year: Some(1999),
            locale_key: crate::metadata::DEFAULT_METADATA_LOCALE.into(),
            provider_locale_key: Some("en-US".into()),
            provider_payload_json: Some(
                json!({
                    "credits": {
                        "cast": [
                            {
                                "cast_id": 7,
                                "credit_id": "52fe425bc3a36847f80181c7",
                                "id": 6384,
                                "name": "Keanu Reeves",
                                "character": "Neo",
                                "order": 0,
                                "profile_path": "/keanu.jpg"
                            }
                        ],
                        "crew": []
                    }
                })
                .to_string(),
            ),
        };

        let details = metadata_details(&snapshot);

        assert_eq!(details.people.len(), 1);
        assert_eq!(details.people[0].name, "Keanu Reeves");
        assert_eq!(details.people[0].biography, None);
        assert_eq!(details.people[0].external_id.as_deref(), Some("6384"));
        assert_eq!(details.people[0].character_name.as_deref(), Some("Neo"));
    }

    #[test]
    fn tmdb_people_include_external_ids_from_person_detail_payload() {
        let snapshot = StoredMetadataSnapshot {
            provider_id: MetadataProviderId::Tmdb,
            external_id: "603".into(),
            media_type: Some("movie".into()),
            title: Some("The Matrix".into()),
            overview: None,
            artwork_url: None,
            backdrop_url: None,
            release_year: Some(1999),
            locale_key: crate::metadata::DEFAULT_METADATA_LOCALE.into(),
            provider_locale_key: Some("en-US".into()),
            provider_payload_json: Some(
                json!({
                    "credits": {
                        "cast": [
                            {
                                "id": 6384,
                                "name": "Keanu Reeves",
                                "character": "Neo",
                                "order": 0,
                                "koko_person": {
                                    "id": 6384,
                                    "name": "Keanu Reeves",
                                    "external_ids": {
                                        "imdb_id": "nm0000206",
                                        "wikidata_id": "Q43416",
                                        "freebase_mid": "/m/01vvycq"
                                    }
                                }
                            }
                        ],
                        "crew": []
                    }
                })
                .to_string(),
            ),
        };

        let details = metadata_details(&snapshot);

        assert_eq!(details.people.len(), 1);
        assert!(
            details.people[0]
                .external_ids
                .iter()
                .any(|id| { id.source == "tmdb" && id.external_id == "6384" })
        );
        assert!(
            details.people[0]
                .external_ids
                .iter()
                .any(|id| { id.source == "imdb" && id.external_id == "nm0000206" })
        );
        assert!(
            details.people[0]
                .external_ids
                .iter()
                .any(|id| { id.source == "wikidata" && id.external_id == "Q43416" })
        );
        assert!(
            details.people[0]
                .external_ids
                .iter()
                .any(|id| { id.source == "freebase_mid" && id.external_id == "/m/01vvycq" })
        );
        assert!(
            !details.people[0]
                .external_ids
                .iter()
                .any(|id| id.source == "cast" || id.source == "credit")
        );
    }

    #[test]
    fn tmdb_metadata_details_include_episode_guest_stars_as_shallow_people() {
        let snapshot = StoredMetadataSnapshot {
            provider_id: MetadataProviderId::Tmdb,
            external_id: "tv:1396:season:1:episode:1".into(),
            media_type: Some("tv_episode".into()),
            title: Some("Pilot".into()),
            overview: None,
            artwork_url: None,
            backdrop_url: None,
            release_year: Some(2008),
            locale_key: crate::metadata::DEFAULT_METADATA_LOCALE.into(),
            provider_locale_key: Some("en-US".into()),
            provider_payload_json: Some(
                json!({
                    "guest_stars": [
                        {
                            "id": 1111,
                            "name": "Guest One",
                            "character": "Episode Character",
                            "order": 2,
                            "profile_path": "/guest-one.jpg"
                        }
                    ],
                    "credits": {
                        "cast": [],
                        "crew": [],
                        "guest_stars": [
                            {
                                "id": 2222,
                                "name": "Guest Two",
                                "character": "Another Character",
                                "order": 1,
                                "profile_path": "/guest-two.jpg"
                            }
                        ]
                    }
                })
                .to_string(),
            ),
        };

        let details = metadata_details(&snapshot);

        assert_eq!(details.people.len(), 2);
        assert_eq!(details.people[0].name, "Guest Two");
        assert_eq!(details.people[0].role.as_deref(), Some("Guest Star"));
        assert_eq!(details.people[0].department.as_deref(), Some("Cast"));
        assert_eq!(
            details.people[0].character_name.as_deref(),
            Some("Another Character")
        );
        assert_eq!(details.people[0].biography, None);
        assert_eq!(
            details.people[0].image_url.as_deref(),
            Some("https://image.tmdb.org/t/p/w185/guest-two.jpg")
        );
        assert_eq!(details.people[1].name, "Guest One");
        assert_eq!(details.people[1].role.as_deref(), Some("Guest Star"));
    }
}
