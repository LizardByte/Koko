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
    Person,
    Record,
    Response,
};
use serde::de::DeserializeOwned;
use std::time::{
    Duration,
    Instant,
};
use strsim::normalized_levenshtein;
use tvdb4::apis::{
    self,
    login_api,
};
use tvdb4::models::{
    EpisodeExtendedRecord,
    Gender,
    LoginPostRequest,
    MovieExtendedRecord,
    PeopleExtendedRecord,
    SearchResult,
    SeasonExtendedRecord,
    SeriesExtendedRecord,
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
    expires_at: Instant,
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
    let mut params = vec![
        ("query", query.to_string()),
        ("limit", "20".to_string()),
    ];
    if let Some(kind) = media_type {
        params.push(("type", kind.to_string()));
    }
    let response: Response<Vec<SearchResult>> =
        get_json(&provider, "search", params, "search").await?;
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
    for season in series.data.seasons.unwrap_or_default() {
        let (Some(id), Some(number)) = (
            season.id,
            season.number.and_then(|number| i32::try_from(number).ok()),
        ) else {
            continue;
        };
        let season: Response<SeasonExtendedRecord> = get_json(
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

    wait_for_rate_limit().await;
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
                    let backoff =
                        retry_after.unwrap_or_else(|| base_backoff.saturating_mul(multiplier));
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
