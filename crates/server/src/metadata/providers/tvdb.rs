use serde_json::Value;
use strsim::normalized_levenshtein;

use crate::config::{MetadataProviderId, MetadataProviderSettings, MetadataSettings};
use crate::metadata::{
    HTTP_CLIENT, MediaLibraryKind, MetadataProviderDescriptor, MetadataSearchResult,
    StoredMetadataSnapshot, TVDB_API_BASE, TVDB_AUTH_TOKEN, TVDB_RATE_LIMITER, TvdbCachedToken,
    TvdbDescendantTarget, cleanup_movie_title, extract_release_year, format_payload_snippet,
    movie_match_score, parse_movie_name, parse_retry_after_seconds, provider_settings,
    show_search_query,
};
use reqwest::StatusCode;
use reqwest::header::RETRY_AFTER;
use std::time::{Duration, Instant};

pub(crate) fn descriptor() -> MetadataProviderDescriptor {
    MetadataProviderDescriptor {
        id: MetadataProviderId::Tvdb,
        display_name: "TheTVDB".into(),
        description: "Alternative movie and television metadata provider with strong series and episode coverage.".into(),
        supported_kinds: vec![MediaLibraryKind::Movies, MediaLibraryKind::Shows],
        requires_api_key: true,
        implemented: true,
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
    let provider = provider_settings(settings, MetadataProviderId::Tvdb)
        .map_err(|error| format!("TheTVDB {}", error))?;
    match media_type {
        "movie" => {
            let payload = get_json(
                &provider,
                &format!("movies/{external_id}/extended"),
                vec![("meta", "translations".to_string())],
                &format!("movie details lookup for {external_id}"),
            )
            .await?;
            Ok(movie_snapshot_from_value(external_id, &payload))
        }
        "series" => {
            let payload = get_json(
                &provider,
                &format!("series/{external_id}/extended"),
                vec![("meta", "translations".to_string())],
                &format!("series details lookup for {external_id}"),
            )
            .await?;
            Ok(series_snapshot_from_value(external_id, &payload))
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
    let payload = get_json(
        &provider,
        &format!("seasons/{season_external_id}/extended"),
        vec![("meta", "translations".to_string())],
        &format!("season lookup for series:{show_external_id}:season:{season_external_id}"),
    )
    .await?;
    Ok(season_snapshot_from_value(
        show_external_id,
        season_number,
        season_external_id,
        &payload,
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
    let payload = get_json(
        &provider,
        &format!("episodes/{episode_external_id}/extended"),
        vec![("meta", "translations".to_string())],
        &format!(
            "episode lookup for series:{show_external_id}:season:{season_number}:episode:{episode_external_id}"
        ),
    )
    .await?;
    Ok(episode_snapshot_from_value(
        show_external_id,
        season_number,
        episode_number,
        episode_external_id,
        &payload,
    ))
}

pub(crate) async fn load_show_descendant_targets(
    settings: &MetadataSettings,
    show_external_id: &str,
) -> Result<Vec<TvdbDescendantTarget>, String> {
    let provider = provider_settings(settings, MetadataProviderId::Tvdb)
        .map_err(|error| format!("TheTVDB {}", error))?;
    let series_payload = get_json(
        &provider,
        &format!("series/{show_external_id}/extended"),
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

    let payload = serde_json::json!({ "apikey": api_key });
    let response = HTTP_CLIENT
        .post(format!("{}/login", TVDB_API_BASE))
        .json(&payload)
        .send()
        .await
        .map_err(|error| format!("TheTVDB login request failed: {}", error))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("TheTVDB login response failed: {}", error))?;
    if !status.is_success() {
        return Err(format!(
            "TheTVDB login failed with status {}{}",
            status,
            format_payload_snippet(&body)
        ));
    }

    let parsed: Value = serde_json::from_str(&body)
        .map_err(|error| format!("TheTVDB login returned invalid JSON: {}", error))?;
    let token = parsed
        .get("data")
        .and_then(|value| value.get("token"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "TheTVDB login response did not include a token.".to_string())?
        .to_string();

    let cached = TvdbCachedToken {
        token: token.clone(),
        expires_at: Instant::now() + Duration::from_secs(60 * 60 * 24 * 25),
    };
    let mut cache = TVDB_AUTH_TOKEN.lock().await;
    *cache = Some(cached);
    Ok(token)
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
        let request_url = format!("{}/{}", TVDB_API_BASE, path.trim_start_matches('/'));
        let response = HTTP_CLIENT
            .get(&request_url)
            .bearer_auth(&token)
            .query(&query)
            .send()
            .await;

        match response {
            Ok(response) => {
                let status = response.status();
                let retry_after = response
                    .headers()
                    .get(RETRY_AFTER)
                    .and_then(|value| value.to_str().ok())
                    .and_then(parse_retry_after_seconds)
                    .map(Duration::from_secs);
                let payload = response.text().await.map_err(|error| error.to_string())?;
                if status.is_success() {
                    return Ok(payload);
                }

                if status == StatusCode::UNAUTHORIZED {
                    let mut cache = TVDB_AUTH_TOKEN.lock().await;
                    *cache = None;
                }

                let rate_limited = status == StatusCode::TOO_MANY_REQUESTS
                    || retry_after.is_some()
                    || payload.to_ascii_lowercase().contains("rate limit");
                let payload_snippet = format_payload_snippet(&payload);
                let retryable =
                    status == StatusCode::UNAUTHORIZED || rate_limited || status.is_server_error();
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
                        payload_snippet,
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
                    payload_snippet
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

fn search_result_from_value(item: Value) -> Option<MetadataSearchResult> {
    let item_type = item
        .get("type")
        .and_then(Value::as_str)
        .map(|value| value.to_ascii_lowercase())?;
    let media_type = match item_type.as_str() {
        "series" | "tv series" => "series",
        "movie" => "movie",
        _ => return None,
    };

    let external_id = object_id(&item)?.to_string();
    let title = best_title(&item)?;
    Some(MetadataSearchResult {
        provider_id: MetadataProviderId::Tvdb,
        external_id,
        media_type: media_type.into(),
        title,
        overview: best_overview(&item),
        artwork_url: artwork_url(&item),
        backdrop_url: backdrop_url(&item),
        release_year: release_year(&item),
    })
}

fn movie_snapshot_from_value(
    external_id: &str,
    payload: &Value,
) -> StoredMetadataSnapshot {
    let data = payload.get("data").unwrap_or(payload);
    StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tvdb,
        external_id: external_id.to_string(),
        media_type: Some("movie".into()),
        title: best_title(data),
        overview: best_overview(data),
        artwork_url: artwork_url(data),
        backdrop_url: backdrop_url(data),
        release_year: release_year(data),
        provider_payload_json: Some(payload.to_string()),
    }
}

fn series_snapshot_from_value(
    external_id: &str,
    payload: &Value,
) -> StoredMetadataSnapshot {
    let data = payload.get("data").unwrap_or(payload);
    StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tvdb,
        external_id: external_id.to_string(),
        media_type: Some("series".into()),
        title: best_title(data),
        overview: best_overview(data),
        artwork_url: artwork_url(data),
        backdrop_url: backdrop_url(data),
        release_year: release_year(data),
        provider_payload_json: Some(payload.to_string()),
    }
}

fn season_snapshot_from_value(
    show_external_id: &str,
    season_number: i32,
    season_external_id: &str,
    payload: &Value,
) -> StoredMetadataSnapshot {
    let data = payload.get("data").unwrap_or(payload);
    StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tvdb,
        external_id: format!("series:{show_external_id}:season:{season_external_id}"),
        media_type: Some("season".into()),
        title: best_title(data).or_else(|| Some(format!("Season {}", season_number))),
        overview: best_overview(data),
        artwork_url: artwork_url(data),
        backdrop_url: backdrop_url(data),
        release_year: release_year(data),
        provider_payload_json: Some(payload.to_string()),
    }
}

fn episode_snapshot_from_value(
    show_external_id: &str,
    season_number: i32,
    _episode_number: i32,
    episode_external_id: &str,
    payload: &Value,
) -> StoredMetadataSnapshot {
    let data = payload.get("data").unwrap_or(payload);
    StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tvdb,
        external_id: format!(
            "series:{show_external_id}:season:{season_number}:episode:{episode_external_id}"
        ),
        media_type: Some("episode".into()),
        title: best_title(data),
        overview: best_overview(data),
        artwork_url: still_url(data).or_else(|| artwork_url(data)),
        backdrop_url: backdrop_url(data),
        release_year: release_year(data),
        provider_payload_json: Some(payload.to_string()),
    }
}

fn object_id(value: &Value) -> Option<i32> {
    value
        .get("id")
        .and_then(Value::as_i64)
        .and_then(|id| i32::try_from(id).ok())
}

fn best_title(value: &Value) -> Option<String> {
    value
        .get("name")
        .or_else(|| value.get("title"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            value
                .get("translations")
                .and_then(Value::as_object)
                .and_then(|translations| {
                    translations.values().find_map(|entries| {
                        entries.as_array().and_then(|entries| {
                            entries.iter().find_map(|entry| {
                                entry
                                    .get("name")
                                    .or_else(|| entry.get("title"))
                                    .and_then(Value::as_str)
                                    .map(str::trim)
                                    .filter(|title| !title.is_empty())
                                    .map(ToOwned::to_owned)
                            })
                        })
                    })
                })
        })
}

fn best_overview(value: &Value) -> Option<String> {
    value
        .get("overview")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|overview| !overview.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            value
                .get("translations")
                .and_then(|translations| translations.get("overviewTranslations"))
                .and_then(Value::as_array)
                .and_then(|entries| {
                    entries.iter().find_map(|entry| {
                        entry
                            .get("overview")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|overview| !overview.is_empty())
                            .map(ToOwned::to_owned)
                    })
                })
        })
}

fn artwork_url(value: &Value) -> Option<String> {
    value
        .get("image")
        .or_else(|| value.get("image_url"))
        .or_else(|| value.get("artwork"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(ToOwned::to_owned)
}

fn backdrop_url(value: &Value) -> Option<String> {
    value
        .get("artworks")
        .and_then(Value::as_array)
        .and_then(|artworks| {
            artworks.iter().find_map(|artwork| {
                let kind = artwork
                    .get("type")
                    .or_else(|| artwork.get("typeName"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if kind.contains("background") || kind.contains("fanart") || kind.contains("banner")
                {
                    artwork
                        .get("image")
                        .or_else(|| artwork.get("image_url"))
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned)
                } else {
                    None
                }
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
                .get("firstAired")
                .or_else(|| value.get("releaseDate"))
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
