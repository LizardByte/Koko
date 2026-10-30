//! Libex's public Audible catalog, with separate work-only and explicit recording matches.

#[cfg(test)]
mod tests;

use super::{
    MediaLibraryKind,
    MetadataProvider,
    MetadataProviderDescriptor,
    MetadataProviderFuture,
    MetadataProviderId,
    MetadataProviderRole,
    MetadataSearchResult,
    MetadataSettings,
    StoredMetadataSnapshot,
};
use crate::media::{
    AudiobookChapter,
    AudiobookMetadata,
    AudiobookRecording,
};
use serde::{
    Deserialize,
    Serialize,
};

pub(super) struct LibexMetadataProvider;

#[cfg(test)]
tokio::task_local! { pub(crate) static TEST_API_ROOT: String; }

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct Person {
    name: String,
}
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct Series {
    asin: Option<String>,
    name: Option<String>,
    position: Option<String>,
}
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
struct Book {
    asin: String,
    region: String,
    title: Option<String>,
    description: Option<String>,
    summary: Option<String>,
    authors: Vec<Person>,
    narrators: Vec<Person>,
    language: Option<String>,
    release_date: Option<String>,
    image_url: Option<String>,
    length_minutes: Option<i64>,
    book_format: Option<String>,
    series: Vec<Series>,
    #[serde(rename = "koko_recording_selected")]
    recording_selected: bool,
    #[serde(rename = "koko_chapters")]
    chapters: Vec<AudiobookChapter>,
}

fn names(people: &[Person]) -> String {
    people
        .iter()
        .map(|p| p.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
fn matching_name(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Compare work titles without an author's possessive prefix or publisher subtitle.
pub(crate) fn work_title_matches(
    local: &str,
    candidate: &str,
    author: &str,
) -> bool {
    let normalize = |title: &str| {
        let main = title.split([':', '–', '—']).next().unwrap_or(title);
        let title = matching_name(main);
        let prefix = format!("{}s", matching_name(author));
        title
            .strip_prefix(&prefix)
            .filter(|suffix| !suffix.is_empty())
            .unwrap_or(&title)
            .to_string()
    };
    !local.trim().is_empty() && normalize(local) == normalize(candidate)
}
fn valid_asin(value: &str) -> bool {
    value.len() == 10 && value.bytes().all(|c| c.is_ascii_alphanumeric())
}
fn valid_region(value: &str) -> bool {
    [
        "us", "uk", "au", "ca", "de", "fr", "es", "it", "jp", "in", "br",
    ]
    .contains(&value)
}

fn region(settings: &MetadataSettings) -> String {
    let locale = settings
        .providers
        .iter()
        .find(|p| p.id == MetadataProviderId::Libex)
        .map(|p| p.language.as_str())
        .unwrap_or("en-US");
    let country = locale
        .rsplit(['-', '_'])
        .next()
        .unwrap_or("us")
        .to_ascii_lowercase();
    match country.as_str() {
        "gb" => "uk".into(),
        country if valid_region(country) => country.into(),
        _ => "us".into(),
    }
}

async fn get(
    path: &str,
    query: &[(String, String)],
) -> Result<Option<serde_json::Value>, String> {
    let root = "https://libexdb.com".to_string();
    #[cfg(test)]
    let root = TEST_API_ROOT.try_with(Clone::clone).unwrap_or(root);
    let response = crate::metadata::http::client()
        .get(format!("{root}/{path}"))
        .query(query)
        .send()
        .await
        .map_err(|error| format!("Libex request failed: {error}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    response
        .error_for_status()
        .map_err(|error| format!("Libex response failed: {error}"))?
        .json()
        .await
        .map(Some)
        .map_err(|error| format!("Invalid Libex response: {error}"))
}

fn search_result(
    book: &Book,
    recording: bool,
) -> Option<MetadataSearchResult> {
    let title = book.title.as_deref()?.trim();
    if !valid_asin(&book.asin) || !valid_region(&book.region) || title.is_empty() {
        return None;
    }
    let credit = names(&book.narrators);
    let label = if recording {
        [
            credit,
            book.release_date
                .as_deref()
                .and_then(|date| date.get(..4))
                .unwrap_or_default()
                .into(),
            book.book_format.clone().unwrap_or_default(),
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
    } else {
        "Book details only".into()
    };
    Some(MetadataSearchResult {
        provider_id: MetadataProviderId::Libex,
        external_id: format!(
            "{}/{}{}",
            book.region,
            book.asin,
            if recording { "/recording" } else { "" }
        ),
        media_type: "audiobook".into(),
        title: format!("{title} · {label}"),
        overview: Some(names(&book.authors)),
        artwork_url: book.image_url.clone(),
        backdrop_url: None,
        release_year: recording
            .then(|| book.release_date.as_deref()?.get(..4)?.parse().ok())
            .flatten(),
        score: None,
        track_count: None,
    })
}

async fn search_books(
    settings: &MetadataSettings,
    title: &str,
    author: Option<&str>,
) -> Result<Vec<Book>, String> {
    let mut query = vec![
        ("title".into(), title.into()),
        ("region".into(), region(settings)),
        ("limit".into(), "10".into()),
    ];
    if let Some(author) = author.filter(|value| !value.trim().is_empty()) {
        query.push(("author".into(), author.into()));
    }
    let Some(payload) = get("search", &query).await? else {
        return Ok(Vec::new());
    };
    serde_json::from_value(payload)
        .map_err(|error| format!("Invalid Libex search results: {error}"))
}

pub(crate) async fn guess(
    settings: &MetadataSettings,
    metadata: &AudiobookMetadata,
) -> Result<Option<MetadataSearchResult>, String> {
    let books = matching_books(settings, metadata).await?;
    // A work match does not establish which local narration is present.
    Ok(books.first().and_then(|book| search_result(book, false)))
}

async fn matching_books(
    settings: &MetadataSettings,
    metadata: &AudiobookMetadata,
) -> Result<Vec<Book>, String> {
    let mut seen = std::collections::HashSet::new();
    let mut books = Vec::new();
    let mut last_error = None;
    let mut succeeded = false;
    for (title, author) in [
        (&metadata.title, &metadata.author),
        (&metadata.folder_title, &metadata.folder_author),
    ] {
        if title.is_empty()
            || author.is_empty()
            || !seen.insert((matching_name(title), matching_name(author)))
        {
            continue;
        }
        match search_books(settings, title, Some(author)).await {
            Ok(candidates) => {
                succeeded = true;
                books.extend(candidates.into_iter().filter(|book| {
                    book.title
                        .as_deref()
                        .is_some_and(|name| work_title_matches(title, name, author))
                        && book
                            .authors
                            .iter()
                            .any(|person| matching_name(&person.name) == matching_name(author))
                }));
            }
            Err(error) => last_error = Some(error),
        }
    }
    if !succeeded {
        if let Some(error) = last_error {
            return Err(error);
        }
    }
    let mut identities = std::collections::HashSet::new();
    books.retain(|book| identities.insert((book.region.clone(), book.asin.clone())));
    Ok(books)
}

fn recording(book: &Book) -> Option<AudiobookRecording> {
    Some(AudiobookRecording {
        provider_id: MetadataProviderId::Libex,
        external_id: format!("{}/{}/recording", book.region, book.asin),
        title: book.title.clone()?,
        narrator: names(&book.narrators),
        language: book.language.clone(),
        release_year: book
            .release_date
            .as_deref()
            .and_then(|value| value.get(..4)?.parse().ok()),
        abridged: book.book_format.as_deref().and_then(|value| {
            match value.to_ascii_lowercase().as_str() {
                "abridged" => Some(true),
                "unabridged" => Some(false),
                _ => None,
            }
        }),
        runtime_ms: book
            .length_minutes
            .and_then(|value| value.checked_mul(60_000)),
        asins: vec![book.asin.clone()],
        artwork_url: book.image_url.clone(),
    })
}

pub(crate) async fn recording_choices(
    settings: &MetadataSettings,
    metadata: &AudiobookMetadata,
) -> Result<Vec<AudiobookRecording>, String> {
    Ok(matching_books(settings, metadata)
        .await?
        .iter()
        .filter_map(recording)
        .collect())
}

fn flatten_chapters(
    values: &[serde_json::Value],
    depth: usize,
    chapters: &mut Vec<AudiobookChapter>,
) {
    if depth > 16 || chapters.len() >= 10_000 {
        return;
    }
    for value in values.iter().take(10_000 - chapters.len()) {
        if chapters.len() >= 10_000 {
            break;
        }
        if let Some(children) = value["chapters"]
            .as_array()
            .filter(|children| !children.is_empty())
        {
            flatten_chapters(children, depth + 1, chapters);
            continue;
        }
        let Some(start) = value["startOffsetMs"].as_i64().filter(|start| *start >= 0) else {
            continue;
        };
        let Some(end) = value["lengthMs"]
            .as_i64()
            .filter(|length| *length > 0)
            .and_then(|length| start.checked_add(length))
        else {
            continue;
        };
        chapters.push(AudiobookChapter {
            title: value["title"].as_str().unwrap_or("Chapter").into(),
            start_ms: start,
            end_ms: end,
        });
    }
}

impl MetadataProvider for LibexMetadataProvider {
    fn descriptor(&self) -> MetadataProviderDescriptor {
        MetadataProviderDescriptor {
            id: MetadataProviderId::Libex,
            display_name: "LibexDB".into(),
            description: "Book and audiobook recording metadata from Libex's Audible catalog."
                .into(),
            supported_kinds: vec![MediaLibraryKind::Books],
            requires_api_key: false,
            implemented: true,
            role: MetadataProviderRole::Primary,
            extends_provider_ids: Vec::new(),
            attribution_text: "Metadata provided by Libex, sourced from Audible. Libex is \
                               independent of Audible and Amazon."
                .into(),
            attribution_url: "https://libexdb.com/".into(),
            logo_light_url: Some("/assets/providers/libex-light.png".into()),
            logo_dark_url: Some("/assets/providers/libex-dark.png".into()),
        }
    }

    fn search<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        query: &'a str,
        media_type: Option<&'a str>,
    ) -> MetadataProviderFuture<'a, Vec<MetadataSearchResult>> {
        Box::pin(async move {
            if media_type.is_some_and(|kind| kind != "audiobook") {
                return Ok(Vec::new());
            }
            Ok(search_books(settings, query, None)
                .await?
                .iter()
                .flat_map(|book| {
                    [
                        search_result(book, false),
                        search_result(book, true),
                    ]
                })
                .flatten()
                .collect())
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
            let parts = external_id.split('/').collect::<Vec<_>>();
            if !(parts.len() == 2 || parts.len() == 3 && parts[2] == "recording")
                || !valid_region(parts[0])
                || !valid_asin(parts[1])
            {
                return Err("Invalid Libex region or ASIN".into());
            }
            let query = vec![
                ("region".into(), parts[0].into()),
                (
                    "cache".into(),
                    (!crate::metadata::cache_policy::bypassed()).to_string(),
                ),
            ];
            let raw = get(&format!("book/{}", parts[1]), &query)
                .await?
                .ok_or("Libex book was not found")?;
            let mut book: Book = serde_json::from_value(raw)
                .map_err(|error| format!("Invalid Libex book: {error}"))?;
            if book.asin != parts[1]
                || book.region != parts[0]
                || book
                    .title
                    .as_deref()
                    .is_none_or(|title| title.trim().is_empty())
            {
                return Err("Libex returned a different or incomplete book".into());
            }
            book.recording_selected = parts.len() == 3;
            book.chapters.clear();
            if book.recording_selected {
                // Missing chapter data is optional and must not prevent book metadata loading.
                match get(
                    &format!("book/{}/chapters", parts[1]),
                    &[("region".into(), parts[0].into())],
                )
                .await
                {
                    Ok(Some(raw)) if raw["isAccurate"] == true => {
                        if let Some(chapters) = raw["chapters"].as_array() {
                            flatten_chapters(chapters, 0, &mut book.chapters);
                        }
                        book.chapters.sort_by_key(|chapter| chapter.start_ms);
                        book.chapters.dedup_by_key(|chapter| chapter.start_ms);
                    }
                    Err(error) => log::warn!("Optional Libex chapters unavailable: {error}"),
                    _ => {}
                }
            }
            Ok(StoredMetadataSnapshot {
                provider_id: MetadataProviderId::Libex,
                external_id: external_id.into(),
                media_type: Some("audiobook".into()),
                title: book.title.clone(),
                overview: book
                    .recording_selected
                    .then(|| book.description.clone().or_else(|| book.summary.clone()))
                    .flatten(),
                artwork_url: book
                    .recording_selected
                    .then(|| book.image_url.clone())
                    .flatten(),
                backdrop_url: None,
                release_year: book
                    .recording_selected
                    .then(|| book.release_date.as_deref()?.get(..4)?.parse().ok())
                    .flatten(),
                locale_key: String::new(),
                provider_locale_key: None,
                provider_payload_json: Some(
                    serde_json::to_string(&book).map_err(|error| error.to_string())?,
                ),
            })
        })
    }

    fn search_with_artist<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        query: &'a str,
        media_type: Option<&'a str>,
        author: Option<&'a str>,
    ) -> MetadataProviderFuture<'a, Vec<MetadataSearchResult>> {
        Box::pin(async move {
            if media_type.is_some_and(|kind| kind != "audiobook") {
                return Ok(Vec::new());
            }
            Ok(search_books(settings, query, author)
                .await?
                .iter()
                .flat_map(|book| {
                    [
                        search_result(book, false),
                        search_result(book, true),
                    ]
                })
                .flatten()
                .collect())
        })
    }
}

pub(crate) fn recording_metadata(raw: &str) -> Option<AudiobookMetadata> {
    let book: Book = serde_json::from_str(raw).ok()?;
    Some(AudiobookMetadata {
        recording: book.recording_selected.then(|| recording(&book)).flatten(),
        title: book.title.unwrap_or_default(),
        author: names(&book.authors),
        narrator: if book.recording_selected { names(&book.narrators) } else { String::new() },
        language: book.language,
        series: book.series.first().and_then(|series| series.name.clone()),
        series_position: book
            .series
            .first()
            .and_then(|series| series.position.clone()),
        chapters: book.chapters,
        ..Default::default()
    })
}
