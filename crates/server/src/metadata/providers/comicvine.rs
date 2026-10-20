//! Comic Vine issues and volumes. Exact typed IDs prevent mixing an issue with a series.

use super::*;
use serde_json::Value;
use std::collections::{
    HashMap,
    VecDeque,
};
use std::time::{
    Duration,
    Instant,
};

pub(super) struct ComicVineMetadataProvider;
#[cfg(test)]
mod tests;
#[cfg(test)]
tokio::task_local! { pub(crate) static TEST_API_ROOT: String; }

fn valid_id(
    id: &str,
    kind: &str,
) -> bool {
    let prefix = if kind == "comic_issue" {
        "4000-"
    } else if kind == "comic_series" {
        "4050-"
    } else {
        return false;
    };
    id.strip_prefix(prefix).is_some_and(|number| {
        !number.is_empty() && number.len() <= 16 && number.bytes().all(|b| b.is_ascii_digit())
    })
}

fn plain(raw: &str) -> String {
    static TAGS: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| regex::Regex::new(r"<[^>]*>").unwrap());
    TAGS.replace_all(raw, " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

async fn get(
    settings: &MetadataSettings,
    path: &str,
    mut query: Vec<(&str, String)>,
) -> Result<Value, String> {
    let mut provider = settings
        .providers
        .iter()
        .find(|provider| provider.id == MetadataProviderId::ComicVine && provider.enabled)
        .cloned()
        .ok_or("Enable Comic Vine in Metadata providers")?;
    crate::config::resolve_metadata_provider_api_key(&mut provider)?;
    let key = provider
        .api_key
        .filter(|key| !key.is_empty())
        .ok_or("Comic Vine requires an API key")?;
    let root = "https://comicvine.gamespot.com/api".to_string();
    #[cfg(test)]
    let root = TEST_API_ROOT.try_with(Clone::clone).unwrap_or(root);
    let cache = crate::metadata::metadata_response_cache_key(
        &MetadataProviderId::ComicVine,
        "api",
        &[
            &root,
            path,
            &serde_json::to_string(&query).unwrap(),
        ],
    );
    if let Some(raw) = crate::metadata::read_metadata_response_cache_text(&cache) {
        if let Ok(value) = serde_json::from_str(&raw) {
            return Ok(value);
        }
    }
    if root.starts_with("https://comicvine.gamespot.com/") {
        // Published allowance: 200 requests per resource/hour. Cached hits do not use quota.
        static QUOTA: once_cell::sync::Lazy<
            tokio::sync::Mutex<HashMap<String, VecDeque<Instant>>>,
        > = once_cell::sync::Lazy::new(Default::default);
        let mut quota = QUOTA.lock().await;
        let resource = path.split('/').next().unwrap_or(path).to_string();
        let timestamps = quota.entry(resource).or_default();
        while timestamps
            .front()
            .is_some_and(|time| time.elapsed() >= Duration::from_secs(3600))
        {
            timestamps.pop_front();
        }
        if timestamps.len() >= 200 {
            return Err(
                "Comic Vine's hourly request limit has been reached. Try again later.".into(),
            );
        }
        if let Some(last) = timestamps.back() {
            tokio::time::sleep(Duration::from_secs(1).saturating_sub(last.elapsed())).await;
        }
        timestamps.push_back(Instant::now());
    }
    query.push(("api_key", key));
    query.push(("format", "json".into()));
    // Never include request URLs in errors: they contain the API key.
    let response = crate::metadata::http::client()
        .get(format!("{root}/{path}"))
        .query(&query)
        .send()
        .await
        .map_err(|_| "Comic Vine could not be reached")?;
    if !response.status().is_success() {
        return Err(format!(
            "Comic Vine returned HTTP {}",
            response.status().as_u16()
        ));
    }
    let value: Value = response
        .json()
        .await
        .map_err(|_| "Invalid Comic Vine response")?;
    if value["status_code"].as_i64() != Some(1) {
        return Err(
            "Comic Vine rejected the request. Check the API key and account limits.".into(),
        );
    }
    crate::metadata::write_metadata_response_cache_text(&cache, &value.to_string());
    Ok(value)
}

fn title(
    value: &Value,
    kind: &str,
) -> String {
    if kind == "comic_series" {
        return value["name"].as_str().unwrap_or_default().into();
    }
    let series = value
        .pointer("/volume/name")
        .and_then(Value::as_str)
        .unwrap_or("Comic");
    let number = value["issue_number"].as_str().unwrap_or("?");
    let name = value["name"]
        .as_str()
        .filter(|name| !name.trim().is_empty());
    let suffix = name.map(|name| format!(" — {name}")).unwrap_or_default();
    format!("{series} #{number}{suffix}")
}

fn search_result(
    value: &Value,
    kind: &str,
) -> Option<MetadataSearchResult> {
    let number = value["id"].as_u64()?;
    let prefix = if kind == "comic_series" { "4050" } else { "4000" };
    let date = value["cover_date"]
        .as_str()
        .or_else(|| value["start_year"].as_str());
    Some(MetadataSearchResult {
        provider_id: MetadataProviderId::ComicVine,
        external_id: format!("{prefix}-{number}"),
        media_type: kind.into(),
        title: title(value, kind),
        overview: value["deck"]
            .as_str()
            .or_else(|| value["description"].as_str())
            .map(plain),
        artwork_url: value
            .pointer("/image/super_url")
            .and_then(Value::as_str)
            .map(str::to_string),
        backdrop_url: None,
        release_year: date.and_then(|date| date.get(..4)?.parse().ok()),
        score: None,
        track_count: None,
    })
}

impl MetadataProvider for ComicVineMetadataProvider {
    fn descriptor(&self) -> MetadataProviderDescriptor {
        MetadataProviderDescriptor {
            id: MetadataProviderId::ComicVine,
            display_name: "Comic Vine".into(),
            description: "Comic issues, volumes, descriptions, publication dates, and covers. \
                          Requires a personal API key; non-commercial use only."
                .into(),
            supported_kinds: vec![MediaLibraryKind::Books],
            requires_api_key: true,
            implemented: true,
            role: MetadataProviderRole::Primary,
            extends_provider_ids: Vec::new(),
            attribution_text: "Comic metadata provided by Comic Vine.".into(),
            attribution_url: "https://comicvine.gamespot.com/".into(),
            logo_light_url: Some("/assets/providers/comicvine.svg".into()),
            logo_dark_url: Some("/assets/providers/comicvine.svg".into()),
        }
    }

    fn search<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        query: &'a str,
        media_type: Option<&'a str>,
    ) -> MetadataProviderFuture<'a, Vec<MetadataSearchResult>> {
        Box::pin(async move {
            let kind = media_type.unwrap_or("comic_issue");
            if !matches!(kind, "comic_issue" | "comic_series") {
                return Ok(Vec::new());
            }
            let resource = if kind == "comic_series" { "volume" } else { "issue" };
            let response = get(
                settings,
                "search/",
                vec![
                    ("query", query.into()),
                    ("resources", resource.into()),
                    ("limit", "20".into()),
                ],
            )
            .await?;
            Ok(response["results"]
                .as_array()
                .ok_or("Comic Vine search results are missing")?
                .iter()
                .filter_map(|value| search_result(value, kind))
                .collect())
        })
    }

    fn fetch_snapshot<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        external_id: &'a str,
        media_type: &'a str,
        _people: bool,
    ) -> MetadataProviderFuture<'a, StoredMetadataSnapshot> {
        Box::pin(async move {
            if !valid_id(external_id, media_type) {
                return Err("Invalid Comic Vine identifier or record type".into());
            }
            let resource = if media_type == "comic_series" { "volume" } else { "issue" };
            let response = get(settings, &format!("{resource}/{external_id}/"), Vec::new()).await?;
            let value = &response["results"];
            let result = search_result(value, media_type)
                .filter(|result| result.external_id == external_id)
                .ok_or("Comic Vine returned a different record")?;
            Ok(StoredMetadataSnapshot {
                provider_id: MetadataProviderId::ComicVine,
                external_id: result.external_id,
                media_type: Some(media_type.into()),
                title: Some(result.title),
                overview: result.overview,
                artwork_url: result.artwork_url,
                backdrop_url: None,
                release_year: result.release_year,
                locale_key: String::new(),
                provider_locale_key: None,
                provider_payload_json: Some(value.to_string()),
            })
        })
    }

    fn metadata_details(
        &self,
        snapshot: &StoredMetadataSnapshot,
    ) -> ProviderMetadataDetails {
        ProviderMetadataDetails {
            external_ids: vec![
                crate::metadata::ProviderExternalId {
                    source: "comicvine".into(),
                    external_id: snapshot.external_id.clone(),
                },
            ],
            genres: vec!["Comics".into()],
            ..Default::default()
        }
    }
}
