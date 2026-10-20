//! Author search and profiles use Open Library author identities rather than book work IDs.

use super::*;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct Author {
    key: String,
    name: String,
    bio: Value,
    photos: Vec<i64>,
    alternate_names: Vec<String>,
    birth_date: Option<String>,
    death_date: Option<String>,
    top_work: Option<String>,
    work_count: Option<u64>,
}

fn valid_id(id: &str) -> bool {
    id.strip_prefix("authors/OL")
        .and_then(|id| id.strip_suffix('A'))
        .is_some_and(|id| {
            !id.is_empty() && id.len() <= 16 && id.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn canonical_id(key: &str) -> Option<String> {
    let key = key.trim_start_matches('/');
    let id = if key.starts_with("authors/") { key.into() } else { format!("authors/{key}") };
    valid_id(&id).then_some(id)
}

fn text(value: &Value) -> Option<String> {
    value
        .as_str()
        .or_else(|| value["value"].as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn lifespan(author: &Author) -> Option<String> {
    let parts = [
        ("Born", &author.birth_date),
        ("Died", &author.death_date),
    ]
    .into_iter()
    .filter_map(|(label, date)| {
        date.as_deref()
            .map(str::trim)
            .filter(|date| !date.is_empty())
            .map(|date| format!("{label} {date}"))
    })
    .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn portrait(author: &Author) -> Option<String> {
    author
        .photos
        .iter()
        .find(|id| **id > 0)
        .map(|id| format!("https://covers.openlibrary.org/a/id/{id}-L.jpg?default=false"))
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct SearchResponse {
    docs: Vec<Author>,
    #[serde(rename = "numFound", alias = "num_found")]
    count: Option<usize>,
}

async fn documents(query: &str) -> Result<SearchResponse, String> {
    serde_json::from_value(
        get(
            "search/authors.json",
            &[
                ("q".into(), query.into()),
                ("limit".into(), "10".into()),
            ],
        )
        .await?,
    )
    .map_err(|error| format!("Invalid Open Library author search: {error}"))
}

fn result(author: &Author) -> Option<MetadataSearchResult> {
    let id = canonical_id(&author.key)?;
    let name = author.name.trim();
    if name.is_empty() {
        return None;
    }
    let olid = id.strip_prefix("authors/")?;
    Some(MetadataSearchResult {
        provider_id: MetadataProviderId::OpenLibrary,
        external_id: id.clone(),
        media_type: "author".into(),
        title: name.into(),
        overview: Some(
            [
                lifespan(author),
                author
                    .top_work
                    .as_deref()
                    .map(str::trim)
                    .filter(|title| !title.is_empty())
                    .map(|title| format!("Known for: {title}")),
                author.work_count.map(|count| {
                    format!(
                        "{count} {} listed",
                        if count == 1 { "work" } else { "works" }
                    )
                }),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · "),
        )
        .filter(|value| !value.is_empty()),
        artwork_url: portrait(author).or_else(|| {
            Some(format!(
                "https://covers.openlibrary.org/a/olid/{olid}-L.jpg?default=false"
            ))
        }),
        backdrop_url: None,
        release_year: None,
        score: None,
        track_count: None,
    })
}

pub(super) async fn search(query: &str) -> Result<Vec<MetadataSearchResult>, String> {
    Ok(documents(query)
        .await?
        .docs
        .iter()
        .filter_map(result)
        .collect())
}

/// Auto-match only one exact name or documented alias; never choose between namesakes.
pub(crate) async fn guess_author(name: &str) -> Result<Option<MetadataSearchResult>, String> {
    let normalize = |name: &str| {
        name.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    let wanted = normalize(name);
    if wanted.is_empty() || name == "Unknown author" {
        return Ok(None);
    }
    let response = documents(name).await?;
    // A truncated result set cannot prove that a name is unique.
    if response
        .count
        .is_some_and(|count| count > response.docs.len())
    {
        return Ok(None);
    }
    let mut matches = std::collections::HashMap::new();
    for author in &response.docs {
        if normalize(&author.name) == wanted
            || author
                .alternate_names
                .iter()
                .any(|alias| normalize(alias) == wanted)
        {
            if let Some(result) = result(author) {
                matches.insert(result.external_id.clone(), result);
            }
        }
    }
    Ok((matches.len() == 1)
        .then(|| matches.into_values().next())
        .flatten())
}

pub(super) async fn snapshot(external_id: &str) -> Result<StoredMetadataSnapshot, String> {
    if !valid_id(external_id) {
        return Err("Invalid Open Library author identifier".into());
    }
    let author: Author = serde_json::from_value(get(&format!("{external_id}.json"), &[]).await?)
        .map_err(|error| error.to_string())?;
    if author.key.trim_start_matches('/') != external_id || author.name.trim().is_empty() {
        return Err("Open Library returned a different or incomplete author".into());
    }
    Ok(StoredMetadataSnapshot {
        provider_id: MetadataProviderId::OpenLibrary,
        external_id: external_id.into(),
        media_type: Some("author".into()),
        title: Some(author.name.trim().into()),
        overview: text(&author.bio),
        artwork_url: portrait(&author),
        backdrop_url: None,
        release_year: None,
        locale_key: String::new(),
        provider_locale_key: None,
        provider_payload_json: Some(
            serde_json::to_string(&author).map_err(|error| error.to_string())?,
        ),
    })
}

pub(super) fn details(snapshot: &StoredMetadataSnapshot) -> ProviderMetadataDetails {
    let author: Author = snapshot
        .provider_payload_json
        .as_deref()
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_default();
    ProviderMetadataDetails {
        tagline: lifespan(&author),
        external_ids: vec![
            crate::metadata::ProviderExternalId {
                source: "openlibrary".into(),
                external_id: snapshot.external_id.clone(),
            },
        ],
        ..Default::default()
    }
}
