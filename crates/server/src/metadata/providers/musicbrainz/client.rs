//! Crate-backed MusicBrainz requests, with one process-wide rate limit.

use std::time::Duration;

use musicbrainz_rs::api_bindium::ureq::Agent;
use musicbrainz_rs::api_bindium::{
    ApiClient,
    ApiRequest,
    ApiRequestError,
};
use musicbrainz_rs::entity::{
    artist::Artist,
    recording::Recording,
    release::Release,
    release_group::ReleaseGroup,
};
use musicbrainz_rs::{
    Fetch,
    MusicBrainzClient,
    MusicBrainzParser,
    Search,
};
use once_cell::sync::Lazy;
use tokio::{
    sync::Mutex,
    time::Instant,
};

type Request = ApiRequest<MusicBrainzParser<serde_json::Value>>;

static NEXT_REQUEST: Lazy<Mutex<Option<Instant>>> = Lazy::new(|| Mutex::new(None));
const USER_AGENT: &str = concat!(
    "Koko/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/LizardByte/Koko)"
);
static CLIENT: Lazy<MusicBrainzClient> = Lazy::new(|| {
    let config = MusicBrainzClient::default_agent_config_builder()
        .user_agent(USER_AGENT)
        .timeout_global(Some(Duration::from_secs(30)))
        .timeout_connect(Some(Duration::from_secs(10)))
        .build();
    MusicBrainzClient::builder()
        .api_client(
            ApiClient::builder()
                .agent(Agent::new_with_config(config))
                .build(),
        )
        .build()
});

fn client() -> MusicBrainzClient {
    #[allow(unused_mut)]
    let mut client = CLIENT.clone();
    #[cfg(test)]
    if let Ok(root) = super::TEST_API_ROOT.try_with(Clone::clone) {
        let url = reqwest::Url::parse(&root).expect("test MusicBrainz endpoint");
        client.musicbrainz_use_https = url.scheme() == "https";
        client.musicbrainz_authority = match url.port() {
            Some(port) => format!("{}:{port}", url.host_str().unwrap()),
            None => url.host_str().unwrap().into(),
        };
    }
    client
}

// Keep Koko's small normalization DTO at the boundary: the crate's Release
// currently omits search's top-level track-count and requires media IDs that
// MusicBrainz does not supply in search results. The crate still builds, sends,
// bounds and parses every request.
fn payload<P>(request: ApiRequest<P>) -> Request {
    request.set_parser(MusicBrainzParser::<serde_json::Value>::default())
}

pub(super) async fn search(
    query: &str,
    entity: &str,
) -> Result<serde_json::Value, String> {
    let client = client();
    request_payload(search_request(query, entity, &client)?, &client).await
}

pub(super) fn search_request(
    query: &str,
    entity: &str,
    client: &MusicBrainzClient,
) -> Result<Request, String> {
    // api_bindium 0.5 escapes a whole query, leaving parameter separators and
    // literal plus/percent signs untouched. Escape these before its URL builder.
    let query = query
        .replace('%', "%25")
        .replace('&', "%26")
        .replace('+', "%2B");
    let request = match entity {
        "artist" => Artist::search(query)
            .limit(20)
            .as_api_request(client)
            .map(payload),
        "release" => Release::search(query)
            .limit(20)
            .as_api_request(client)
            .map(payload),
        "release-group" => ReleaseGroup::search(query)
            .limit(20)
            .as_api_request(client)
            .map(payload),
        "recording" => Recording::search(query)
            .limit(20)
            .as_api_request(client)
            .map(payload),
        _ => return Err(format!("Unsupported MusicBrainz entity: {entity}")),
    }
    .map_err(|error| error.to_string())?;
    Ok(request)
}

pub(super) async fn fetch(
    id: &str,
    entity: &str,
) -> Result<serde_json::Value, String> {
    let client = client();
    let request = match entity {
        "artist" => artist_request(id, true, &client),
        "release" => Release::fetch()
            .id(id)
            .with_artist_credits()
            .with_genres()
            .with_release_groups()
            .as_api_request(&client)
            .map(payload),
        "release-group" => ReleaseGroup::fetch()
            .id(id)
            .with_artists()
            .with_genres()
            .as_api_request(&client)
            .map(payload),
        "recording" => Recording::fetch()
            .id(id)
            .with_artists()
            .with_genres()
            .as_api_request(&client)
            .map(payload),
        _ => return Err(format!("Unsupported MusicBrainz entity: {entity}")),
    }
    .map_err(|error| error.to_string())?;
    request_payload(request, &client).await
}

fn artist_request(
    id: &str,
    members: bool,
    client: &MusicBrainzClient,
) -> Result<Request, musicbrainz_rs::api_bindium::endpoints::UriBuilderError> {
    let mut query = Artist::fetch();
    query.id(id).with_genres().with_url_relations();
    if members {
        query.with_artist_relations();
    }
    query.as_api_request(client).map(payload)
}

pub(super) async fn person(id: &str) -> Result<serde_json::Value, String> {
    let client = client();
    let request = artist_request(id, false, &client).map_err(|error| error.to_string())?;
    request_payload(request, &client).await
}

async fn request_payload(
    mut request: Request,
    client: &MusicBrainzClient,
) -> Result<serde_json::Value, String> {
    let mut next = NEXT_REQUEST.lock().await;
    // Disable the crate's internal retry loop so retries also pass through our
    // strict limiter, rather than its default allowance for bursts.
    request.max_tries = Some(1);
    for attempt in 0..3 {
        if let Some(deadline) = *next {
            tokio::time::sleep_until(deadline).await;
        }
        *next = Some(Instant::now() + Duration::from_millis(1100));
        request.reset();
        request.retry_after = std::time::Instant::now();
        let response = match request.send_async(&client.api_client).await {
            Ok(response) => response,
            Err(ApiRequestError::MaxRetriesExceeded { .. }) if attempt < 2 => {
                // The crate records Retry-After for 503s and backoff for transport failures.
                let delay = request
                    .retry_after
                    .saturating_duration_since(std::time::Instant::now())
                    .clamp(Duration::from_millis(1100), Duration::from_secs(60));
                *next = Some(Instant::now() + delay);
                continue;
            }
            Err(error) => return Err(format!("MusicBrainz request failed: {error}")),
        };
        let status = response.inner.data.status();
        if status.as_u16() == 429 && attempt < 2 {
            let delay = response
                .inner
                .data
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(2)
                .clamp(2, 60);
            *next = Some(Instant::now() + Duration::from_secs(delay));
            continue;
        }
        if !status.is_success() {
            return Err(format!("MusicBrainz request failed: HTTP {status}"));
        }
        // Body reads are blocking in the crate's transport, including its async API.
        return tokio::task::spawn_blocking(move || {
            response
                .parse()
                .map_err(|error| format!("Invalid MusicBrainz response: {error}"))
        })
        .await
        .map_err(|error| format!("MusicBrainz response task failed: {error}"))?;
    }
    unreachable!()
}

#[cfg(test)]
pub(super) async fn test_request(url: reqwest::Url) -> Result<serde_json::Value, String> {
    let request = ApiRequest::builder()
        .uri(
            url.as_str()
                .parse::<musicbrainz_rs::api_bindium::ureq::http::Uri>()
                .unwrap(),
        )
        .verb(musicbrainz_rs::api_bindium::HTTPVerb::Get)
        .parser(MusicBrainzParser::<serde_json::Value>::default())
        .build();
    request_payload(request, &CLIENT).await
}
