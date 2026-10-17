mod response;

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
    ProviderMetadataDetails,
    ProviderMetadataPerson,
    StoredMetadataSnapshot,
    cleanup_movie_title,
    format_payload_snippet,
    managed_metadata_asset_dir,
    metadata_asset_db_path,
    metadata_response_cache_key,
    movie_match_score,
    parse_movie_name,
    provider_settings,
    read_metadata_response_cache_text,
    show_search_query,
    try_cache_item_artwork,
    write_metadata_response_cache_text,
};
use response::{
    EpisodeExtendedRecord,
    MovieExtendedRecord,
    Person,
    Record,
    Response,
    SeasonExtendedRecord,
    SeriesExtendedRecord,
};
use serde::de::DeserializeOwned;
use std::time::{
    Duration,
    Instant,
};
use strsim::normalized_levenshtein;
use tvdb4::models::{
    Gender,
    LoginPostRequest,
    PeopleExtendedRecord,
    SearchResult,
    Translation,
};
const TVDB_API_BASE: &str = "https://api4.thetvdb.com/v4";
/// Fixed request policy for TheTVDB, independent of saved provider settings.
const TVDB_REQUESTS_PER_SECOND: u32 = 4;
/// Retry transient failures up to three times after the initial request.
const TVDB_RETRY_ATTEMPTS: usize = 3;
/// Initial retry delay before applying exponential backoff or Retry-After.
const TVDB_RETRY_BACKOFF: Duration = Duration::from_millis(1_000);

static TVDB_RATE_LIMITER: once_cell::sync::Lazy<tokio::sync::Mutex<Instant>> =
    once_cell::sync::Lazy::new(|| tokio::sync::Mutex::new(Instant::now()));
static TVDB_AUTH_TOKEN: once_cell::sync::Lazy<tokio::sync::Mutex<Option<TvdbCachedToken>>> =
    once_cell::sync::Lazy::new(|| tokio::sync::Mutex::new(None));

#[derive(Debug, Clone)]
struct TvdbCachedToken {
    token: String,
    credential_fingerprint: Vec<u8>,
    expires_at: Instant,
}

impl TvdbCachedToken {
    fn matches(
        &self,
        fingerprint: &[u8],
        now: Instant,
    ) -> bool {
        self.expires_at > now && self.credential_fingerprint == fingerprint
    }
}

pub(crate) fn descriptor() -> MetadataProviderDescriptor {
    MetadataProviderDescriptor {
        id: MetadataProviderId::Tvdb,
        display_name: "TheTVDB".into(),
        description: "Alternative movie and television metadata provider with strong series and \
                      episode coverage."
            .into(),
        supported_kinds: vec![
            MediaLibraryKind::Movies,
            MediaLibraryKind::Shows,
        ],
        requires_api_key: true,
        implemented: true,
        role: MetadataProviderRole::Primary,
        extends_provider_ids: Vec::new(),
        attribution_text: "Metadata and artwork provided by TheTVDB.".into(),
        attribution_url: "https://thetvdb.com/".into(),
        logo_light_url: Some("/assets/providers/tvdb-light.png".into()),
        logo_dark_url: Some("/assets/providers/tvdb-dark.png".into()),
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
        "series" => MetadataItemKind::Show,
        "season" => MetadataItemKind::Season,
        "episode" => MetadataItemKind::Episode,
        "list" => MetadataItemKind::Collection,
        "people" => MetadataItemKind::Person,
        "company" => MetadataItemKind::Company,
        "award" => MetadataItemKind::Award,
        _ => MetadataItemKind::Item,
    }
}

pub(crate) async fn search(
    settings: &MetadataSettings,
    query: &str,
    media_type: Option<&str>,
) -> Result<Vec<MetadataSearchResult>, String> {
    let provider = provider_settings(settings, MetadataProviderId::Tvdb)?;
    search_with_fallback(query, |query| search_query(&provider, query, media_type)).await
}

async fn search_with_fallback<F, Fut>(
    query: &str,
    mut search: F,
) -> Result<Vec<MetadataSearchResult>, String>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<MetadataSearchResult>, String>>,
{
    let results = search(query.to_string()).await?;
    if !results.is_empty()
        || !query
            .split_whitespace()
            .any(|word| word.eq_ignore_ascii_case("the"))
    {
        return Ok(results);
    }

    // TVDB can return no hits for an official title containing "the", while
    // finding that same record when these standalone articles are omitted.
    // Keep the original query authoritative and make at most one extra request.
    let fallback = query
        .split_whitespace()
        .filter(|word| !word.eq_ignore_ascii_case("the"))
        .collect::<Vec<_>>()
        .join(" ");
    if fallback.is_empty() {
        return Ok(results);
    }
    search(fallback).await
}

async fn search_query(
    provider: &MetadataProviderSettings,
    query: String,
    media_type: Option<&str>,
) -> Result<Vec<MetadataSearchResult>, String> {
    let mut params = vec![
        ("query", query),
        ("limit", "20".to_string()),
    ];
    if let Some(kind) = media_type {
        params.push(("type", kind.to_string()));
    }
    let response: Response<Vec<SearchResult>> =
        get_json(provider, "search", params, "search").await?;
    Ok(response
        .data
        .into_iter()
        .filter_map(|item| response::search_result(item, &provider.language))
        .filter(|item| media_type.is_none_or(|kind| item.media_type == kind))
        .collect())
}

async fn fetch_record<T: DeserializeOwned + Record>(
    settings: &MetadataSettings,
    path: &str,
    translation_path: &str,
    include_people: bool,
) -> Result<Response<T>, String> {
    let provider = provider_settings(settings, MetadataProviderId::Tvdb)?;
    let mut response: Response<T> =
        get_json(&provider, path, vec![("meta", "translations".into())], path).await?;
    response.translation = fetch_translation_payload(&provider, translation_path).await;
    if include_people {
        let mut ids = std::collections::HashSet::new();
        let ids = response
            .data
            .characters()
            .iter()
            .filter_map(|character| character.people_id)
            .filter(|id| ids.insert(*id))
            .take(80)
            .collect::<Vec<_>>();
        for id in ids {
            if let Ok(person) = fetch_person(&provider, i64::from(id)).await {
                response.people.insert(i64::from(id), person);
            }
        }
    }
    Ok(response)
}

pub(crate) async fn fetch_snapshot(
    settings: &MetadataSettings,
    external_id: &str,
    media_type: &str,
    include_person_details: bool,
) -> Result<StoredMetadataSnapshot, String> {
    let id = parse_tvdb_external_id(external_id, media_type)?;
    let language = provider_settings(settings, MetadataProviderId::Tvdb)?.language;
    match media_type {
        "movie" => {
            let record = fetch_record::<MovieExtendedRecord>(
                settings,
                &format!("movies/{id}/extended"),
                &format!("movies/{id}/translations/{language}"),
                include_person_details,
            )
            .await?;
            Ok(response::snapshot(
                &record,
                external_id,
                media_type,
                &language,
            ))
        }
        "series" => {
            let record = fetch_record::<SeriesExtendedRecord>(
                settings,
                &format!("series/{id}/extended"),
                &format!("series/{id}/translations/{language}"),
                include_person_details,
            )
            .await?;
            Ok(response::snapshot(
                &record,
                external_id,
                media_type,
                &language,
            ))
        }
        other => Err(format!("Unsupported TheTVDB media type: {other}")),
    }
}

pub(crate) async fn fetch_season_snapshot(
    settings: &MetadataSettings,
    show_external_id: &str,
    season_number: i32,
    season_external_id: &str,
    include_person_details: bool,
) -> Result<StoredMetadataSnapshot, String> {
    let id = parse_tvdb_external_id(season_external_id, "season")?;
    let language = provider_settings(settings, MetadataProviderId::Tvdb)?.language;
    let record = fetch_record::<SeasonExtendedRecord>(
        settings,
        &format!("seasons/{id}/extended"),
        &format!("seasons/{id}/translations/{language}"),
        include_person_details,
    )
    .await?;
    let mut snapshot = response::snapshot(
        &record,
        &format!("series:{show_external_id}:season:{season_external_id}"),
        "season",
        &language,
    );
    if snapshot.title.is_none() {
        snapshot.title = Some(format!("Season {season_number}"));
    }
    Ok(snapshot)
}

pub(crate) async fn fetch_episode_snapshot(
    settings: &MetadataSettings,
    show_external_id: &str,
    season_number: i32,
    _episode_number: i32,
    episode_external_id: &str,
    include_person_details: bool,
) -> Result<StoredMetadataSnapshot, String> {
    let id = parse_tvdb_external_id(episode_external_id, "episode")?;
    let language = provider_settings(settings, MetadataProviderId::Tvdb)?.language;
    let record = fetch_record::<EpisodeExtendedRecord>(
        settings,
        &format!("episodes/{id}/extended"),
        &format!("episodes/{id}/translations/{language}"),
        include_person_details,
    )
    .await?;
    Ok(response::snapshot(
        &record,
        &format!("series:{show_external_id}:season:{season_number}:episode:{episode_external_id}"),
        "episode",
        &language,
    ))
}

pub(crate) async fn load_show_descendant_targets(
    settings: &MetadataSettings,
    show_external_id: &str,
) -> Result<Vec<ProviderDescendantTarget>, String> {
    let provider = provider_settings(settings, MetadataProviderId::Tvdb)?;
    let id = parse_tvdb_external_id(show_external_id, "series")?;
    let series: Response<SeriesExtendedRecord> = get_json(
        &provider,
        &format!("series/{id}/extended"),
        Vec::new(),
        "series descendant lookup",
    )
    .await?;
    let mut targets = Vec::new();
    for season in default_order_seasons(&series.data)? {
        let (Some(id), Some(number)) = (
            season.id,
            season.number.and_then(|number| i32::try_from(number).ok()),
        ) else {
            continue;
        };
        let season: Response<response::SeasonEpisodes> = get_json(
            &provider,
            &format!("seasons/{id}/extended"),
            Vec::new(),
            "season descendant lookup",
        )
        .await?;
        for episode in season.data.episodes.unwrap_or_default() {
            let (Some(episode_id), Some(episode_number)) = (episode.id, episode.number) else {
                continue;
            };
            targets.push(ProviderDescendantTarget {
                season_number: number,
                episode_number,
                season_external_id: id.to_string(),
                episode_external_id: episode_id.to_string(),
            });
        }
    }
    Ok(targets)
}

fn default_order_seasons(
    series: &SeriesExtendedRecord
) -> Result<Vec<&response::SeasonBaseRecord>, String> {
    let order = series
        .default_season_type
        .ok_or_else(|| "TheTVDB series is missing defaultSeasonType".to_string())?;
    Ok(series
        .seasons
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter(|season| season.r#type.as_deref().and_then(|kind| kind.id) == Some(order))
        .collect())
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

    if let Some(tvdb_id) = parsed.provider_id("tvdb").map(str::to_string) {
        let snapshot = fetch_snapshot(settings, &tvdb_id, "movie", false).await?;
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
    for result in search(settings, &query, Some("series")).await? {
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

fn parse_tvdb_external_id(
    external_id: &str,
    media_type: &str,
) -> Result<i64, String> {
    external_id
        .parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| {
            format!(
                "TheTVDB {media_type} external id must be a positive integer, got {external_id:?}"
            )
        })
}

async fn wait_for_rate_limit() {
    let interval = Duration::from_secs_f64(1.0 / f64::from(TVDB_REQUESTS_PER_SECOND));
    let mut next_available_at = TVDB_RATE_LIMITER.lock().await;
    let now = Instant::now();
    if *next_available_at > now {
        tokio::time::sleep((*next_available_at).saturating_duration_since(now)).await;
    }
    let base = Instant::now();
    *next_available_at = base.checked_add(interval).unwrap_or(base);
}

async fn auth_token(provider: &MetadataProviderSettings) -> Result<String, String> {
    let api_key = provider
        .api_key
        .clone()
        .unwrap_or_default()
        .trim()
        .to_string();
    if api_key.is_empty() {
        return Err("TheTVDB is enabled but no API key is configured.".into());
    }
    use sha2::Digest;
    let credential_fingerprint = sha2::Sha256::digest(api_key.as_bytes()).to_vec();
    {
        let cache = TVDB_AUTH_TOKEN.lock().await;
        if let Some(cached) = cache
            .as_ref()
            .filter(|cached| cached.matches(&credential_fingerprint, Instant::now()))
        {
            return Ok(cached.token.clone());
        }
    }

    wait_for_rate_limit().await;
    #[derive(serde::Deserialize)]
    struct Token {
        token: Option<String>,
    }
    let response = crate::metadata::http::client()
        .post(format!("{TVDB_API_BASE}/login"))
        .json(&LoginPostRequest::new(api_key))
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json::<Response<Token>>()
        .await
        .map_err(|error| error.to_string())?;
    let token = response
        .data
        .token
        .map(|token| token.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "TheTVDB login response did not include a token.".to_string())?;

    let cached = TvdbCachedToken {
        token: token.clone(),
        credential_fingerprint,
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
    let query_key = query
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    let cache_key =
        metadata_response_cache_key(&MetadataProviderId::Tvdb, "http", &[path, &query_key]);
    if let Some(payload) = read_metadata_response_cache_text(&cache_key) {
        return Ok(payload);
    }

    let retry_attempts = TVDB_RETRY_ATTEMPTS;
    let base_backoff = TVDB_RETRY_BACKOFF;

    for attempt in 0..=retry_attempts {
        wait_for_rate_limit().await;
        let token = auth_token(provider).await?;
        let request_url = format!("{}/{}", TVDB_API_BASE, path.trim_start_matches('/'));
        let request = crate::metadata::http::client()
            .get(&request_url)
            .bearer_auth(token)
            .query(&query);
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
                    write_metadata_response_cache_text(&cache_key, &payload);
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
                    let backoff = retry_after
                        .unwrap_or_else(|| base_backoff.saturating_mul(multiplier))
                        .min(Duration::from_secs(60));
                    log::warn!(
                        "TheTVDB request retry scheduled for {} after status {}{}{} (attempt \
                         {}/{} in {} ms)",
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
                        "TheTVDB request retry scheduled for {} after transport error: {} \
                         (attempt {}/{} in {} ms)",
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

async fn get_json<T: DeserializeOwned>(
    provider: &MetadataProviderSettings,
    path: &str,
    query: Vec<(&'static str, String)>,
    context: &str,
) -> Result<Response<T>, String> {
    let payload = get_text(provider, path, query, context).await?;
    serde_json::from_str(&payload)
        .map_err(|error| format!("TheTVDB {context} returned invalid response: {error}"))
}

async fn fetch_translation_payload(
    provider: &MetadataProviderSettings,
    path: &str,
) -> Option<Translation> {
    match get_json::<Translation>(provider, path, Vec::new(), "translation").await {
        Ok(response) => Some(response.data),
        Err(error) => {
            log::debug!("Skipping TheTVDB translation payload: {error}");
            None
        }
    }
}

async fn fetch_person(
    provider: &MetadataProviderSettings,
    id: i64,
) -> Result<Person, String> {
    let record: Response<PeopleExtendedRecord> = get_json(
        provider,
        &format!("people/{id}/extended"),
        vec![("meta", "translations".into())],
        "person details",
    )
    .await?;
    let gender_label = if let Some(id) = record.data.gender {
        get_json::<Vec<Gender>>(provider, "genders", Vec::new(), "gender names")
            .await
            .ok()
            .and_then(|response| {
                response
                    .data
                    .into_iter()
                    .find(|gender| gender.id == Some(i64::from(id)))
            })
            .and_then(|gender| response::text(gender.name.as_deref()))
    } else {
        None
    };
    Ok(Person {
        data: record.data,
        cached_image_path: None,
        gender_label,
    })
}

pub(crate) async fn fetch_person_metadata(
    settings: &MetadataSettings,
    external_id: &str,
) -> Result<Option<ProviderMetadataPerson>, String> {
    let provider = provider_settings(settings, MetadataProviderId::Tvdb)?;
    let id = parse_tvdb_external_id(external_id, "people")?;
    Ok(response::person_details(
        &fetch_person(&provider, id).await?,
        &provider.language,
    ))
}

pub(crate) fn metadata_details(snapshot: &StoredMetadataSnapshot) -> ProviderMetadataDetails {
    fn decode<T: Record + DeserializeOwned>(
        snapshot: &StoredMetadataSnapshot
    ) -> ProviderMetadataDetails {
        match snapshot
            .provider_payload_json
            .as_deref()
            .map(serde_json::from_str::<Response<T>>)
        {
            Some(Ok(response)) => response::details(&response, snapshot),
            Some(Err(error)) => {
                log::warn!(
                    "Invalid stored TheTVDB {} response: {error}",
                    snapshot.external_id
                );
                ProviderMetadataDetails::default()
            }
            None => ProviderMetadataDetails::default(),
        }
    }
    match snapshot.media_type.as_deref() {
        Some("movie") => decode::<MovieExtendedRecord>(snapshot),
        Some("series") => decode::<SeriesExtendedRecord>(snapshot),
        Some("season") => decode::<SeasonExtendedRecord>(snapshot),
        Some("episode") => decode::<EpisodeExtendedRecord>(snapshot),
        _ => ProviderMetadataDetails::default(),
    }
}

pub(crate) async fn cache_person_assets(
    snapshot: &StoredMetadataSnapshot,
    data_dir: &str,
) -> Result<StoredMetadataSnapshot, String> {
    async fn cache<T: Record + DeserializeOwned>(
        snapshot: &StoredMetadataSnapshot,
        data_dir: &str,
    ) -> Result<StoredMetadataSnapshot, String> {
        let Some(payload) = snapshot.provider_payload_json.as_deref() else {
            return Ok(snapshot.clone());
        };
        let mut response: Response<T> =
            serde_json::from_str(payload).map_err(|error| error.to_string())?;
        for character in response.data.characters().to_vec() {
            let Some(id) = character.people_id.map(i64::from) else {
                continue;
            };
            let person = response.people.entry(id).or_insert_with(|| Person {
                data: PeopleExtendedRecord {
                    id: Some(id),
                    name: character.person_name.clone(),
                    image: character.person_img_url.clone(),
                    ..PeopleExtendedRecord::new()
                },
                cached_image_path: None,
                gender_label: None,
            });
            let Some(url) = response::image_url(person.data.image.as_deref()) else {
                continue;
            };
            let dir = managed_metadata_asset_dir(
                data_dir,
                MetadataProviderId::Tvdb,
                &id.to_string(),
                Some("people"),
                &snapshot.locale_key,
            );
            if let Some(path) = try_cache_item_artwork(&url, &dir, "tvdb_profile").await {
                person.cached_image_path = Some(metadata_asset_db_path(data_dir, &path));
            }
        }
        let mut updated = snapshot.clone();
        updated.provider_payload_json =
            Some(serde_json::to_string(&response).map_err(|error| error.to_string())?);
        Ok(updated)
    }
    match snapshot.media_type.as_deref() {
        Some("movie") => cache::<MovieExtendedRecord>(snapshot, data_dir).await,
        Some("series") => cache::<SeriesExtendedRecord>(snapshot, data_dir).await,
        Some("season") => cache::<SeasonExtendedRecord>(snapshot, data_dir).await,
        Some("episode") => cache::<EpisodeExtendedRecord>(snapshot, data_dir).await,
        _ => Ok(snapshot.clone()),
    }
}

fn parse_retry_after_seconds(value: &str) -> Option<u64> {
    value.trim().parse().ok()
}

struct TvdbRemoteIdSourceType {
    id: i64,
    source_name: &'static str,
    source: &'static str,
}

// Source type ids come from TheTVDB's `/sources/types` contract.
const TVDB_REMOTE_ID_SOURCE_TYPES: &[TvdbRemoteIdSourceType] = &[
    TvdbRemoteIdSourceType {
        id: 2,
        source_name: "IMDB",
        source: "imdb",
    },
    TvdbRemoteIdSourceType {
        id: 3,
        source_name: "TMS (Zap2It)",
        source: "zap2it",
    },
    TvdbRemoteIdSourceType {
        id: 4,
        source_name: "Official Website",
        source: "official_website",
    },
    TvdbRemoteIdSourceType {
        id: 5,
        source_name: "Facebook",
        source: "facebook",
    },
    TvdbRemoteIdSourceType {
        id: 6,
        source_name: "X",
        source: "twitter",
    },
    TvdbRemoteIdSourceType {
        id: 7,
        source_name: "Reddit",
        source: "reddit",
    },
    TvdbRemoteIdSourceType {
        id: 8,
        source_name: "Instagram",
        source: "instagram",
    },
    TvdbRemoteIdSourceType {
        id: 9,
        source_name: "Wikipedia",
        source: "wikipedia",
    },
    TvdbRemoteIdSourceType {
        id: 10,
        source_name: "Wikidata",
        source: "wikidata",
    },
    TvdbRemoteIdSourceType {
        id: 11,
        source_name: "YouTube",
        source: "youtube",
    },
    TvdbRemoteIdSourceType {
        id: 12,
        source_name: "TheMovieDB.com",
        source: "tmdb",
    },
    TvdbRemoteIdSourceType {
        id: 13,
        source_name: "EIDR",
        source: "eidr",
    },
];

fn tvdb_requested_language_preference(provider_language: &str) -> Vec<String> {
    let normalized = provider_language.trim().to_ascii_lowercase();
    let mut languages = Vec::new();
    push_language_preference(&mut languages, &normalized);
    match normalized.as_str() {
        "eng" | "en" | "english" => {
            push_language_preference(&mut languages, "eng");
            push_language_preference(&mut languages, "en");
            push_language_preference(&mut languages, "english");
        }
        "spa" | "es" | "spanish" => {
            push_language_preference(&mut languages, "spa");
            push_language_preference(&mut languages, "es");
        }
        "fra" | "fr" | "french" => {
            push_language_preference(&mut languages, "fra");
            push_language_preference(&mut languages, "fr");
        }
        "deu" | "de" | "ger" | "german" => {
            push_language_preference(&mut languages, "deu");
            push_language_preference(&mut languages, "de");
        }
        "ita" | "it" | "italian" => {
            push_language_preference(&mut languages, "ita");
            push_language_preference(&mut languages, "it");
        }
        "jpn" | "ja" | "japanese" => {
            push_language_preference(&mut languages, "jpn");
            push_language_preference(&mut languages, "ja");
        }
        "por" | "pt" | "portuguese" => {
            push_language_preference(&mut languages, "por");
            push_language_preference(&mut languages, "pt");
        }
        _ => {}
    }
    languages
}

fn tvdb_language_preference(provider_language: &str) -> Vec<String> {
    let mut languages = tvdb_requested_language_preference(provider_language);
    for language in ["eng", "en", "english"] {
        push_language_preference(&mut languages, language);
    }
    languages
}

fn push_language_preference(
    languages: &mut Vec<String>,
    language: &str,
) {
    let language = language.trim().to_ascii_lowercase();
    if !language.is_empty() && !languages.iter().any(|entry| entry == &language) {
        languages.push(language);
    }
}

#[cfg(test)]
mod tests {
    fn falcon_search_result() -> super::MetadataSearchResult {
        super::response::search_result(
            serde_json::from_value(serde_json::json!({
                "tvdb_id": "362496", "type": "series", "year": "2021",
                "name": "The Falcon and The Winter Soldier"
            }))
            .unwrap(),
            "eng",
        )
        .unwrap()
    }

    #[tokio::test]
    async fn empty_official_title_search_retries_without_standalone_articles() {
        let mut requests = Vec::new();
        let results = super::search_with_fallback("The Falcon and The Winter Soldier", |query| {
            requests.push(query.clone());
            std::future::ready(Ok(match query.as_str() {
                "The Falcon and The Winter Soldier" => Vec::new(),
                "Falcon and Winter Soldier" => vec![falcon_search_result()],
                _ => panic!("Unexpected search query: {query}"),
            }))
        })
        .await
        .unwrap();
        assert_eq!(
            requests,
            [
                "The Falcon and The Winter Soldier",
                "Falcon and Winter Soldier"
            ]
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].external_id, "362496");
        assert_eq!(results[0].title, "The Falcon and The Winter Soldier");
        assert_eq!(results[0].release_year, Some(2021));
        assert_eq!(results[0].media_type, "series");
    }

    #[tokio::test]
    async fn successful_title_search_does_not_broaden_the_results() {
        let mut requests = Vec::new();
        let results = super::search_with_fallback("The Falcon and The Winter Soldier", |query| {
            requests.push(query);
            std::future::ready(Ok(vec![falcon_search_result()]))
        })
        .await
        .unwrap();
        assert_eq!(requests, ["The Falcon and The Winter Soldier"]);
        assert_eq!(results[0].external_id, "362496");
    }

    #[tokio::test]
    async fn empty_fallback_is_bounded_and_preserves_other_words_and_unicode() {
        let mut requests = Vec::new();
        let results = super::search_with_fallback("THE Théâtre of the Other", |query| {
            requests.push(query);
            std::future::ready(Ok(Vec::new()))
        })
        .await
        .unwrap();
        assert!(results.is_empty());
        assert_eq!(
            requests,
            [
                "THE Théâtre of the Other",
                "Théâtre of Other"
            ]
        );
    }

    #[tokio::test]
    async fn queries_without_articles_or_searchable_terms_do_not_retry() {
        for query in [
            "Other",
            "Them",
            "Théâtre",
            "The the",
            "the",
            "",
        ] {
            let mut requests = Vec::new();
            assert!(
                super::search_with_fallback(query, |query| {
                    requests.push(query);
                    std::future::ready(Ok(Vec::new()))
                })
                .await
                .unwrap()
                .is_empty()
            );
            assert_eq!(requests, [query]);
        }
    }

    #[tokio::test]
    async fn provider_errors_are_not_treated_as_empty_searches() {
        let mut requests = Vec::new();
        let error = super::search_with_fallback("The Show", |query| {
            requests.push(query);
            std::future::ready(Err("provider unavailable".to_string()))
        })
        .await
        .unwrap_err();
        assert_eq!(error, "provider unavailable");
        assert_eq!(requests, ["The Show"]);
    }

    #[test]
    fn cached_auth_tokens_are_bound_to_the_resolved_api_key() {
        use sha2::Digest;
        let first = sha2::Sha256::digest(b"first key").to_vec();
        let second = sha2::Sha256::digest(b"replacement key").to_vec();
        let now = std::time::Instant::now();
        let cached = super::TvdbCachedToken {
            token: "token".into(),
            credential_fingerprint: first.clone(),
            expires_at: now + std::time::Duration::from_secs(10),
        };
        assert!(cached.matches(&first, now));
        assert!(!cached.matches(&second, now));
        assert!(!cached.matches(&first, now + std::time::Duration::from_secs(11)));
    }
    #[test]
    fn popularity_is_never_persisted_as_a_user_rating() {
        let response: super::Response<super::MovieExtendedRecord> =
            serde_json::from_value(serde_json::json!({
                "data": {
                    "id": 901,
                    "name": "Movie",
                    "score": 123456
                }
            }))
            .unwrap();
        let snapshot = super::response::snapshot(&response, "901", "movie", "eng");
        assert_eq!(super::metadata_details(&snapshot).rating, None);
    }
    #[test]
    fn descendants_only_use_the_series_default_season_order() {
        let series = serde_json::from_value::<super::SeriesExtendedRecord>(serde_json::json!({
            "defaultSeasonType": 2,
            "seasons": [
                {"id": 10, "number": 1, "type": {"id": 1}},
                {"id": 20, "number": 1, "type": {"id": 2}},
                {"id": 30, "number": 1, "type": {"id": 3}}
            ]
        }))
        .unwrap();
        let seasons = super::default_order_seasons(&series).unwrap();
        assert_eq!(seasons.len(), 1);
        assert_eq!(seasons[0].id, Some(20));
        assert!(super::default_order_seasons(&super::SeriesExtendedRecord::default()).is_err());
    }
    use super::*;
    use serde_json::json;

    #[test]
    fn search_uses_api_record_id_and_documented_translations() {
        let item: SearchResult = serde_json::from_value(json!({
            "tvdb_id": "711",
            "id": "movie-711",
            "objectID": "movie-711",
            "type": "movie",
            "name": "T-34",
            "translations": {
                "eng": "English title"
            },
            "overviews": {
                "eng": "English overview"
            },
            "year": "2018",
            "image_url": "https://example.test/poster.jpg"
        }))
        .unwrap();
        let result = response::search_result(item, "eng").unwrap();
        assert_eq!(result.external_id, "711");
        assert_eq!(result.title, "English title");
        assert_eq!(result.overview.as_deref(), Some("English overview"));
        assert_eq!(result.release_year, Some(2018));
        let item: SearchResult = serde_json::from_value(json!({
            "objectID": "movie-711",
            "type": "movie",
            "name": "T-34"
        }))
        .unwrap();
        assert!(response::search_result(item, "eng").is_none());
    }

    #[test]
    fn extended_record_uses_translation_endpoint_and_exact_artwork_fields() {
        let record: Response<MovieExtendedRecord> = serde_json::from_value(json!({
            "data": {
                "id": 901,
                "name": "Original",
                "artworks": [
                    {
                        "type": 14,
                        "image": "/poster.jpg"
                    },
                    {
                        "type": 15,
                        "image": "/backdrop.jpg"
                    },
                    {
                        "type": 25,
                        "image": "/logo.png",
                        "language": "eng"
                    }
                ],
                "genres": [
                    {
                        "name": "Action"
                    }
                ],
                "remoteIds": [
                    {
                        "id": "tt1234567",
                        "type": 2
                    }
                ]
            },
            "translation": {
                "language": "eng",
                "name": "Translated",
                "overview": "Overview",
                "tagline": "Tagline"
            }
        }))
        .unwrap();
        let snapshot = response::snapshot(&record, "901", "movie", "eng");
        assert_eq!(snapshot.title.as_deref(), Some("Translated"));
        assert_eq!(snapshot.overview.as_deref(), Some("Overview"));
        let details = metadata_details(&snapshot);
        assert_eq!(details.tagline.as_deref(), Some("Tagline"));
        assert_eq!(details.genres, vec!["Action"]);
        assert_eq!(
            details.logo_url.as_deref(),
            Some("https://artworks.thetvdb.com/logo.png")
        );
        assert!(
            details
                .external_ids
                .iter()
                .any(|id| id.source == "imdb" && id.external_id == "tt1234567")
        );
    }

    #[test]
    fn movie_records_accept_empty_and_populated_company_arrays() {
        // Reduced from failed movie responses: an empty first category used to
        // report a Company length error, and a populated one a string error.
        for companies in [
            json!({
                "studio": [],
                "network": [],
                "production": [{"id": 1928, "name": "Production company"}],
                "distributor": [],
                "special_effects": []
            }),
            json!({
                "studio": [{"id": 2053, "name": "Studio"}],
                "network": [],
                "production": [],
                "distributor": [],
                "special_effects": []
            }),
            serde_json::Value::Null,
        ] {
            let payload = json!({
                "data": {
                    "id": 117712,
                    "name": "Movie",
                    "first_release": {"date": "2020-10-01"},
                    "translations": {
                        "nameTranslations": [{"language": "eng", "name": "English title"}],
                        "overviewTranslations": [{"language": "eng", "overview": "Overview"}]
                    },
                    "artworks": [{"type": 14, "image": "/poster.jpg"}],
                    "genres": [{"id": 1, "name": "Action"}],
                    "contentRatings": [{"country": "usa", "name": "R"}],
                    "remoteIds": [{"id": "tt1234567", "type": 2}],
                    "trailers": [{
                        "url": "https://www.youtube.com/watch?v=abcdefghijk",
                        "language": "eng"
                    }],
                    "characters": [{
                        "peopleId": 42,
                        "personName": "Person",
                        "peopleType": "Actor",
                        "name": "Character"
                    }],
                    "companies": companies
                }
            })
            .to_string();
            let record: Response<MovieExtendedRecord> = serde_json::from_str(&payload).unwrap();
            let mut snapshot = response::snapshot(&record, "117712", "movie", "eng");
            assert_eq!(snapshot.title.as_deref(), Some("English title"));
            assert_eq!(snapshot.overview.as_deref(), Some("Overview"));
            assert_eq!(snapshot.release_year, Some(2020));
            assert_eq!(
                snapshot.artwork_url.as_deref(),
                Some("https://artworks.thetvdb.com/poster.jpg")
            );
            // Both newly serialized snapshots and previously cached full API
            // responses must retain their metadata when decoded again.
            for payload in [
                snapshot.provider_payload_json.clone().unwrap(),
                payload,
            ] {
                snapshot.provider_payload_json = Some(payload);
                let details = metadata_details(&snapshot);
                assert_eq!(details.genres, vec!["Action"]);
                assert_eq!(details.content_rating.as_deref(), Some("R"));
                assert!(
                    details
                        .external_ids
                        .iter()
                        .any(|id| id.source == "imdb" && id.external_id == "tt1234567")
                );
                assert_eq!(
                    details.trailer_url.as_deref(),
                    Some("https://www.youtube.com/watch?v=abcdefghijk")
                );
                assert_eq!(details.people.len(), 1);
                assert_eq!(details.people[0].external_id.as_deref(), Some("42"));
                assert_eq!(
                    details.people[0].character_name.as_deref(),
                    Some("Character")
                );
            }
        }
    }

    #[test]
    fn characters_use_people_type_person_id_and_person_image() {
        let record: Response<SeriesExtendedRecord> = serde_json::from_value(json!({
            "data": {
                "characters": [
                    {
                        "id": 999,
                        "peopleId": 42,
                        "personName": "A Director",
                        "peopleType": "Director",
                        "type": 3,
                        "image": "/character.jpg",
                        "personImgURL": "/person.jpg",
                        "sort": 7
                    }
                ]
            }
        }))
        .unwrap();
        let snapshot = response::snapshot(&record, "901", "series", "eng");
        let people = metadata_details(&snapshot).people;
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].external_id.as_deref(), Some("42"));
        assert_eq!(people[0].role.as_deref(), Some("Director"));
        assert_eq!(people[0].department.as_deref(), Some("Crew"));
        assert_eq!(
            people[0].image_url.as_deref(),
            Some("https://artworks.thetvdb.com/person.jpg")
        );
        assert_eq!(people[0].sort_order, 7);
        let record: Response<SeriesExtendedRecord> = serde_json::from_value(json!({
            "data": {
                "characters": [
                    {
                        "id": 999,
                        "personName": "Missing person ID",
                        "personId": 42
                    }
                ]
            }
        }))
        .unwrap();
        let snapshot = response::snapshot(&record, "901", "series", "eng");
        assert!(metadata_details(&snapshot).people.is_empty());
    }

    #[test]
    fn television_records_accept_company_arrays_throughout_the_hierarchy() {
        for studio in [
            json!([]),
            json!([{"id": 2053, "name": "Studio"}]),
        ] {
            let companies = json!({
                "studio": studio,
                "network": [],
                "production": [],
                "distributor": [],
                "special_effects": []
            });
            let season_identity = json!({
                "id": 9900,
                "number": 1,
                "type": {"id": 1},
                "companies": companies
            });
            let series: Response<SeriesExtendedRecord> = serde_json::from_value(json!({
                "data": {
                    "name": "Show",
                    "defaultSeasonType": 1,
                    "seasons": [season_identity]
                }
            }))
            .unwrap();
            let seasons = default_order_seasons(&series.data).unwrap();
            assert_eq!(seasons.len(), 1);
            assert_eq!(seasons[0].id, Some(9900));
            assert_eq!(seasons[0].number, Some(1));

            let discovery: Response<response::SeasonEpisodes> = serde_json::from_value(json!({
                "data": {
                    "companies": companies,
                    "episodes": [{"id": 12345, "number": 1}]
                }
            }))
            .unwrap();
            let episode = &discovery.data.episodes.as_ref().unwrap()[0];
            assert_eq!(episode.id, Some(12345));
            assert_eq!(episode.number, Some(1));

            let season: Response<SeasonExtendedRecord> = serde_json::from_value(json!({
                "data": {
                    "id": 9900,
                    "companies": companies,
                    "episodes": [{"id": 12345, "number": 1}],
                    "translations": {
                        "nameTranslations": [{"language": "eng", "name": "Season One"}],
                        "overviewTranslations": [{"language": "eng", "overview": "Season overview"}]
                    },
                    "artwork": [{"type": 7, "image": "/season.jpg"}]
                }
            }))
            .unwrap();
            let snapshot = response::snapshot(&season, "9900", "season", "eng");
            assert_eq!(snapshot.title.as_deref(), Some("Season One"));
            assert_eq!(snapshot.overview.as_deref(), Some("Season overview"));
            assert_eq!(
                snapshot.artwork_url.as_deref(),
                Some("https://artworks.thetvdb.com/season.jpg")
            );

            let episode: Response<EpisodeExtendedRecord> = serde_json::from_value(json!({
                "data": {
                    "name": "Episode One",
                    "overview": "Episode overview",
                    "aired": "2020-01-02",
                    "image": "/episode.jpg",
                    "seasons": [season_identity],
                    "contentRatings": [{"name": "TV-14", "country": "usa"}]
                }
            }))
            .unwrap();
            let snapshot = response::snapshot(&episode, "12345", "episode", "eng");
            assert_eq!(snapshot.title.as_deref(), Some("Episode One"));
            assert_eq!(snapshot.overview.as_deref(), Some("Episode overview"));
            assert_eq!(snapshot.release_year, Some(2020));
            assert_eq!(
                metadata_details(&snapshot).content_rating.as_deref(),
                Some("TV-14")
            );
        }
    }

    #[test]
    fn people_preserve_numeric_gender_and_requested_biography() {
        let data: PeopleExtendedRecord = serde_json::from_value(json!({
            "id": 42,
            "name": "Person",
            "gender": 2,
            "birth": "1970-01-01",
            "biographies": [
                {
                    "language": "eng",
                    "biography": "English"
                },
                {
                    "language": "spa",
                    "biography": "Spanish"
                }
            ]
        }))
        .unwrap();
        assert_eq!(data.gender, Some(2));
        let person = Person {
            data,
            cached_image_path: None,
            gender_label: Some("Female".into()),
        };
        let details = response::person_details(&person, "spa").unwrap();
        assert_eq!(details.gender.as_deref(), Some("Female"));
        assert_eq!(details.biography.as_deref(), Some("Spanish"));
        assert_eq!(
            response::person_details(&person, "fra").unwrap().biography,
            None
        );
    }

    #[test]
    fn season_metadata_decodes_the_translation_object_for_the_requested_locale() {
        // `meta=translations` uses the same object fields as series/movies,
        // while the discovery request omits this object entirely.
        let record: Response<SeasonExtendedRecord> = serde_json::from_value(json!({
            "data": {
                "id": 492243,
                "image": "/season.jpg",
                "year": "2012",
                "translations": {
                    "nameTranslations": [
                        {"language": "eng", "name": "Season One"},
                        {"language": "spa", "name": "Temporada Uno"}
                    ],
                    "overviewTranslations": [
                        {"language": "eng", "overview": "English overview"},
                        {"language": "spa", "overview": "Spanish overview"}
                    ],
                    "aliases": []
                }
            }
        }))
        .unwrap();
        for (language, title, overview) in [
            ("eng", "Season One", "English overview"),
            ("spa", "Temporada Uno", "Spanish overview"),
        ] {
            let snapshot = response::snapshot(&record, "492243", "season", language);
            assert_eq!(snapshot.title.as_deref(), Some(title));
            assert_eq!(snapshot.overview.as_deref(), Some(overview));
            assert_eq!(snapshot.release_year, Some(2012));
            assert_eq!(
                snapshot.artwork_url.as_deref(),
                Some("https://artworks.thetvdb.com/season.jpg")
            );
            let stored: Response<SeasonExtendedRecord> =
                serde_json::from_str(snapshot.provider_payload_json.as_deref().unwrap()).unwrap();
            assert_eq!(
                response::snapshot(&stored, "492243", "season", language).overview,
                snapshot.overview
            );
        }
    }

    #[test]
    fn endpoint_models_reject_invented_field_types() {
        assert!(
            serde_json::from_value::<Response<MovieExtendedRecord>>(json!({
                "data": {
                    "remoteIds": [
                        {
                            "id": 123,
                            "type": 2
                        }
                    ]
                }
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<Response<SeriesExtendedRecord>>(json!({
                "data": {
                    "characters": [
                        {
                            "type": "Actor"
                        }
                    ]
                }
            }))
            .is_err()
        );
        assert!(parse_tvdb_external_id("16777217", "episode").is_ok_and(|id| id == 16777217));
        assert!(parse_tvdb_external_id("NaN", "episode").is_err());
    }
}
