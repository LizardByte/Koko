//! Provider similarity snapshots, fetched with metadata and retained independently of local files.

use super::{
    MetadataProviderId,
    MetadataSettings,
    StoredMetadataSnapshot,
};
use crate::db::schema::item_similarities as rows;
use diesel::prelude::*;
use once_cell::sync::Lazy;
use serde::{
    Deserialize,
    Serialize,
};
use serde_json::Value;
use std::{
    collections::HashSet,
    sync::Arc,
    time::Duration,
};

const PAYLOAD_KEY: &str = "koko_similar_items";
const SIMILARITY_TIMEOUT: Duration = Duration::from_secs(15);
const TMDB_PAGE_CONCURRENCY: usize = 4;
const INSERT_BATCH_SIZE: usize = 128;
static TMDB_REQUESTS: Lazy<tokio::sync::Semaphore> =
    Lazy::new(|| tokio::sync::Semaphore::new(TMDB_PAGE_CONCURRENCY));
static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(concat!(
            "Koko/",
            env!("CARGO_PKG_VERSION"),
            " (https://github.com/LizardByte/Koko)"
        ))
        .build()
        .expect("similar items HTTP client")
});

#[cfg(test)]
tokio::task_local! {
    pub(crate) static TEST_TMDB_ROOT: String;
    pub(crate) static TEST_LISTENBRAINZ_ROOT: String;
}

/// A provider identity, without any requirement that the item exists in Koko.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub(crate) struct Identity {
    pub(crate) provider_id: String,
    pub(crate) media_type: String,
    pub(crate) external_id: String,
}

impl Identity {
    pub(crate) fn new(
        provider: &str,
        kind: &str,
        id: &str,
    ) -> Self {
        Self {
            provider_id: provider.into(),
            media_type: kind.into(),
            external_id: id.into(),
        }
    }
}

/// One ranked result from the provider's complete similarity response.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct SimilarItem {
    #[serde(flatten)]
    identity: Identity,
    rank: i32,
}

/// Add optional discovery data to a refresh; a failed lookup preserves the previous saved list.
pub(crate) async fn enrich_snapshot(
    settings: &MetadataSettings,
    snapshot: &mut StoredMetadataSnapshot,
) {
    enrich_snapshot_with_timeout(settings, snapshot, SIMILARITY_TIMEOUT).await;
}

async fn enrich_snapshot_with_timeout(
    settings: &MetadataSettings,
    snapshot: &mut StoredMetadataSnapshot,
    timeout: Duration,
) {
    // Unit tests opt into discovery traffic with their own local HTTP fixtures.
    #[cfg(test)]
    if match snapshot.provider_id {
        MetadataProviderId::Tmdb => TEST_TMDB_ROOT.try_with(|_| ()).is_err(),
        MetadataProviderId::MusicBrainz => TEST_LISTENBRAINZ_ROOT.try_with(|_| ()).is_err(),
        _ => true,
    } {
        return;
    }
    if !matches!(
        snapshot.provider_id,
        MetadataProviderId::Tmdb | MetadataProviderId::MusicBrainz
    ) {
        return;
    }
    let Some(mut payload) = snapshot
        .provider_payload_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
    else {
        return;
    };
    let fetch = async {
        match snapshot.provider_id {
            MetadataProviderId::Tmdb => {
                tmdb(
                    settings,
                    &snapshot.external_id,
                    snapshot.media_type.as_deref().unwrap_or_default(),
                )
                .await
            }
            MetadataProviderId::MusicBrainz => {
                music(
                    &snapshot.external_id,
                    snapshot.media_type.as_deref().unwrap_or_default(),
                    &payload,
                )
                .await
            }
            _ => unreachable!("similarity provider checked above"),
        }
    };
    let result = tokio::time::timeout(timeout, fetch)
        .await
        .unwrap_or_else(|_| Err("Similarity lookup exceeded its time budget".into()));
    match result {
        Ok(items) => {
            if let Some(object) = payload.as_object_mut() {
                object.insert(
                    PAYLOAD_KEY.into(),
                    serde_json::to_value(items).expect("similar item identities"),
                );
                snapshot.provider_payload_json = Some(payload.to_string());
            }
        }
        Err(error) => log::debug!(
            "Similarity refresh for {}:{} was incomplete: {error}",
            snapshot.provider_id.as_storage_value(),
            snapshot.external_id
        ),
    }
}

fn root(
    default: &str,
    tmdb: bool,
) -> String {
    #[cfg(test)]
    if let Ok(value) = if tmdb {
        TEST_TMDB_ROOT.try_with(Clone::clone)
    } else {
        TEST_LISTENBRAINZ_ROOT.try_with(Clone::clone)
    } {
        return value;
    }
    let _ = tmdb;
    default.into()
}

#[derive(Deserialize)]
struct TmdbPage {
    results: Vec<TmdbId>,
    total_pages: u32,
}

#[derive(Deserialize)]
struct TmdbId {
    id: i64,
}

struct TmdbRequest {
    base: String,
    id: String,
    kind: String,
    key: String,
    language: String,
}

impl TmdbRequest {
    async fn page(
        &self,
        page: u32,
    ) -> Result<TmdbPage, String> {
        let _permit = TMDB_REQUESTS
            .acquire()
            .await
            .map_err(|_| "TMDB discovery stopped")?;
        CLIENT
            .get(format!(
                "{}/{}/{}/recommendations",
                self.base, self.kind, self.id
            ))
            .query(&[
                ("api_key", self.key.as_str()),
                ("language", self.language.as_str()),
                ("page", page.to_string().as_str()),
            ])
            .send()
            .await
            .map_err(|_| "TMDB similarity request failed")?
            .error_for_status()
            .map_err(|_| "TMDB similarity request was rejected")?
            .json::<TmdbPage>()
            .await
            .map_err(|_| "Invalid TMDB similarity response".into())
    }
}

async fn tmdb_pages(request: TmdbRequest) -> Result<Vec<String>, String> {
    let request = Arc::new(request);
    let first = request.page(1).await?;
    // Fetch the complete recommendations list, without crawling the much broader /similar search.
    // Keep page order even when concurrent requests finish out of order.
    let last_page = first.total_pages;
    let mut pages = vec![(1, first.results)];
    let mut pending = tokio::task::JoinSet::new();
    let mut next_page = 2;
    while next_page <= last_page || !pending.is_empty() {
        while next_page <= last_page && pending.len() < TMDB_PAGE_CONCURRENCY {
            let request = Arc::clone(&request);
            let page = next_page;
            pending.spawn(async move {
                request
                    .page(page)
                    .await
                    .map(|response| (page, response.results))
            });
            next_page += 1;
        }
        if let Some(page) = pending.join_next().await {
            pages.push(page.map_err(|_| "TMDB recommendation task failed")??);
        }
    }
    pages.sort_by_key(|(page, _)| *page);
    Ok(pages
        .into_iter()
        .flat_map(|(_, items)| items)
        .filter(|item| item.id > 0)
        .map(|item| item.id.to_string())
        .collect())
}

async fn tmdb(
    settings: &MetadataSettings,
    id: &str,
    kind: &str,
) -> Result<Vec<SimilarItem>, String> {
    if !["movie", "tv"].contains(&kind) {
        return Ok(Vec::new());
    }
    if id.parse::<i64>().ok().filter(|id| *id > 0).is_none() {
        return Err("Invalid TMDB identity".into());
    }
    let provider = super::provider_settings(settings, MetadataProviderId::Tmdb)?;
    let recommended = tmdb_pages(TmdbRequest {
        base: root("https://api.themoviedb.org/3", true),
        id: id.into(),
        kind: kind.into(),
        key: provider.api_key.ok_or("Missing TMDB API key")?,
        language: provider.language,
    })
    .await?;
    let mut seen = HashSet::new();
    Ok(recommended
        .into_iter()
        .filter(|candidate| candidate != id && seen.insert(candidate.clone()))
        .enumerate()
        .map(|(rank, id)| SimilarItem {
            identity: Identity::new("tmdb", kind, &id),
            rank: rank as i32,
        })
        .collect())
}

async fn music(
    id: &str,
    kind: &str,
    payload: &Value,
) -> Result<Vec<SimilarItem>, String> {
    use super::listenbrainz::{
        ARTIST_ALGORITHM,
        RECORDING_ALGORITHM,
        provider_json,
    };
    let base = root("https://labs.api.listenbrainz.org", false);
    let mut queries = Vec::new();
    if kind == "recording" {
        queries.push(("recording", id.to_string()));
    }
    if kind == "artist" {
        queries.push(("artist", id.to_string()));
    } else if let Some(credits) = payload["artist-credit"].as_array() {
        for artist in credits
            .iter()
            .filter_map(|credit| credit["artist"]["id"].as_str())
        {
            if crate::media::valid_mbid(artist) {
                queries.push(("artist", artist.into()));
            }
        }
    }
    let mut seen_queries = HashSet::new();
    let mut seen_results = HashSet::new();
    let mut items = Vec::new();
    for (entity, id) in queries
        .into_iter()
        .filter(|query| seen_queries.insert(query.clone()))
    {
        let (endpoint, parameter, algorithm, field) = if entity == "recording" {
            (
                "similar-recordings",
                "recording_mbids",
                RECORDING_ALGORITHM,
                "recording_mbid",
            )
        } else {
            (
                "similar-artists",
                "artist_mbids",
                ARTIST_ALGORITHM,
                "artist_mbid",
            )
        };
        let response = provider_json(
            &format!("{base}/{endpoint}/json"),
            &[
                (parameter, id),
                ("algorithm", algorithm.into()),
            ],
        )
        .await
        .map_err(|_| "ListenBrainz similarity request failed")?
        .ok_or("ListenBrainz similarities unavailable")?;
        let values = response
            .as_array()
            .ok_or("Invalid ListenBrainz similarity response")?;
        for (rank, value) in values.iter().enumerate() {
            let Some(id) = value[field]
                .as_str()
                .filter(|id| crate::media::valid_mbid(id))
            else {
                continue;
            };
            let identity = Identity::new("musicbrainz", entity, id);
            if seen_results.insert(identity.clone()) {
                items.push(SimilarItem {
                    identity,
                    rank: rank as i32,
                });
            }
        }
    }
    Ok(items)
}

/// Replace a complete saved response atomically; transient failures keep results for the same
/// identity.
pub(crate) fn sync(
    conn: &mut SqliteConnection,
    link_id: i32,
    snapshot: &StoredMetadataSnapshot,
    identity_changed: bool,
) -> QueryResult<()> {
    let payload = snapshot
        .provider_payload_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok());
    let items = payload
        .as_ref()
        .and_then(|value| value.get(PAYLOAD_KEY))
        .and_then(|value| serde_json::from_value::<Vec<SimilarItem>>(value.clone()).ok());
    if items.is_none() && !identity_changed {
        return Ok(());
    }
    diesel::delete(rows::table.filter(rows::metadata_link_id.eq(link_id))).execute(conn)?;
    let values: Vec<_> = items
        .unwrap_or_default()
        .into_iter()
        .filter(|item| item.rank >= 0)
        .map(|item| {
            (
                rows::metadata_link_id.eq(link_id),
                rows::provider_id.eq(item.identity.provider_id),
                rows::media_type.eq(item.identity.media_type),
                rows::external_id.eq(item.identity.external_id),
                rows::rank.eq(item.rank),
            )
        })
        .collect();
    // Five bound values per row stay below SQLite's traditional 999-variable limit.
    for batch in values.chunks(INSERT_BATCH_SIZE) {
        diesel::insert_into(rows::table)
            .values(batch)
            .on_conflict_do_nothing()
            .execute(conn)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
