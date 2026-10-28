//! Cached Apple podcast charts and search; discovery never imports catalog entries into the
//! database.
use super::http;
use once_cell::sync::Lazy;
use rocket_okapi::okapi::schemars::{
    self,
    JsonSchema,
};
use serde::{
    Deserialize,
    Serialize,
};
use serde_json::Value;
use std::{
    collections::HashMap,
    time::{
        Duration,
        Instant,
    },
};
use tokio::sync::Mutex;

/// A discovery result with a public RSS feed, publisher artwork, and directory attribution.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DirectoryPodcast {
    /// Apple directory identifier.
    pub id: String,
    /// Podcast title.
    pub title: String,
    /// Publisher or author name.
    pub author: Option<String>,
    /// Publisher cover image URL.
    pub artwork_url: Option<String>,
    /// Apple Podcasts listing for directory attribution.
    pub apple_url: Option<String>,
    /// Public subscription feed URL, when this is the root show.
    pub feed_url: String,
}
type Cache = HashMap<String, (Instant, Vec<DirectoryPodcast>)>;
static CACHE: Lazy<Mutex<Cache>> = Lazy::new(|| Mutex::new(HashMap::new()));
static LAST_REQUEST: Lazy<Mutex<Option<Instant>>> = Lazy::new(|| Mutex::new(None));

async fn json(url: &str) -> Result<Value, String> {
    // Apple documents approximately 20 Search API calls/minute; share this limit across users.
    let mut previous = LAST_REQUEST.lock().await;
    if let Some(time) = *previous {
        if let Some(delay) = Duration::from_secs(3).checked_sub(time.elapsed()) {
            tokio::time::sleep(delay).await;
        }
    }
    *previous = Some(Instant::now());
    let (_, body) = http::fetch_text(url, 2 * 1024 * 1024).await?;
    serde_json::from_str(&body)
        .map_err(|_| "The podcast directory returned an invalid response".into())
}

pub(super) fn results(value: &Value) -> Vec<DirectoryPodcast> {
    value
        .get("results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| {
            let feed_url = http::normalize_url(row.get("feedUrl")?.as_str()?).ok()?;
            Some(DirectoryPodcast {
                id: row
                    .get("collectionId")
                    .or_else(|| row.get("trackId"))?
                    .as_i64()?
                    .to_string(),
                title: row
                    .get("collectionName")
                    .or_else(|| row.get("trackName"))?
                    .as_str()?
                    .to_owned(),
                author: row
                    .get("artistName")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                artwork_url: row
                    .get("artworkUrl600")
                    .or_else(|| row.get("artworkUrl100"))
                    .and_then(Value::as_str)
                    .and_then(|v| http::normalize_url(v).ok()),
                apple_url: row
                    .get("collectionViewUrl")
                    .or_else(|| row.get("trackViewUrl"))
                    .and_then(Value::as_str)
                    .and_then(|v| http::normalize_url(v).ok()),
                feed_url,
            })
        })
        .collect()
}

/// Browse popular shows by country, or search the directory without an API key.
pub async fn discover(
    country: &str,
    query: &str,
) -> Result<Vec<DirectoryPodcast>, String> {
    if country.len() != 2
        || !country.bytes().all(|c| c.is_ascii_alphabetic())
        || query.chars().count() > 200
    {
        return Err("Use a two-letter country code and a search of at most 200 characters".into());
    }
    let country = country.to_ascii_lowercase();
    let query = query.trim();
    let key = format!("{country}:{query}");
    let mut cache = CACHE.lock().await;
    if let Some((time, items)) = cache.get(&key).filter(|(time, _)| {
        time.elapsed() < Duration::from_secs(if query.is_empty() { 21600 } else { 3600 })
    }) {
        let _ = time;
        return Ok(items.clone());
    }
    let result = if query.is_empty() {
        let charts = json(&format!(
            "https://rss.applemarketingtools.com/api/v2/{country}/podcasts/top/50/podcasts.json"
        ))
        .await?;
        let ids = charts
            .pointer("/feed/results")
            .and_then(Value::as_array)
            .ok_or("The podcast chart is unavailable")?
            .iter()
            .filter_map(|row| row.get("id")?.as_str())
            .filter(|id| id.bytes().all(|c| c.is_ascii_digit()))
            .collect::<Vec<_>>();
        if ids.is_empty() {
            return Err("The podcast chart is unavailable for this country".into());
        }
        let lookup = json(&format!(
            "https://itunes.apple.com/lookup?id={}&entity=podcast&country={country}",
            ids.join(",")
        ))
        .await?;
        let mut items = results(&lookup);
        items.sort_by_key(|item| {
            ids.iter()
                .position(|id| *id == item.id)
                .unwrap_or(usize::MAX)
        });
        items
    } else {
        let mut url = reqwest::Url::parse("https://itunes.apple.com/search").unwrap();
        url.query_pairs_mut().extend_pairs([
            ("media", "podcast"),
            ("entity", "podcast"),
            ("limit", "50"),
            ("country", country.as_str()),
            ("term", query),
        ]);
        results(&json(url.as_str()).await?)
    };
    if cache.len() >= 128 {
        cache.retain(|_, (time, _)| time.elapsed() < Duration::from_secs(3600));
        if cache.len() >= 128 {
            cache.clear();
        }
    }
    cache.insert(key, (Instant::now(), result.clone()));
    Ok(result)
}
