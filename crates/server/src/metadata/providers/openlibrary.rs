//! Open Library author and work identities, independent of audio recordings.

mod authors;
mod collections;
pub(crate) use authors::guess_author;

#[cfg(test)]
mod tests;

use super::*;
use serde::{
    Deserialize,
    Serialize,
};
use serde_json::Value;

pub(super) struct OpenLibraryMetadataProvider;

#[cfg(test)]
tokio::task_local! { static TEST_API_ROOT: String; }

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct Work {
    key: String,
    title: String,
    description: Value,
    covers: Vec<i64>,
    subjects: Vec<String>,
    authors: Vec<Value>,
    first_publish_date: Option<String>,
    #[serde(rename = "koko_author_names")]
    author_names: Vec<String>,
    #[serde(rename = "koko_collections")]
    collections: Vec<collections::CuratedCollection>,
    #[serde(rename = "koko_collections_incomplete")]
    collections_incomplete: bool,
    #[serde(rename = "koko_unavailable_collections")]
    unavailable_collections: Vec<String>,
}

/// Accept only canonical work IDs, never paths supplied by an upstream response.
pub(crate) fn valid_work_id(id: &str) -> bool {
    id.strip_prefix("works/OL")
        .and_then(|id| id.strip_suffix('W'))
        .is_some_and(|id| {
            !id.is_empty() && id.len() <= 16 && id.bytes().all(|b| b.is_ascii_digit())
        })
}

fn cover(id: i64) -> Option<String> {
    (id > 0).then(|| format!("https://covers.openlibrary.org/b/id/{id}-L.jpg?default=false"))
}

async fn get(
    path: &str,
    query: &[(String, String)],
) -> Result<Value, String> {
    request(path, query, false)
        .await?
        .ok_or_else(|| "Invalid Open Library query".into())
}

/// Invalid curator-authored searches can be isolated without treating an outage as an empty result.
async fn request(
    path: &str,
    query: &[(String, String)],
    allow_invalid_query: bool,
) -> Result<Option<Value>, String> {
    let root = "https://openlibrary.org".to_string();
    #[cfg(test)]
    let root = TEST_API_ROOT.try_with(Clone::clone).unwrap_or(root);
    let key = crate::metadata::metadata_response_cache_key(
        &MetadataProviderId::OpenLibrary,
        "api",
        &[
            &root,
            path,
            &serde_json::to_string(query).unwrap(),
        ],
    );
    if let Some(raw) = crate::metadata::read_metadata_response_cache_text(&key) {
        if let Ok(value) = serde_json::from_str(&raw) {
            return Ok(Some(value));
        }
    }
    // Stay within the public API's unidentified-client limit, across all requests.
    if root == "https://openlibrary.org" {
        static NEXT: once_cell::sync::Lazy<tokio::sync::Mutex<tokio::time::Instant>> =
            once_cell::sync::Lazy::new(|| tokio::sync::Mutex::new(tokio::time::Instant::now()));
        let mut next = NEXT.lock().await;
        tokio::time::sleep_until(*next).await;
        *next = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
    }
    let response = crate::metadata::http::client()
        .get(format!("{root}/{path}"))
        .query(query)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if allow_invalid_query && response.status() == reqwest::StatusCode::BAD_REQUEST {
        return Ok(None);
    }
    let raw = response
        .error_for_status()
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let value =
        serde_json::from_str(&raw).map_err(|e| format!("Invalid Open Library response: {e}"))?;
    crate::metadata::write_metadata_response_cache_text(&key, &raw);
    Ok(Some(value))
}

async fn search(
    query: &str,
    author: Option<&str>,
) -> Result<Vec<MetadataSearchResult>, String> {
    let mut parameters = vec![
        (
            if author.is_some() { "title" } else { "q" }.into(),
            query.into(),
        ),
        ("limit".into(), "10".into()),
        (
            "fields".into(),
            "key,title,author_name,cover_i,first_publish_year".into(),
        ),
    ];
    if let Some(author) = author {
        parameters.push(("author".into(), author.into()));
    }
    let response = get("search.json", &parameters).await?;
    Ok(response["docs"]
        .as_array()
        .ok_or("Open Library search results are missing")?
        .iter()
        .filter_map(|doc| {
            let id = doc["key"].as_str()?.trim_start_matches('/');
            let title = doc["title"].as_str()?.trim();
            if !valid_work_id(id) || title.is_empty() {
                return None;
            }
            Some(MetadataSearchResult {
                provider_id: MetadataProviderId::OpenLibrary,
                external_id: id.into(),
                media_type: "book".into(),
                title: title.into(),
                overview: Some(
                    doc["author_name"]
                        .as_array()?
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                artwork_url: doc["cover_i"].as_i64().and_then(cover),
                backdrop_url: None,
                release_year: doc["first_publish_year"]
                    .as_i64()
                    .and_then(|year| year.try_into().ok()),
                score: None,
                track_count: None,
            })
        })
        .collect())
}

/// Match a unique work from title and author; a recording is never inferred.
pub(crate) async fn guess(
    title: &str,
    author: &str,
) -> Result<Option<MetadataSearchResult>, String> {
    if author.trim().is_empty() {
        return Ok(None);
    }
    let normalize = |name: &str| {
        name.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    let mut candidates = search(title, Some(author))
        .await?
        .into_iter()
        .filter(|result| {
            super::libex::work_title_matches(title, &result.title, author)
                && result
                    .overview
                    .as_deref()
                    .is_some_and(|name| normalize(name) == normalize(author))
        });
    let first = candidates.next();
    Ok(if candidates.next().is_none() { first } else { None })
}

impl MetadataProvider for OpenLibraryMetadataProvider {
    fn descriptor(&self) -> MetadataProviderDescriptor {
        MetadataProviderDescriptor {
            id: MetadataProviderId::OpenLibrary,
            display_name: "Open Library".into(),
            description: "Book works, authors, descriptions, subjects, and covers for books and \
                          audiobooks."
                .into(),
            supported_kinds: vec![MediaLibraryKind::Books],
            requires_api_key: false,
            implemented: true,
            role: MetadataProviderRole::Primary,
            extends_provider_ids: Vec::new(),
            attribution_text: "Book metadata provided by Open Library, an Internet Archive \
                               initiative."
                .into(),
            attribution_url: "https://openlibrary.org/".into(),
            logo_light_url: Some("/assets/providers/open-library.png".into()),
            logo_dark_url: Some("/assets/providers/open-library.png".into()),
        }
    }

    fn search<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        query: &'a str,
        media_type: Option<&'a str>,
    ) -> MetadataProviderFuture<'a, Vec<MetadataSearchResult>> {
        Box::pin(async move {
            match media_type {
                Some("author") => authors::search(query).await,
                None | Some("book") => search(query, None).await,
                _ => Ok(Vec::new()),
            }
        })
    }

    fn search_with_artist<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        query: &'a str,
        media_type: Option<&'a str>,
        author: Option<&'a str>,
    ) -> MetadataProviderFuture<'a, Vec<MetadataSearchResult>> {
        Box::pin(async move {
            match media_type {
                Some("author") => authors::search(query).await,
                None | Some("book") => search(query, author).await,
                _ => Ok(Vec::new()),
            }
        })
    }

    fn fetch_snapshot<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        external_id: &'a str,
        media_type: &'a str,
        _people: bool,
    ) -> MetadataProviderFuture<'a, StoredMetadataSnapshot> {
        Box::pin(async move {
            if media_type == "author" {
                return authors::snapshot(external_id).await;
            }
            if media_type != "book" || !valid_work_id(external_id) {
                return Err("Invalid Open Library work identifier".into());
            }
            let mut work: Work =
                serde_json::from_value(get(&format!("{external_id}.json"), &[]).await?)
                    .map_err(|e| e.to_string())?;
            if work.key.trim_start_matches('/') != external_id || work.title.trim().is_empty() {
                return Err("Open Library returned a different or incomplete work".into());
            }
            work.author_names.clear();
            for author in work.authors.iter().take(8) {
                let Some(key) = author.pointer("/author/key").and_then(Value::as_str) else {
                    continue;
                };
                let Some(id) = key
                    .strip_prefix("/authors/OL")
                    .and_then(|s| s.strip_suffix('A'))
                else {
                    continue;
                };
                if id.is_empty() || id.len() > 16 || !id.bytes().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                let document = get(&format!("{}.json", key.trim_start_matches('/')), &[]).await?;
                if let Some(name) = document["name"].as_str() {
                    work.author_names.push(name.into());
                }
            }
            // Never trust provider-payload cache fields supplied by an upstream work record.
            work.collections.clear();
            work.collections_incomplete = false;
            work.unavailable_collections.clear();
            match collections::memberships(external_id).await {
                Ok(found) => {
                    work.collections = found.collections;
                    work.unavailable_collections = found.unavailable;
                }
                Err(error) => {
                    work.collections_incomplete = true;
                    log::warn!("Open Library curated collections unavailable: {error}");
                }
            }
            let overview = work
                .description
                .as_str()
                .or_else(|| work.description["value"].as_str())
                .map(str::to_string);
            Ok(StoredMetadataSnapshot {
                provider_id: MetadataProviderId::OpenLibrary,
                external_id: external_id.into(),
                media_type: Some("book".into()),
                title: Some(work.title.clone()),
                overview,
                artwork_url: work.covers.iter().find_map(|id| cover(*id)),
                backdrop_url: None,
                release_year: work
                    .first_publish_date
                    .as_deref()
                    .and_then(|date| date.get(..4)?.parse().ok()),
                locale_key: String::new(),
                provider_locale_key: None,
                provider_payload_json: Some(
                    serde_json::to_string(&work).map_err(|e| e.to_string())?,
                ),
            })
        })
    }

    fn metadata_details(
        &self,
        snapshot: &StoredMetadataSnapshot,
    ) -> ProviderMetadataDetails {
        if snapshot.media_type.as_deref() == Some("author") {
            return authors::details(snapshot);
        }
        let work: Work = snapshot
            .provider_payload_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        ProviderMetadataDetails {
            collections_incomplete: work.collections_incomplete,
            unavailable_collection_ids: work.unavailable_collections,
            collections: work
                .collections
                .into_iter()
                .map(|collection| ProviderMetadataCollection {
                    external_id: collection.key,
                    name: Some(collection.title),
                    overview: None,
                    artwork_url: None,
                    backdrop_url: None,
                    theme_song_url: None,
                })
                .collect(),
            external_ids: vec![
                crate::metadata::ProviderExternalId {
                    source: "openlibrary".into(),
                    external_id: snapshot.external_id.clone(),
                },
            ],
            genres: work.subjects.into_iter().take(12).collect(),
            ..Default::default()
        }
    }
}

pub(crate) fn work_metadata(raw: &str) -> Option<crate::media::AudiobookMetadata> {
    let work: Work = serde_json::from_str(raw).ok()?;
    if !valid_work_id(work.key.trim_start_matches('/')) || work.title.trim().is_empty() {
        return None;
    }
    Some(crate::media::AudiobookMetadata {
        title: work.title,
        author: work.author_names.join(", "),
        openlibrary_work_id: Some(work.key.trim_start_matches('/').into()),
        ..Default::default()
    })
}
