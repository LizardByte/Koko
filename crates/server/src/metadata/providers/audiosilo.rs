//! AudioSilo's public CC0 work and recording metadata. Community prose is not imported.

#[cfg(test)]
pub(crate) mod tests;

use super::{
    MediaLibraryKind,
    MetadataProvider,
    MetadataProviderDescriptor,
    MetadataProviderFuture,
    MetadataProviderId,
    MetadataProviderRole,
    MetadataSearchResult,
    MetadataSettings,
    ProviderMetadataCollection,
    ProviderMetadataDetails,
    StoredMetadataSnapshot,
};
use serde::{
    Deserialize,
    Serialize,
};
use serde_json::Value;

pub(super) struct AudioSiloMetadataProvider;

#[cfg(test)]
tokio::task_local! { static TEST_API_ROOT: String; }

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct Person {
    id: String,
    name: String,
}
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct Series {
    id: String,
    name: String,
    position: String,
}
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct Recording {
    id: String,
    narrators: Vec<Person>,
    runtime_min: Option<i64>,
    release_date: Option<String>,
    publisher: Option<String>,
    abridged: Option<bool>,
    cover_url: Option<String>,
    asin: Vec<RecordingAsin>,
}
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct RecordingAsin {
    asin: String,
}
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct Work {
    id: String,
    title: String,
    authors: Vec<Person>,
    language: Option<String>,
    first_published: Option<String>,
    series: Vec<Series>,
    recordings: Option<Vec<Recording>>,
    #[serde(rename = "koko_recording")]
    recording: Option<Recording>,
    #[serde(rename = "koko_chapters")]
    chapters: Vec<crate::media::AudiobookChapter>,
}

fn valid_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

async fn get_optional(
    path: &str,
    query: &[(String, String)],
) -> Result<Option<Value>, String> {
    let root = "https://meta.audiosilo.app/api/v1".to_string();
    #[cfg(test)]
    let root = TEST_API_ROOT.try_with(Clone::clone).unwrap_or(root);
    let response = crate::metadata::http::client()
        .get(format!("{root}/{path}"))
        .query(query)
        .send()
        .await
        .map_err(|error| format!("AudioSilo request failed: {error}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    response
        .error_for_status()
        .map_err(|error| format!("AudioSilo response failed: {error}"))?
        .json()
        .await
        .map(Some)
        .map_err(|error| format!("Invalid AudioSilo response: {error}"))
}

async fn get(
    path: &str,
    query: &[(String, String)],
) -> Result<Value, String> {
    get_optional(path, query)
        .await?
        .ok_or_else(|| "AudioSilo record was not found".into())
}

fn names(people: &[Person]) -> String {
    people
        .iter()
        .map(|person| person.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
fn year(value: Option<&str>) -> Option<i32> {
    value?.get(..4)?.parse().ok()
}

fn recording_identity(
    work: &Work,
    recording: &Recording,
) -> crate::media::AudiobookRecording {
    crate::media::AudiobookRecording {
        provider_id: MetadataProviderId::AudioSilo,
        external_id: format!("{}/{}", work.id, recording.id),
        title: work.title.clone(),
        narrator: names(&recording.narrators),
        language: work.language.clone(),
        release_year: year(recording.release_date.as_deref()),
        abridged: recording.abridged,
        runtime_ms: recording
            .runtime_min
            .and_then(|minutes| minutes.checked_mul(60_000)),
        asins: recording.asin.iter().map(|id| id.asin.clone()).collect(),
        artwork_url: recording.cover_url.clone(),
    }
}

/// Return distinct narrations of matching works; never silently select the first one.
pub(crate) async fn recording_choices(
    metadata: &crate::media::AudiobookMetadata
) -> Result<Vec<crate::media::AudiobookRecording>, String> {
    let query = format!("{} {}", metadata.title, metadata.author);
    let response = get(
        "works/search",
        &[
            ("q".into(), query),
            ("limit".into(), "5".into()),
        ],
    )
    .await?;
    let mut choices = Vec::new();
    for result in response["results"]
        .as_array()
        .ok_or("AudioSilo results missing")?
        .iter()
        .take(5)
    {
        let Some(id) = result["id"].as_str().filter(|id| valid_slug(id)) else {
            continue;
        };
        let work: Work = serde_json::from_value(get(&format!("works/{id}"), &[]).await?)
            .map_err(|error| error.to_string())?;
        let normalize = |name: &str| {
            name.chars()
                .filter(|c| c.is_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect::<String>()
        };
        if !super::libex::work_title_matches(&metadata.title, &work.title, &metadata.author)
            || metadata.author.is_empty()
            || !work
                .authors
                .iter()
                .any(|author| normalize(&author.name) == normalize(&metadata.author))
        {
            continue;
        }
        if let Some(recordings) = &work.recordings {
            choices.extend(
                recordings
                    .iter()
                    .map(|recording| recording_identity(&work, recording)),
            );
        }
    }
    Ok(choices)
}

fn result(value: &Value) -> Option<MetadataSearchResult> {
    let id = value["id"].as_str()?;
    let title = value["title"].as_str()?.trim();
    if !valid_slug(id) || title.is_empty() {
        return None;
    }
    let authors =
        serde_json::from_value::<Vec<Person>>(value["authors"].clone()).unwrap_or_default();
    Some(MetadataSearchResult {
        provider_id: MetadataProviderId::AudioSilo,
        external_id: id.into(),
        media_type: "audiobook".into(),
        title: title.into(),
        overview: Some(names(&authors)),
        artwork_url: value["cover_url"].as_str().map(str::to_string),
        backdrop_url: None,
        release_year: year(value["release_date"].as_str()),
        score: value["score"].as_f64().map(|score| score / 100.0),
        track_count: None,
    })
}

async fn recording_results(
    index: usize,
    work: MetadataSearchResult,
) -> Result<(usize, Vec<MetadataSearchResult>), String> {
    let payload = get(&format!("works/{}", work.external_id), &[]).await?;
    let document: Work = serde_json::from_value(payload).map_err(|error| error.to_string())?;
    let mut results = vec![work.clone()];
    for recording in document
        .recordings
        .unwrap_or_default()
        .into_iter()
        .filter(|r| valid_slug(&r.id))
    {
        let narrator = names(&recording.narrators);
        let label = [
            narrator,
            recording.release_date.unwrap_or_default(),
            if recording.abridged == Some(true) { "Abridged".into() } else { String::new() },
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
        results.push(MetadataSearchResult {
            external_id: format!("{}/{}", work.external_id, recording.id),
            title: format!(
                "{} · {}",
                work.title,
                if label.is_empty() { recording.id.as_str() } else { label.as_str() }
            ),
            artwork_url: recording.cover_url.or_else(|| work.artwork_url.clone()),
            ..work.clone()
        });
    }
    Ok((index, results))
}

pub(crate) async fn guess(
    metadata: &crate::media::AudiobookMetadata,
    duration: Option<i64>,
) -> Result<Option<MetadataSearchResult>, String> {
    let mut query = vec![
        ("title".into(), metadata.title.clone()),
        ("limit".into(), "5".into()),
    ];
    if !metadata.author.is_empty() {
        query.push(("author".into(), metadata.author.clone()));
    }
    if let Some(series) = &metadata.series {
        query.push(("series".into(), series.clone()));
    }
    if let Some(position) = &metadata.series_position {
        query.push(("position".into(), position.clone()));
    }
    if let Some(duration) = duration.filter(|ms| *ms > 0) {
        query.push(("runtime".into(), (duration / 1000).to_string()));
    }
    let response = get("works/match", &query).await?;
    let matches = response["results"]
        .as_array()
        .ok_or("AudioSilo match results are missing")?;
    let Some(first) = matches
        .first()
        .filter(|value| value["score"].as_f64().unwrap_or(0.0) >= 95.0)
    else {
        return Ok(None);
    };
    // An exact title can identify a work, but cannot identify a narrator's recording.
    if matches
        .get(1)
        .is_some_and(|value| value["score"].as_f64().unwrap_or(0.0) >= 90.0)
    {
        return Ok(None);
    }
    Ok(result(first))
}

impl MetadataProvider for AudioSiloMetadataProvider {
    fn descriptor(&self) -> MetadataProviderDescriptor {
        MetadataProviderDescriptor {
            id: MetadataProviderId::AudioSilo,
            display_name: "AudioSilo Meta".into(),
            description: "Audiobook works, recordings, authors, narrators, series, covers, and \
                          chapters."
                .into(),
            supported_kinds: vec![MediaLibraryKind::Books],
            requires_api_key: false,
            implemented: true,
            role: MetadataProviderRole::Secondary,
            extends_provider_ids: vec![MetadataProviderId::Libex],
            attribution_text: "Factual metadata from AudioSilo Meta (CC0 1.0). Community \
                               descriptions and recaps are not imported."
                .into(),
            attribution_url: "https://meta.audiosilo.app/".into(),
            logo_light_url: Some("/assets/providers/audiosilo.svg".into()),
            logo_dark_url: Some("/assets/providers/audiosilo.svg".into()),
        }
    }

    fn secondary_metadata_reference_priority(
        &self,
        source: &MetadataProviderId,
        item_type: &str,
        database: &str,
    ) -> Option<usize> {
        (*source == MetadataProviderId::Libex && item_type == "audiobook" && database == "libex")
            .then_some(0)
    }

    fn fetch_secondary_metadata<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        item_type: &'a str,
        database: &'a str,
        external_id: &'a str,
        _locale: &'a str,
    ) -> MetadataProviderFuture<'a, Option<ProviderMetadataDetails>> {
        Box::pin(async move {
            if item_type == "audiobook" && database == "audiosilo" {
                let snapshot = self
                    .fetch_snapshot(settings, external_id, "audiobook", false)
                    .await?;
                let mut details = self.metadata_details(&snapshot);
                details.artwork_url = snapshot.artwork_url;
                details.provider_payload_json = snapshot.provider_payload_json;
                return Ok(Some(details));
            }
            if item_type != "audiobook" || database != "libex" {
                return Ok(None);
            }
            let parts = external_id.split('/').collect::<Vec<_>>();
            if !(parts.len() == 2 || (parts.len() == 3 && parts[2] == "recording"))
                || parts[1].len() != 10
                || !parts[1].bytes().all(|c| c.is_ascii_alphanumeric())
            {
                return Ok(None);
            }
            let Some(found) = get_optional("lookup", &[("asin".into(), parts[1].into())]).await?
            else {
                return Ok(None);
            };
            let Some(work) = found
                .pointer("/work/id")
                .and_then(Value::as_str)
                .filter(|id| valid_slug(id))
            else {
                return Ok(None);
            };
            let identity = if parts.len() == 3 {
                let Some(recording) = found["recording_id"].as_str().filter(|id| valid_slug(id))
                else {
                    return Ok(None);
                };
                format!("{work}/{recording}")
            } else {
                work.to_string()
            };
            let snapshot = self
                .fetch_snapshot(settings, &identity, "audiobook", false)
                .await?;
            let mut details = self.metadata_details(&snapshot);
            details.artwork_url = snapshot.artwork_url;
            details.provider_payload_json = snapshot.provider_payload_json;
            Ok(Some(details))
        })
    }

    fn search<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        query: &'a str,
        media_type: Option<&'a str>,
    ) -> MetadataProviderFuture<'a, Vec<MetadataSearchResult>> {
        Box::pin(async move {
            if media_type.is_some_and(|kind| kind != "audiobook") {
                return Ok(Vec::new());
            }
            let response = get(
                "works/search",
                &[
                    ("q".into(), query.into()),
                    ("limit".into(), "20".into()),
                ],
            )
            .await?;
            let works = response["results"]
                .as_array()
                .ok_or("AudioSilo search results are missing")?
                .iter()
                .filter_map(result)
                .take(5)
                .collect::<Vec<_>>();
            let mut tasks = tokio::task::JoinSet::new();
            for (index, work) in works.iter().cloned().enumerate() {
                #[cfg(test)]
                let root = TEST_API_ROOT.try_with(Clone::clone).ok();
                tasks.spawn(async move {
                    #[cfg(test)]
                    if let Some(root) = root {
                        return TEST_API_ROOT
                            .scope(root, recording_results(index, work))
                            .await;
                    }
                    recording_results(index, work).await
                });
            }
            let mut expanded = Vec::new();
            while let Some(result) = tasks.join_next().await {
                match result {
                    Ok(Ok(value)) => expanded.push(value),
                    Ok(Err(error)) => {
                        log::warn!("AudioSilo recording choices unavailable: {error}")
                    }
                    Err(error) => log::warn!("AudioSilo search worker failed: {error}"),
                }
            }
            expanded.sort_by_key(|(index, _)| *index);
            let expanded_ids = expanded
                .iter()
                .map(|(index, _)| *index)
                .collect::<std::collections::HashSet<_>>();
            let mut results = expanded
                .into_iter()
                .flat_map(|(_, results)| results)
                .collect::<Vec<_>>();
            results.extend(
                works
                    .into_iter()
                    .enumerate()
                    .filter(|(index, _)| !expanded_ids.contains(index))
                    .map(|(_, work)| work),
            );
            Ok(results)
        })
    }

    fn fetch_snapshot<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        external_id: &'a str,
        _media_type: &'a str,
        _people: bool,
    ) -> MetadataProviderFuture<'a, StoredMetadataSnapshot> {
        Box::pin(async move {
            let (work_id, recording_id) = external_id
                .split_once('/')
                .map_or((external_id, None), |(work, recording)| {
                    (work, Some(recording))
                });
            if !valid_slug(work_id) || recording_id.is_some_and(|id| !valid_slug(id)) {
                return Err("Invalid AudioSilo work or recording identifier".into());
            }
            let mut work: Work =
                serde_json::from_value(get(&format!("works/{work_id}"), &[]).await?)
                    .map_err(|error| format!("Invalid AudioSilo work: {error}"))?;
            if work.title.trim().is_empty() {
                return Err("AudioSilo work has no title".into());
            }
            work.recording = None;
            work.chapters.clear();
            if let Some(id) = recording_id {
                let recording = work
                    .recordings
                    .as_ref()
                    .and_then(|items| items.iter().find(|recording| recording.id == id))
                    .cloned()
                    .ok_or("AudioSilo recording does not belong to this work")?;
                let chapters =
                    get(&format!("works/{work_id}/recordings/{id}/chapters"), &[]).await?;
                work.chapters = chapters["chapters"]
                    .as_array()
                    .ok_or("AudioSilo chapters are missing")?
                    .iter()
                    .filter_map(|chapter| {
                        let start = chapter["start_ms"].as_i64()?;
                        let length = chapter["length_ms"].as_i64()?;
                        let end = start.checked_add(length)?;
                        (start >= 0 && length > 0).then(|| crate::media::AudiobookChapter {
                            title: chapter["title"].as_str().unwrap_or("Chapter").into(),
                            start_ms: start,
                            end_ms: end,
                        })
                    })
                    .collect();
                work.chapters.sort_by_key(|chapter| chapter.start_ms);
                work.recording = Some(recording);
            }
            let cover = work.recording.as_ref().and_then(|r| r.cover_url.clone());
            Ok(StoredMetadataSnapshot {
                provider_id: MetadataProviderId::AudioSilo,
                external_id: external_id.into(),
                media_type: Some("audiobook".into()),
                title: Some(work.title.clone()),
                overview: None,
                artwork_url: cover,
                backdrop_url: None,
                release_year: year(work.first_published.as_deref()),
                locale_key: String::new(),
                provider_locale_key: None,
                provider_payload_json: Some(
                    serde_json::to_string(&work).map_err(|error| error.to_string())?,
                ),
            })
        })
    }

    fn metadata_details(
        &self,
        snapshot: &StoredMetadataSnapshot,
    ) -> ProviderMetadataDetails {
        let work = snapshot
            .provider_payload_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Work>(raw).ok())
            .unwrap_or_default();
        ProviderMetadataDetails {
            collections: work
                .series
                .into_iter()
                .map(|series| ProviderMetadataCollection {
                    external_id: series.id,
                    name: Some(series.name),
                    overview: None,
                    artwork_url: None,
                    backdrop_url: None,
                    theme_song_url: None,
                })
                .collect(),
            ..Default::default()
        }
    }
}

/// Normalize only factual fields retained in our typed provider snapshot.
pub(crate) fn recording_metadata(raw: &str) -> Option<crate::media::AudiobookMetadata> {
    let work = serde_json::from_str::<Work>(raw).ok()?;
    let mut recording = work
        .recording
        .as_ref()
        .map(|recording| recording_identity(&work, recording));
    if let (Some(recording), Some(chapter)) = (recording.as_mut(), work.chapters.last()) {
        recording.runtime_ms = Some(chapter.end_ms);
    }
    Some(crate::media::AudiobookMetadata {
        recording,
        title: work.title,
        author: names(&work.authors),
        narrator: work
            .recording
            .as_ref()
            .map(|r| names(&r.narrators))
            .unwrap_or_default(),
        language: work.language,
        series: work.series.first().map(|s| s.name.clone()),
        series_position: work.series.first().map(|s| s.position.clone()),
        chapters: work.chapters,
        ..Default::default()
    })
}
