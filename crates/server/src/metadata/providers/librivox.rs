//! LibriVox projects identify individual volunteer recordings; section boundaries are file starts.

#[cfg(test)]
mod tests;

use super::*;
use crate::media::{
    AudiobookChapter,
    AudiobookMetadata,
    AudiobookRecording,
};
use serde::{
    Deserialize,
    Deserializer,
    Serialize,
};

pub(super) struct LibriVoxMetadataProvider;

/// Local file evidence used to confirm a project without relying on title similarity alone.
pub(crate) struct RecordingFile {
    pub path: String,
    pub duration_ms: Option<i64>,
}

#[cfg(test)]
tokio::task_local! { pub(crate) static TEST_API_ROOT: String; }

/// The API uses both JSON numbers and decimal strings for identifiers and durations.
fn text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::String(value) => value,
        serde_json::Value::Number(value) => value.to_string(),
        _ => String::new(),
    })
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct Author {
    first_name: String,
    last_name: String,
}
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct Reader {
    display_name: String,
}
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct Genre {
    name: String,
}
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct Section {
    #[serde(deserialize_with = "text")]
    section_number: String,
    title: String,
    listen_url: String,
    #[serde(deserialize_with = "text")]
    playtime: String,
    readers: Vec<Reader>,
}
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct Book {
    #[serde(deserialize_with = "text")]
    id: String,
    title: String,
    description: String,
    language: String,
    #[serde(deserialize_with = "text")]
    num_sections: String,
    #[serde(deserialize_with = "text")]
    totaltimesecs: String,
    authors: Vec<Author>,
    sections: Vec<Section>,
    genres: Vec<Genre>,
    coverart_jpg: Option<String>,
    #[serde(rename = "koko_recording_selected")]
    recording_selected: bool,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 12
        && id.bytes().all(|byte| byte.is_ascii_digit())
        && id.parse::<u64>().is_ok_and(|value| value > 0)
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn authors(book: &Book) -> String {
    book.authors
        .iter()
        .map(|author| {
            format!("{} {}", author.first_name.trim(), author.last_name.trim())
                .trim()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn narrators(book: &Book) -> String {
    let mut seen = std::collections::HashSet::new();
    book.sections
        .iter()
        .flat_map(|section| &section.readers)
        .map(|reader| reader.display_name.trim())
        .filter(|name| !name.is_empty() && seen.insert(normalize(name)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn duration(value: &str) -> Option<i64> {
    value
        .parse::<i64>()
        .ok()
        .filter(|seconds| *seconds > 0)?
        .checked_mul(1000)
}

fn cover(book: &Book) -> Option<String> {
    book.coverart_jpg
        .as_ref()
        .filter(|url| {
            reqwest::Url::parse(url).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
        })
        .cloned()
}

async fn books(query: &[(String, String)]) -> Result<Vec<Book>, String> {
    // Keep bulk library matching considerate of LibriVox's volunteer-run API.
    #[cfg(not(test))]
    {
        static LAST: once_cell::sync::Lazy<tokio::sync::Mutex<Option<tokio::time::Instant>>> =
            once_cell::sync::Lazy::new(|| tokio::sync::Mutex::new(None));
        let mut last = LAST.lock().await;
        if let Some(time) = *last {
            tokio::time::sleep_until(time + std::time::Duration::from_secs(1)).await;
        }
        *last = Some(tokio::time::Instant::now());
    }
    let root = "https://librivox.org/api/feed/audiobooks/".to_string();
    #[cfg(test)]
    let root = TEST_API_ROOT.try_with(Clone::clone).unwrap_or(root);
    let response = crate::metadata::http::client()
        .get(root)
        .query(query)
        .query(&[
            ("format", "json"),
            ("extended", "1"),
            ("coverart", "1"),
            ("limit", "50"),
        ])
        .send()
        .await
        .map_err(|error| format!("LibriVox request failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("LibriVox response failed: {error}"))?
        .json::<serde_json::Value>()
        .await
        .map_err(|error| format!("Invalid LibriVox response: {error}"))?;
    let Some(values) = response.get("books") else {
        if response["error"]
            .as_str()
            .is_some_and(|error| error.to_ascii_lowercase().contains("no results"))
        {
            return Ok(Vec::new());
        }
        return Err("LibriVox response has no books".into());
    };
    let mut books: Vec<Book> = serde_json::from_value(values.clone())
        .map_err(|error| format!("Invalid LibriVox books: {error}"))?;
    books.retain(|book| valid_id(&book.id) && !book.title.trim().is_empty());
    for book in &mut books {
        book.recording_selected = false;
        book.sections
            .sort_by_key(|section| section.section_number.parse::<u32>().unwrap_or(u32::MAX));
    }
    Ok(books)
}

fn search_result(
    book: &Book,
    recording: bool,
) -> MetadataSearchResult {
    MetadataSearchResult {
        provider_id: MetadataProviderId::LibriVox,
        external_id: format!("{}{}", book.id, if recording { "/recording" } else { "" }),
        media_type: "audiobook".into(),
        title: format!(
            "{} · {}",
            book.title,
            if recording { narrators(book) } else { "Book details only".into() }
        ),
        overview: Some(authors(book)),
        artwork_url: cover(book),
        backdrop_url: None,
        release_year: None,
        score: None,
        track_count: None,
    }
}

fn author_matches(
    book: &Book,
    author: &str,
) -> bool {
    !author.trim().is_empty()
        && (normalize(&authors(book)) == normalize(author)
            || book.authors.iter().any(|person| {
                normalize(&format!("{} {}", person.first_name, person.last_name))
                    == normalize(author)
            }))
}

async fn matching_books(metadata: &AudiobookMetadata) -> Result<Vec<Book>, String> {
    let mut found = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut succeeded = false;
    let mut last_error = None;
    for (title, author) in [
        (&metadata.title, &metadata.author),
        (&metadata.folder_title, &metadata.folder_author),
    ] {
        if title.trim().is_empty()
            || author.trim().is_empty()
            || !seen.insert((normalize(title), normalize(author)))
        {
            continue;
        }
        match books(&[("title".into(), regex::escape(title))]).await {
            Ok(books) => {
                succeeded = true;
                found.extend(books.into_iter().filter(|book| {
                    super::libex::work_title_matches(title, &book.title, author)
                        && author_matches(book, author)
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
    let mut ids = std::collections::HashSet::new();
    found.retain(|book| ids.insert(book.id.clone()));
    Ok(found)
}

fn identity(book: &Book) -> AudiobookRecording {
    AudiobookRecording {
        provider_id: MetadataProviderId::LibriVox,
        external_id: format!("{}/recording", book.id),
        title: book.title.clone(),
        narrator: narrators(book),
        language: Some(book.language.clone()).filter(|s| !s.is_empty()),
        release_year: None,
        abridged: None,
        runtime_ms: duration(&book.totaltimesecs),
        asins: Vec::new(),
        artwork_url: cover(book),
    }
}

/// Original LibriVox filenames identify sections across MP3 bitrate variants and folder moves.
fn file_key(path: &str) -> String {
    let path = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase();
    let path = path.as_str();
    let stem = path.rsplit_once('.').map_or(path, |(stem, _)| stem);
    let stem = stem
        .rsplit_once('_')
        .filter(|(_, quality)| {
            quality
                .strip_suffix("kb")
                .is_some_and(|value| value.bytes().all(|b| b.is_ascii_digit()))
        })
        .map_or(stem, |(stem, _)| stem);
    stem.to_ascii_lowercase()
}

fn section_files(book: &Book) -> Vec<String> {
    book.sections
        .iter()
        .map(|section| {
            reqwest::Url::parse(&section.listen_url)
                .ok()
                .and_then(|url| url.path_segments()?.next_back().map(str::to_string))
                .unwrap_or_default()
        })
        .collect()
}

fn files_match(
    book: &Book,
    parts: &[RecordingFile],
) -> bool {
    if book.sections.is_empty() || book.sections.len() != parts.len() {
        return false;
    }
    let files = section_files(book);
    let mut matched = std::collections::HashSet::new();
    book.sections.iter().zip(files).all(|(section, file)| {
        let matching = parts
            .iter()
            .filter(|part| !file.is_empty() && file_key(&part.path) == file_key(&file))
            .collect::<Vec<_>>();
        matching.len() == 1
            && matched.insert(file_key(&matching[0].path))
            && duration(&section.playtime)
                .zip(matching[0].duration_ms)
                .is_some_and(|(expected, actual)| (expected - actual).abs() <= 2000)
    })
}

fn recording_matches(
    book: &Book,
    local: &AudiobookMetadata,
    runtime: Option<i64>,
    parts: &[RecordingFile],
) -> bool {
    if parts.is_empty()
        || parts
            .iter()
            .any(|part| part.duration_ms.is_none_or(|duration| duration <= 0))
    {
        return false;
    }
    let runtime_matches =
        duration(&book.totaltimesecs)
            .zip(runtime)
            .is_some_and(|(expected, actual)| {
                (expected - actual).abs() <= 2000.max(book.sections.len() as i64 * 1000)
            });
    let narrator = narrators(book);
    if !local.narrator.trim().is_empty() && normalize(&local.narrator) != normalize(&narrator) {
        return false;
    }
    runtime_matches
        && (files_match(book, parts) || !local.narrator.trim().is_empty() && !narrator.is_empty())
}

/// Match a written title first; confirm a narration only with matching local recording evidence.
pub(crate) async fn guess(
    metadata: &AudiobookMetadata,
    runtime: Option<i64>,
    parts: &[RecordingFile],
) -> Result<Option<MetadataSearchResult>, String> {
    let books = matching_books(metadata).await?;
    let recordings = books
        .iter()
        .filter(|book| recording_matches(book, metadata, runtime, parts))
        .collect::<Vec<_>>();
    if recordings.len() == 1 {
        return Ok(Some(search_result(recordings[0], true)));
    }
    Ok(books.first().map(|book| search_result(book, false)))
}

/// Offer every project for the work without substituting the first narrator automatically.
pub(crate) async fn recording_choices(
    metadata: &AudiobookMetadata
) -> Result<Vec<AudiobookRecording>, String> {
    Ok(matching_books(metadata)
        .await?
        .iter()
        .map(identity)
        .collect())
}

/// Section timings describe file boundaries, including files containing multiple printed chapters.
fn chapters(book: &Book) -> Vec<AudiobookChapter> {
    let count = book.num_sections.parse::<usize>().ok();
    if book.sections.is_empty() || book.sections.len() > 10000 || count != Some(book.sections.len())
    {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut offset = 0_i64;
    for (index, section) in book.sections.iter().enumerate() {
        if section.section_number.parse::<usize>().ok() != Some(index + 1) {
            return Vec::new();
        }
        let Some(end) = duration(&section.playtime).and_then(|length| offset.checked_add(length))
        else {
            return Vec::new();
        };
        result.push(AudiobookChapter {
            title: section.title.clone(),
            start_ms: offset,
            end_ms: end,
        });
        offset = end;
    }
    if duration(&book.totaltimesecs)
        .is_some_and(|total| (total - offset).abs() > book.sections.len() as i64 * 1000)
    {
        return Vec::new();
    }
    result
}

/// Restore provider recording facts separately from embedded tags and the written work identity.
pub(crate) fn recording_metadata(raw: &str) -> Option<AudiobookMetadata> {
    let book: Book = serde_json::from_str(raw).ok()?;
    let chapters = if book.recording_selected { chapters(&book) } else { Vec::new() };
    Some(AudiobookMetadata {
        title: book.title.clone(),
        author: authors(&book),
        recording: book.recording_selected.then(|| identity(&book)),
        narrator: if book.recording_selected { narrators(&book) } else { String::new() },
        language: Some(book.language.clone()).filter(|value| !value.is_empty()),
        chapter_files: if chapters.is_empty() { Vec::new() } else { section_files(&book) },
        chapters,
        ..Default::default()
    })
}

impl MetadataProvider for LibriVoxMetadataProvider {
    fn descriptor(&self) -> MetadataProviderDescriptor {
        MetadataProviderDescriptor {
            id: MetadataProviderId::LibriVox,
            display_name: "LibriVox".into(),
            description: "Volunteer audiobook recordings, readers, covers, and section timings \
                          from LibriVox."
                .into(),
            supported_kinds: vec![MediaLibraryKind::Books],
            requires_api_key: false,
            implemented: true,
            role: MetadataProviderRole::Primary,
            extends_provider_ids: Vec::new(),
            attribution_text: "Recording metadata provided by LibriVox.".into(),
            attribution_url: "https://librivox.org/".into(),
            logo_light_url: Some("/assets/providers/librivox.png".into()),
            logo_dark_url: Some("/assets/providers/librivox.png".into()),
        }
    }

    fn search<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        query: &'a str,
        media_type: Option<&'a str>,
    ) -> MetadataProviderFuture<'a, Vec<MetadataSearchResult>> {
        self.search_with_artist(settings, query, media_type, None)
    }

    fn search_with_artist<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        query: &'a str,
        media_type: Option<&'a str>,
        author: Option<&'a str>,
    ) -> MetadataProviderFuture<'a, Vec<MetadataSearchResult>> {
        Box::pin(async move {
            if query.trim().is_empty() || media_type.is_some_and(|kind| kind != "audiobook") {
                return Ok(Vec::new());
            }
            Ok(books(&[("title".into(), regex::escape(query.trim()))])
                .await?
                .iter()
                .filter(|book| {
                    author.is_none_or(|author| {
                        author.trim().is_empty() || author_matches(book, author)
                    })
                })
                .flat_map(|book| {
                    [
                        search_result(book, false),
                        search_result(book, true),
                    ]
                })
                .collect())
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
            let id = external_id
                .strip_suffix("/recording")
                .unwrap_or(external_id);
            if media_type != "audiobook" || !valid_id(id) {
                return Err("Invalid LibriVox recording identifier".into());
            }
            let mut book = books(&[("id".into(), id.into())])
                .await?
                .into_iter()
                .find(|book| book.id == id)
                .ok_or("LibriVox project was not found")?;
            book.recording_selected = external_id.ends_with("/recording");
            Ok(StoredMetadataSnapshot {
                provider_id: MetadataProviderId::LibriVox,
                external_id: external_id.into(),
                media_type: Some("audiobook".into()),
                title: Some(book.title.clone()),
                overview: book
                    .recording_selected
                    .then(|| book.description.clone())
                    .filter(|s| !s.is_empty()),
                artwork_url: book.recording_selected.then(|| cover(&book)).flatten(),
                backdrop_url: None,
                release_year: None,
                locale_key: String::new(),
                provider_locale_key: None,
                provider_payload_json: Some(
                    serde_json::to_string(&book).map_err(|error| error.to_string())?,
                ),
            })
        })
    }

    fn metadata_details(
        &self,
        snapshot: &StoredMetadataSnapshot,
    ) -> ProviderMetadataDetails {
        let book = snapshot
            .provider_payload_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Book>(raw).ok())
            .unwrap_or_default();
        ProviderMetadataDetails {
            genres: book
                .genres
                .into_iter()
                .map(|genre| genre.name)
                .filter(|name| !name.is_empty())
                .collect(),
            ..Default::default()
        }
    }
}
