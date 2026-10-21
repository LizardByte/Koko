//! Shared ListenBrainz similarity requests for metadata refreshes and explicit radio actions.

use once_cell::sync::Lazy;
use rocket::{
    http::Status,
    tokio::sync::Mutex,
};
use std::{
    collections::HashMap,
    time::{
        Duration,
        Instant,
    },
};

static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(6))
        .user_agent(concat!(
            "Koko/",
            env!("CARGO_PKG_VERSION"),
            " (https://github.com/LizardByte/Koko)"
        ))
        .build()
        .expect("music discovery HTTP client")
});
type Cached = (Instant, serde_json::Value);
pub(crate) const RECORDING_ALGORITHM: &str = concat!(
    "session_based_days_7500_session_300_contribution_5_",
    "threshold_15_limit_50_skip_30_top_n_listeners_1000"
);
pub(crate) const ARTIST_ALGORITHM: &str = concat!(
    "session_based_days_7500_session_300_contribution_5_",
    "threshold_10_limit_100_filter_True_skip_30"
);
static CACHE: Lazy<Mutex<HashMap<String, Cached>>> = Lazy::new(|| Mutex::new(HashMap::new()));
pub(crate) async fn provider_json(
    url: &str,
    params: &[(&str, String)],
) -> Result<Option<serde_json::Value>, Status> {
    let request = CLIENT
        .get(url)
        .query(params)
        .build()
        .map_err(|_| Status::BadRequest)?;
    let key = request.url().to_string();
    if let Some((time, payload)) = CACHE.lock().await.get(&key) {
        if !super::cache_policy::bypassed() && time.elapsed() < Duration::from_secs(3600) {
            return Ok(Some(payload.clone()));
        }
    }
    let first = CLIENT
        .execute(request.try_clone().ok_or(Status::InternalServerError)?)
        .await;
    let retry = first.as_ref().map_or(true, |response| {
        response.status().is_server_error()
            || response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
    });
    // Retry transient recommendation-service failures once before using local radio.
    let response = if retry {
        rocket::tokio::time::sleep(Duration::from_millis(250)).await;
        CLIENT.execute(request).await
    } else {
        first
    }
    .map_err(|error| {
        log::warn!("Music discovery request failed: {error}");
        Status::BadGateway
    })?;
    let mut response = if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    } else {
        response
            .error_for_status()
            .map_err(|_| Status::BadGateway)?
    };
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Status::BadGateway)? {
        if bytes.len() + chunk.len() > 1024 * 1024 {
            return Err(Status::BadGateway);
        }
        bytes.extend_from_slice(&chunk);
    }
    let payload =
        serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|_| Status::BadGateway)?;
    let mut cache = CACHE.lock().await;
    if cache.len() >= 128 {
        cache.clear();
    }
    cache.insert(key, (Instant::now(), payload.clone()));
    Ok(Some(payload))
}
