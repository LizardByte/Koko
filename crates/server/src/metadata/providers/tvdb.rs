use serde_json::Value;
use strsim::normalized_levenshtein;
use tvdb4::apis::{self, login_api};
use tvdb4::models::LoginPostRequest;

use crate::config::{MetadataProviderId, MetadataProviderSettings, MetadataSettings};
use crate::metadata::{
    MediaLibraryKind, MetadataItemKind, MetadataProviderDescriptor, MetadataSearchResult,
    StoredMetadataSnapshot, TVDB_API_BASE, TVDB_AUTH_TOKEN, TVDB_RATE_LIMITER, TvdbCachedToken,
    TvdbDescendantTarget, cleanup_movie_title, extract_release_year, format_payload_snippet,
    movie_match_score, parse_movie_name, provider_settings, show_search_query,
};
use std::time::{Duration, Instant};

pub(crate) fn descriptor() -> MetadataProviderDescriptor {
    MetadataProviderDescriptor {
        id: MetadataProviderId::Tvdb,
        display_name: "TheTVDB".into(),
        description: "Alternative movie and television metadata provider with strong series and episode coverage.".into(),
        supported_kinds: vec![MediaLibraryKind::Movies, MediaLibraryKind::Shows],
        requires_api_key: true,
        implemented: true,
        attribution_text: "Metadata and artwork provided by TheTVDB.".into(),
        attribution_url: "https://thetvdb.com/".into(),
        logo_light_url: Some("https://thetvdb.com/images/attribution/logo2.png".into()),
        logo_dark_url: Some("https://thetvdb.com/images/attribution/logo1.png".into()),
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
        "series" | "tv" => MetadataItemKind::Show,
        "season" => MetadataItemKind::Season,
        "episode" => MetadataItemKind::Episode,
        "list" => MetadataItemKind::Collection,
        "people" | "person" | "actor" => MetadataItemKind::Person,
        "company" => MetadataItemKind::Company,
        "award" => MetadataItemKind::Award,
        _ => MetadataItemKind::Item,
    }
}

pub(crate) async fn search(
    settings: &MetadataSettings,
    query: &str,
) -> Result<Vec<MetadataSearchResult>, String> {
    let provider = provider_settings(settings, MetadataProviderId::Tvdb)
        .map_err(|error| format!("TheTVDB {}", error))?;
    let payload = get_json(
        &provider,
        "search",
        vec![
            ("query", query.to_string()),
            ("limit", "20".to_string()),
        ],
        &format!("search query {:?}", query),
    )
    .await?;
    let results = payload
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Ok(results
        .into_iter()
        .filter_map(search_result_from_value)
        .collect())
}

pub(crate) async fn fetch_snapshot(
    settings: &MetadataSettings,
    external_id: &str,
    media_type: &str,
) -> Result<StoredMetadataSnapshot, String> {
    match media_type {
        "movie" => {
            let provider = provider_settings(settings, MetadataProviderId::Tvdb)
                .map_err(|error| format!("TheTVDB {}", error))?;
            let payload = fetch_movie_payload(settings, external_id).await?;
            let translation =
                fetch_translation_payload(&provider, "movies", external_id, &provider.language)
                    .await;
            Ok(movie_snapshot_from_value(
                external_id,
                &payload,
                translation.as_ref(),
                &provider.language,
            ))
        }
        "series" | "tv" => {
            let provider = provider_settings(settings, MetadataProviderId::Tvdb)
                .map_err(|error| format!("TheTVDB {}", error))?;
            let payload = fetch_series_payload(settings, external_id).await?;
            let translation =
                fetch_translation_payload(&provider, "series", external_id, &provider.language)
                    .await;
            Ok(series_snapshot_from_value(
                external_id,
                &payload,
                translation.as_ref(),
                &provider.language,
            ))
        }
        "season" => fetch_season_snapshot(settings, external_id, 0, external_id).await,
        "episode" => fetch_episode_snapshot(settings, external_id, 0, 0, external_id).await,
        other => Err(format!("Unsupported TheTVDB media type: {}", other)),
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

    if let Some(tvdb_id) = parsed.tvdb_id.clone() {
        let snapshot = fetch_snapshot(settings, &tvdb_id, "movie").await?;
        return Ok(Some(MetadataSearchResult {
            provider_id: MetadataProviderId::Tvdb,
            external_id: tvdb_id,
            media_type: "movie".into(),
            title: snapshot.title.unwrap_or(parsed.title),
            overview: snapshot.overview,
            artwork_url: snapshot.artwork_url,
            backdrop_url: snapshot.backdrop_url,
            release_year: snapshot.release_year,
            score: Some(1.0),
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
        if result.media_type != "series" {
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
    season_external_id: &str,
) -> Result<StoredMetadataSnapshot, String> {
    let provider = provider_settings(settings, MetadataProviderId::Tvdb)
        .map_err(|error| format!("TheTVDB {}", error))?;
    let season_id = parse_tvdb_external_id(season_external_id, "season")?;
    let payload = get_json(
        &provider,
        &format!("seasons/{season_id}/extended"),
        vec![("meta", "translations".to_string())],
        &format!("season lookup for series:{show_external_id}:season:{season_external_id}"),
    )
    .await?;
    let translation =
        fetch_translation_payload(&provider, "seasons", season_external_id, &provider.language)
            .await;
    Ok(season_snapshot_from_value(
        show_external_id,
        season_number,
        season_external_id,
        &payload,
        translation.as_ref(),
        &provider.language,
    ))
}

pub(crate) async fn fetch_episode_snapshot(
    settings: &MetadataSettings,
    show_external_id: &str,
    season_number: i32,
    episode_number: i32,
    episode_external_id: &str,
) -> Result<StoredMetadataSnapshot, String> {
    let provider = provider_settings(settings, MetadataProviderId::Tvdb)
        .map_err(|error| format!("TheTVDB {}", error))?;
    let episode_id = parse_tvdb_external_id(episode_external_id, "episode")?;
    let payload = get_json(
        &provider,
        &format!("episodes/{episode_id}/extended"),
        vec![("meta", "translations".to_string())],
        &format!(
            "episode lookup for series:{show_external_id}:season:{season_number}:episode:{episode_external_id}"
        ),
    )
    .await?;
    let translation = fetch_translation_payload(
        &provider,
        "episodes",
        episode_external_id,
        &provider.language,
    )
    .await;
    Ok(episode_snapshot_from_value(
        show_external_id,
        season_number,
        episode_number,
        episode_external_id,
        &payload,
        translation.as_ref(),
        &provider.language,
    ))
}

pub(crate) async fn load_show_descendant_targets(
    settings: &MetadataSettings,
    show_external_id: &str,
) -> Result<Vec<TvdbDescendantTarget>, String> {
    let provider = provider_settings(settings, MetadataProviderId::Tvdb)
        .map_err(|error| format!("TheTVDB {}", error))?;
    let show_id = parse_tvdb_external_id(show_external_id, "series")?;
    let series_payload = get_json(
        &provider,
        &format!("series/{show_id}/extended"),
        vec![("meta", "translations".to_string())],
        &format!("series descendant lookup for {show_external_id}"),
    )
    .await?;

    let mut targets = Vec::new();
    for season in series_payload
        .get("data")
        .and_then(|value| value.get("seasons"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        let Some(season_id) = object_id(&season) else {
            continue;
        };
        let Some(season_number) = season_number(&season) else {
            continue;
        };

        let season_payload = get_json(
            &provider,
            &format!("seasons/{season_id}/extended"),
            vec![("meta", "translations".to_string())],
            &format!("season descendant lookup for {season_id}"),
        )
        .await?;
        let episodes = season_payload
            .get("data")
            .and_then(|value| value.get("episodes"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for episode in episodes {
            let Some(episode_id) = object_id(&episode) else {
                continue;
            };
            let Some(episode_number) = episode_number(&episode) else {
                continue;
            };
            targets.push(TvdbDescendantTarget {
                season_number,
                episode_number,
                season_external_id: season_id.to_string(),
                episode_external_id: episode_id.to_string(),
            });
        }
    }

    Ok(targets)
}

fn tvdb_configuration(token: Option<String>) -> apis::configuration::Configuration {
    let mut config = apis::configuration::Configuration::new();
    config.base_path = TVDB_API_BASE.to_string();
    config.user_agent = Some(format!("Koko/{}", env!("CARGO_PKG_VERSION")));
    config.bearer_access_token = token;
    config
}

fn parse_tvdb_external_id(
    external_id: &str,
    media_type: &str,
) -> Result<f32, String> {
    external_id.parse::<f32>().map_err(|_| {
        format!(
            "TheTVDB {} external id must be numeric, got {:?}",
            media_type, external_id
        )
    })
}

async fn wait_for_rate_limit(provider: &MetadataProviderSettings) {
    let requests_per_second = provider.rate_limit_per_second.max(1);
    let interval = Duration::from_secs_f64(1.0 / f64::from(requests_per_second));
    let mut next_available_at = TVDB_RATE_LIMITER.lock().await;
    let now = Instant::now();
    if *next_available_at > now {
        tokio::time::sleep((*next_available_at).saturating_duration_since(now)).await;
    }
    let base = Instant::now();
    *next_available_at = base.checked_add(interval).unwrap_or(base);
}

async fn auth_token(provider: &MetadataProviderSettings) -> Result<String, String> {
    let now = Instant::now();
    {
        let cache = TVDB_AUTH_TOKEN.lock().await;
        if let Some(cached) = cache.as_ref().filter(|cached| cached.expires_at > now) {
            return Ok(cached.token.clone());
        }
    }

    let api_key = provider
        .api_key
        .clone()
        .unwrap_or_default()
        .trim()
        .to_string();
    if api_key.is_empty() {
        return Err("TheTVDB is enabled but no API key is configured.".into());
    }

    wait_for_rate_limit(provider).await;
    let config = tvdb_configuration(None);
    let response = login_api::login_post(&config, LoginPostRequest::new(api_key))
        .await
        .map_err(|error| format_tvdb_error("login", error))?;
    let token = response
        .data
        .and_then(|data| data.token)
        .map(|token| token.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "TheTVDB login response did not include a token.".to_string())?;

    let cached = TvdbCachedToken {
        token: token.clone(),
        expires_at: Instant::now() + Duration::from_secs(60 * 60 * 24 * 25),
    };
    let mut cache = TVDB_AUTH_TOKEN.lock().await;
    *cache = Some(cached);
    Ok(token)
}

fn format_tvdb_error<T: std::fmt::Debug>(
    context: &str,
    error: apis::Error<T>,
) -> String {
    match error {
        apis::Error::ResponseError(response) => format!(
            "TheTVDB {} failed with status {}{}",
            context,
            response.status,
            format_payload_snippet(&response.content)
        ),
        other => format!("TheTVDB {} request failed: {}", context, other),
    }
}

async fn get_text(
    provider: &MetadataProviderSettings,
    path: &str,
    query: Vec<(&'static str, String)>,
    context: &str,
) -> Result<String, String> {
    let retry_attempts = usize::try_from(provider.retry_attempts).unwrap_or(0);
    let base_backoff = Duration::from_millis(u64::from(provider.retry_backoff_ms.max(1)));

    for attempt in 0..=retry_attempts {
        wait_for_rate_limit(provider).await;
        let token = auth_token(provider).await?;
        let config = tvdb_configuration(Some(token));
        let request_url = format!("{}/{}", TVDB_API_BASE, path.trim_start_matches('/'));
        let mut request = config
            .client
            .get(&request_url)
            .bearer_auth(config.bearer_access_token.as_deref().unwrap_or_default())
            .query(&query);
        if let Some(user_agent) = config.user_agent.as_deref() {
            request = request.header("user-agent", user_agent);
        }
        let response = request.send().await;

        match response {
            Ok(response) => {
                let status = response.status();
                let retry_after = response
                    .headers()
                    .get("retry-after")
                    .and_then(|value| value.to_str().ok())
                    .and_then(parse_retry_after_seconds)
                    .map(Duration::from_secs);
                let payload = response.text().await.map_err(|error| error.to_string())?;
                if status.is_success() {
                    return Ok(payload);
                }

                if status.as_u16() == 401 {
                    let mut cache = TVDB_AUTH_TOKEN.lock().await;
                    *cache = None;
                }

                let rate_limited = status.as_u16() == 429
                    || retry_after.is_some()
                    || payload.to_ascii_lowercase().contains("rate limit");
                let retryable = status.as_u16() == 401 || rate_limited || status.is_server_error();
                if retryable && attempt < retry_attempts {
                    let attempt_number = attempt + 1;
                    let multiplier = 1_u32
                        .checked_shl(u32::try_from(attempt).unwrap_or(0))
                        .unwrap_or(u32::MAX);
                    let backoff =
                        retry_after.unwrap_or_else(|| base_backoff.saturating_mul(multiplier));
                    log::warn!(
                        "TheTVDB request retry scheduled for {} after status {}{}{} (attempt {}/{} in {} ms)",
                        context,
                        status,
                        if rate_limited { " [rate limited]" } else { "" },
                        format_payload_snippet(&payload),
                        attempt_number,
                        retry_attempts + 1,
                        backoff.as_millis()
                    );
                    tokio::time::sleep(backoff).await;
                    continue;
                }

                return Err(format!(
                    "TheTVDB {} failed with status {}{}{}",
                    context,
                    status,
                    if rate_limited { " [rate limited]" } else { "" },
                    format_payload_snippet(&payload)
                ));
            }
            Err(error) => {
                if attempt < retry_attempts {
                    let attempt_number = attempt + 1;
                    let multiplier = 1_u32
                        .checked_shl(u32::try_from(attempt).unwrap_or(0))
                        .unwrap_or(u32::MAX);
                    let backoff = base_backoff.saturating_mul(multiplier);
                    log::warn!(
                        "TheTVDB request retry scheduled for {} after transport error: {} (attempt {}/{} in {} ms)",
                        context,
                        error,
                        attempt_number,
                        retry_attempts + 1,
                        backoff.as_millis()
                    );
                    tokio::time::sleep(backoff).await;
                    continue;
                }

                return Err(format!("TheTVDB {} request failed: {}", context, error));
            }
        }
    }

    Err(format!("TheTVDB {} request failed after retries", context))
}

async fn get_json(
    provider: &MetadataProviderSettings,
    path: &str,
    query: Vec<(&'static str, String)>,
    context: &str,
) -> Result<Value, String> {
    let payload = get_text(provider, path, query, context).await?;
    serde_json::from_str::<Value>(&payload)
        .map_err(|error| format!("TheTVDB {} returned invalid JSON: {}", context, error))
}

fn parse_retry_after_seconds(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok()
}

async fn fetch_movie_payload(
    settings: &MetadataSettings,
    external_id: &str,
) -> Result<Value, String> {
    let provider = provider_settings(settings, MetadataProviderId::Tvdb)
        .map_err(|error| format!("TheTVDB {}", error))?;
    let movie_id = parse_tvdb_external_id(external_id, "movie")?;
    get_json(
        &provider,
        &format!("movies/{movie_id}/extended"),
        vec![("meta", "translations".to_string())],
        &format!("movie details lookup for {external_id}"),
    )
    .await
}

async fn fetch_series_payload(
    settings: &MetadataSettings,
    external_id: &str,
) -> Result<Value, String> {
    let provider = provider_settings(settings, MetadataProviderId::Tvdb)
        .map_err(|error| format!("TheTVDB {}", error))?;
    let series_id = parse_tvdb_external_id(external_id, "series")?;
    get_json(
        &provider,
        &format!("series/{series_id}/extended"),
        vec![("meta", "translations".to_string())],
        &format!("series details lookup for {external_id}"),
    )
    .await
}

async fn fetch_translation_payload(
    provider: &MetadataProviderSettings,
    record_path: &str,
    external_id: &str,
    provider_language: &str,
) -> Option<Value> {
    let id = parse_tvdb_external_id(external_id, record_path).ok()?;
    match get_json(
        provider,
        &format!(
            "{}/{}/translations/{}",
            record_path.trim_matches('/'),
            id,
            provider_language
        ),
        Vec::new(),
        &format!("{record_path} translation lookup for {external_id} [{provider_language}]"),
    )
    .await
    {
        Ok(payload) => payload.get("data").cloned().or(Some(payload)),
        Err(error) => {
            log::debug!("Skipping TheTVDB translation payload: {}", error);
            None
        }
    }
}

fn search_result_from_value(item: Value) -> Option<MetadataSearchResult> {
    let item_type = item
        .get("type")
        .and_then(Value::as_str)
        .map(|value| value.to_ascii_lowercase())?;
    let media_type = match item_type.as_str() {
        "series" | "tv series" | "tv" | "show" => "series",
        "movie" | "film" | "feature film" => "movie",
        _ => return None,
    };

    let external_id = object_id(&item)?.to_string();
    let title = best_title(&item, "eng")?;
    Some(MetadataSearchResult {
        provider_id: MetadataProviderId::Tvdb,
        external_id,
        media_type: media_type.into(),
        title,
        overview: best_overview(&item, "eng"),
        artwork_url: artwork_url(&item),
        backdrop_url: backdrop_url(&item),
        release_year: release_year(&item),
        score: None,
    })
}

fn movie_snapshot_from_value(
    external_id: &str,
    payload: &Value,
    translation: Option<&Value>,
    provider_language: &str,
) -> StoredMetadataSnapshot {
    let enriched_payload = payload_with_translation(payload, translation);
    let data = enriched_payload.get("data").unwrap_or(&enriched_payload);
    StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tvdb,
        external_id: external_id.to_string(),
        media_type: Some("movie".into()),
        title: best_title(data, provider_language),
        overview: best_overview(data, provider_language),
        artwork_url: artwork_url(data),
        backdrop_url: backdrop_url(data),
        release_year: release_year(data),
        locale_key: crate::metadata::DEFAULT_METADATA_LOCALE.to_string(),
        provider_locale_key: None,
        provider_payload_json: Some(enriched_payload.to_string()),
    }
}

fn series_snapshot_from_value(
    external_id: &str,
    payload: &Value,
    translation: Option<&Value>,
    provider_language: &str,
) -> StoredMetadataSnapshot {
    let enriched_payload = payload_with_translation(payload, translation);
    let data = enriched_payload.get("data").unwrap_or(&enriched_payload);
    StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tvdb,
        external_id: external_id.to_string(),
        media_type: Some("series".into()),
        title: best_title(data, provider_language),
        overview: best_overview(data, provider_language),
        artwork_url: artwork_url(data),
        backdrop_url: backdrop_url(data),
        release_year: release_year(data),
        locale_key: crate::metadata::DEFAULT_METADATA_LOCALE.to_string(),
        provider_locale_key: None,
        provider_payload_json: Some(enriched_payload.to_string()),
    }
}

fn season_snapshot_from_value(
    show_external_id: &str,
    season_number: i32,
    season_external_id: &str,
    payload: &Value,
    translation: Option<&Value>,
    provider_language: &str,
) -> StoredMetadataSnapshot {
    let enriched_payload = payload_with_translation(payload, translation);
    let data = enriched_payload.get("data").unwrap_or(&enriched_payload);
    StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tvdb,
        external_id: format!("series:{show_external_id}:season:{season_external_id}"),
        media_type: Some("season".into()),
        title: best_title(data, provider_language)
            .or_else(|| Some(format!("Season {}", season_number))),
        overview: best_overview(data, provider_language),
        artwork_url: artwork_url(data),
        backdrop_url: backdrop_url(data),
        release_year: release_year(data),
        locale_key: crate::metadata::DEFAULT_METADATA_LOCALE.to_string(),
        provider_locale_key: None,
        provider_payload_json: Some(enriched_payload.to_string()),
    }
}

fn episode_snapshot_from_value(
    show_external_id: &str,
    season_number: i32,
    _episode_number: i32,
    episode_external_id: &str,
    payload: &Value,
    translation: Option<&Value>,
    provider_language: &str,
) -> StoredMetadataSnapshot {
    let enriched_payload = payload_with_translation(payload, translation);
    let data = enriched_payload.get("data").unwrap_or(&enriched_payload);
    StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tvdb,
        external_id: format!(
            "series:{show_external_id}:season:{season_number}:episode:{episode_external_id}"
        ),
        media_type: Some("episode".into()),
        title: best_title(data, provider_language),
        overview: best_overview(data, provider_language),
        artwork_url: still_url(data).or_else(|| artwork_url(data)),
        backdrop_url: backdrop_url(data),
        release_year: release_year(data),
        locale_key: crate::metadata::DEFAULT_METADATA_LOCALE.to_string(),
        provider_locale_key: None,
        provider_payload_json: Some(enriched_payload.to_string()),
    }
}

fn payload_with_translation(
    payload: &Value,
    translation: Option<&Value>,
) -> Value {
    let Some(translation) = translation else {
        return payload.clone();
    };
    let mut payload = payload.clone();
    if let Some(data) = payload.get_mut("data").and_then(Value::as_object_mut) {
        data.insert("koko_translation".into(), translation.clone());
    } else if let Some(map) = payload.as_object_mut() {
        map.insert("koko_translation".into(), translation.clone());
    }
    payload
}

fn object_id(value: &Value) -> Option<i32> {
    value
        .get("id")
        .and_then(Value::as_i64)
        .and_then(|id| i32::try_from(id).ok())
        .or_else(|| {
            value
                .get("id")
                .and_then(Value::as_str)
                .and_then(|id| id.parse::<i32>().ok())
        })
        .or_else(|| {
            value
                .get("tvdb_id")
                .and_then(Value::as_str)
                .and_then(|id| id.parse::<i32>().ok())
        })
        .or_else(|| {
            value
                .get("objectID")
                .and_then(Value::as_str)
                .and_then(|id| id.parse::<i32>().ok())
        })
}

fn best_title(
    value: &Value,
    provider_language: &str,
) -> Option<String> {
    let preferred = tvdb_language_preference(provider_language);
    value
        .get("koko_translation")
        .and_then(|translation| text_field(translation, &["name", "title"]))
        .or_else(|| {
            value
                .get("translations")
                .and_then(|translations| translation_record(translations, &preferred))
                .and_then(|translation| text_field(translation, &["name", "title"]))
        })
        .or_else(|| {
            value
                .get("name")
                .or_else(|| value.get("title"))
                .or_else(|| value.get("name_translated"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|title| !title.is_empty())
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            value
                .get("translations")
                .and_then(Value::as_object)
                .and_then(|translations| {
                    preferred
                        .iter()
                        .find_map(|key| translations.get(key))
                        .or_else(|| translations.values().next())
                })
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|title| !title.is_empty())
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            value
                .get("translations")
                .and_then(Value::as_array)
                .and_then(|translations| {
                    translations
                        .iter()
                        .find(|translation| {
                            translation
                                .get("language")
                                .or_else(|| translation.get("languageCode"))
                                .and_then(Value::as_str)
                                .map(|language| {
                                    matches!(
                                        language.to_ascii_lowercase().as_str(),
                                        "eng" | "en" | "english"
                                    )
                                })
                                .unwrap_or(false)
                        })
                        .or_else(|| translations.first())
                })
                .and_then(|translation| {
                    translation.get("name").or_else(|| translation.get("title"))
                })
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|title| !title.is_empty())
                .map(ToOwned::to_owned)
        })
}

fn best_overview(
    value: &Value,
    provider_language: &str,
) -> Option<String> {
    let preferred = tvdb_language_preference(provider_language);
    value
        .get("koko_translation")
        .and_then(|translation| text_field(translation, &["overview", "description"]))
        .or_else(|| {
            value
                .get("translations")
                .and_then(|translations| translation_record(translations, &preferred))
                .and_then(|translation| text_field(translation, &["overview", "description"]))
        })
        .or_else(|| {
            text_field(
                value,
                &[
                    "overview",
                    "description",
                    "shortDescription",
                    "longDescription",
                ],
            )
        })
        .or_else(|| translated_overview(value.get("overviews"), &preferred))
        .or_else(|| translated_overview(value.get("overviewTranslations"), &preferred))
        .or_else(|| translated_overview(value.get("translations"), &preferred))
}

fn translation_record<'a>(
    value: &'a Value,
    preferred_keys: &[String],
) -> Option<&'a Value> {
    if let Some(map) = value.as_object() {
        return preferred_keys
            .iter()
            .find_map(|key| map.get(key))
            .or_else(|| map.values().next());
    }

    value.as_array().and_then(|translations| {
        preferred_keys
            .iter()
            .find_map(|key| {
                translations.iter().find(|translation| {
                    translation
                        .get("language")
                        .or_else(|| translation.get("languageCode"))
                        .or_else(|| translation.get("iso_639_1"))
                        .and_then(Value::as_str)
                        .map(|language| language.eq_ignore_ascii_case(key))
                        .unwrap_or(false)
                })
            })
            .or_else(|| translations.first())
    })
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
            .filter(|overview| !overview.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn translated_overview(
    value: Option<&Value>,
    preferred_keys: &[String],
) -> Option<String> {
    let value = value?;
    if let Some(map) = value.as_object() {
        return preferred_keys
            .iter()
            .find_map(|key| map.get(key).and_then(translation_overview_value))
            .or_else(|| map.values().find_map(translation_overview_value));
    }

    value.as_array().and_then(|translations| {
        preferred_keys
            .iter()
            .find_map(|key| {
                translations.iter().find_map(|translation| {
                    let language = translation
                        .get("language")
                        .or_else(|| translation.get("languageCode"))
                        .or_else(|| translation.get("iso_639_1"))
                        .and_then(Value::as_str)?;
                    language
                        .eq_ignore_ascii_case(key.as_str())
                        .then(|| translation_overview_value(translation))
                        .flatten()
                })
            })
            .or_else(|| translations.iter().find_map(translation_overview_value))
    })
}

fn tvdb_language_preference(provider_language: &str) -> Vec<String> {
    let normalized = provider_language.trim().to_ascii_lowercase();
    let mut languages = Vec::new();
    if !normalized.is_empty() {
        languages.push(normalized.clone());
    }
    match normalized.as_str() {
        "eng" | "en" | "english" => {}
        "spa" => languages.push("es".into()),
        "fra" => languages.push("fr".into()),
        "deu" => languages.push("de".into()),
        "ita" => languages.push("it".into()),
        "jpn" => languages.push("ja".into()),
        "por" => languages.push("pt".into()),
        _ => {}
    }
    for language in ["eng", "en", "english"] {
        if !languages.iter().any(|entry| entry == language) {
            languages.push(language.to_string());
        }
    }
    languages
}

fn translation_overview_value(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|overview| !overview.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| text_field(value, &["overview", "description"]))
}

fn artwork_url(value: &Value) -> Option<String> {
    tvdb_artwork_url(value, &[14, 2, 7]).or_else(|| {
        value
            .get("image")
            .or_else(|| value.get("image_url"))
            .or_else(|| value.get("poster"))
            .or_else(|| value.get("thumbnail"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn backdrop_url(value: &Value) -> Option<String> {
    tvdb_artwork_url(value, &[15, 3, 8]).or_else(|| {
        value
            .get("artworks")
            .and_then(Value::as_array)
            .and_then(|artworks| {
                artworks.iter().find_map(|artwork| {
                    artwork
                        .get("image")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|url| !url.is_empty())
                        .map(ToOwned::to_owned)
                })
            })
    })
}

fn tvdb_artwork_url(
    value: &Value,
    preferred_types: &[i64],
) -> Option<String> {
    let artworks = value
        .get("artworks")
        .or_else(|| value.get("artwork"))
        .and_then(Value::as_array)?;
    preferred_types.iter().find_map(|preferred_type| {
        artworks
            .iter()
            .filter(|artwork| artwork.get("type").and_then(Value::as_i64) == Some(*preferred_type))
            .max_by(|left, right| {
                let left_score = left.get("score").and_then(Value::as_f64).unwrap_or(0.0);
                let right_score = right.get("score").and_then(Value::as_f64).unwrap_or(0.0);
                left_score
                    .partial_cmp(&right_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .and_then(|artwork| {
                artwork
                    .get("image")
                    .or_else(|| artwork.get("thumbnail"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|url| !url.is_empty())
                    .map(ToOwned::to_owned)
            })
    })
}

fn still_url(value: &Value) -> Option<String> {
    value
        .get("image")
        .or_else(|| value.get("image_url"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(ToOwned::to_owned)
}

fn release_year(value: &Value) -> Option<i32> {
    value
        .get("year")
        .and_then(Value::as_i64)
        .and_then(|year| i32::try_from(year).ok())
        .or_else(|| {
            value
                .get("year")
                .and_then(Value::as_str)
                .and_then(|year| year.parse::<i32>().ok())
        })
        .or_else(|| {
            value
                .get("firstAired")
                .or_else(|| value.get("releaseDate"))
                .or_else(|| value.get("first_release"))
                .and_then(Value::as_str)
                .map(|value| value.to_string())
                .and_then(|value| extract_release_year(Some(value)))
        })
}

fn season_number(value: &Value) -> Option<i32> {
    value
        .get("number")
        .or_else(|| value.get("seasonNumber"))
        .and_then(Value::as_i64)
        .and_then(|number| i32::try_from(number).ok())
}

fn episode_number(value: &Value) -> Option<i32> {
    value
        .get("number")
        .or_else(|| value.get("episodeNumber"))
        .and_then(Value::as_i64)
        .and_then(|number| i32::try_from(number).ok())
}

#[cfg(test)]
mod tests {
    use super::{
        artwork_url, backdrop_url, best_overview, movie_snapshot_from_value,
        search_result_from_value,
    };
    use serde_json::json;

    #[test]
    fn tvdb_search_result_accepts_object_id_and_translations() {
        let result = search_result_from_value(json!({
            "type": "movie",
            "objectID": "901",
            "translations": {
                "eng": "Top Gun: Maverick"
            },
            "overviews": {
                "eng": "After more than thirty years of service..."
            },
            "year": "2022",
            "image_url": "https://example.test/poster.jpg"
        }))
        .expect("expected TVDB search result to parse");

        assert_eq!(result.external_id, "901");
        assert_eq!(result.media_type, "movie");
        assert_eq!(result.title, "Top Gun: Maverick");
        assert_eq!(result.release_year, Some(2022));
        assert_eq!(
            result.overview.as_deref(),
            Some("After more than thirty years of service...")
        );
    }

    #[test]
    fn tvdb_search_result_accepts_show_alias_type() {
        let result = search_result_from_value(json!({
            "type": "show",
            "tvdb_id": "42",
            "name": "Example Show"
        }))
        .expect("expected TVDB show search result to parse");

        assert_eq!(result.external_id, "42");
        assert_eq!(result.media_type, "series");
        assert_eq!(result.title, "Example Show");
    }

    #[test]
    fn tvdb_movie_snapshot_prefers_translation_payload_for_overview_and_tagline() {
        let payload = json!({
            "data": {
                "id": 901,
                "name": "Provider Name",
                "overviewTranslations": ["eng", "rus"],
                "translations": {
                    "rus": "rus"
                },
                "artworks": [
                    { "type": 14, "image": "https://example.test/poster.jpg", "score": 1.0 },
                    { "type": 15, "image": "https://example.test/backdrop.jpg", "score": 1.0 },
                    { "type": 25, "image": "https://example.test/logo.png", "score": 1.0 }
                ],
                "genres": [{ "name": "Action" }]
            }
        });
        let translation = json!({
            "language": "eng",
            "name": "Translated Name",
            "overview": "Translated overview.",
            "tagline": "Translated tagline."
        });

        let snapshot = movie_snapshot_from_value("901", &payload, Some(&translation), "eng");
        assert_eq!(snapshot.title.as_deref(), Some("Translated Name"));
        assert_eq!(snapshot.overview.as_deref(), Some("Translated overview."));
        assert_eq!(
            snapshot.artwork_url.as_deref(),
            Some("https://example.test/poster.jpg")
        );
        assert_eq!(
            snapshot.backdrop_url.as_deref(),
            Some("https://example.test/backdrop.jpg")
        );
        assert!(
            snapshot
                .provider_payload_json
                .as_deref()
                .is_some_and(|payload| payload.contains("Translated tagline."))
        );
    }

    #[test]
    fn tvdb_overview_does_not_use_language_code_names() {
        let payload = json!({
            "translations": [
                { "language": "rus", "name": "rus" }
            ]
        });

        assert_eq!(best_overview(&payload, "eng"), None);
    }

    #[test]
    fn tvdb_artwork_uses_documented_type_ids() {
        let payload = json!({
            "artworks": [
                { "type": 25, "image": "https://example.test/logo.png", "score": 9.0 },
                { "type": 14, "image": "https://example.test/poster.jpg", "score": 4.0 },
                { "type": 15, "image": "https://example.test/backdrop.jpg", "score": 6.0 }
            ]
        });

        assert_eq!(
            artwork_url(&payload).as_deref(),
            Some("https://example.test/poster.jpg")
        );
        assert_eq!(
            backdrop_url(&payload).as_deref(),
            Some("https://example.test/backdrop.jpg")
        );
    }
}
