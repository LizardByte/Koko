//! Resolve curated collection pages, never a patron's unrelated personal reading lists.

use super::*;
use std::collections::HashSet;

const INVALID_QUERY: &str = "Invalid curator-authored collection search";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(super) struct CuratedCollection {
    pub key: String,
    pub title: String,
}

#[derive(Default)]
pub(super) struct Memberships {
    pub collections: Vec<CuratedCollection>,
    pub unavailable: Vec<String>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Page {
    key: String,
    title: String,
    body: Value,
}

#[derive(Debug, PartialEq)]
enum Carousel {
    Query(String),
    List(String),
}

fn valid_page(key: &str) -> bool {
    key.starts_with("/collections/")
        && key.len() <= 512
        && !key
            .chars()
            .any(|c| c.is_control() || matches!(c, '?' | '#' | '\\'))
        && key
            .split('/')
            .skip(1)
            .all(|part| !part.is_empty() && !matches!(part, "." | ".."))
}

fn list_key(value: &str) -> Option<String> {
    static LIST: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"^(/people/[A-Za-z0-9_.-]+/lists/OL[0-9]+L)(?:/[^?#]*)?$").unwrap()
    });
    LIST.captures(value.trim())
        .map(|captures| captures[1].to_string())
}

/// Read literal macro arguments as data; curator-authored code is never evaluated.
fn carousels(body: &str) -> Vec<Carousel> {
    static MACRO: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?s)\{\{\s*(QueryCarousel|ListCarousel)\((.*?)\)\s*\}\}").unwrap()
    });
    static ARG: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(
            r#"(?s)(?:^|,)\s*(?:(\w+)\s*=\s*)?(?:"((?:\\.|[^"\\])*)"|'((?:\\.|[^'\\])*)')"#,
        )
        .unwrap()
    });
    MACRO
        .captures_iter(body)
        .filter_map(|call| {
            let query = ARG.captures_iter(&call[2]).find_map(|argument| {
                let name = argument.get(1).map(|name| name.as_str());
                (name == Some("query")
                    || name == Some("list_key")
                    || name.is_none() && argument.get(0)?.start() == 0)
                    .then(|| {
                        unquote(
                            argument
                                .get(2)
                                .or_else(|| argument.get(3))
                                .unwrap()
                                .as_str(),
                        )
                    })
            })?;
            if call[1].eq("ListCarousel") {
                list_key(&query).map(Carousel::List)
            } else if !query.trim().is_empty() && query.len() <= 16_000 {
                Some(Carousel::Query(query))
            } else {
                None
            }
        })
        .collect()
}

fn unquote(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            output.push(c);
            continue;
        }
        match chars.next() {
            Some(c @ ('\'' | '"' | '\\')) => output.push(c),
            Some(c) => {
                output.push('\\');
                output.push(c);
            }
            None => output.push('\\'),
        }
    }
    output
}

async fn catalog() -> Result<Vec<Page>, String> {
    let mut pages = Vec::new();
    loop {
        let response = get(
            "query.json",
            &[
                ("type".into(), "/type/page".into()),
                ("key~".into(), "/collections/*".into()),
                ("title".into(), String::new()),
                ("body".into(), String::new()),
                ("limit".into(), "1000".into()),
                ("offset".into(), pages.len().to_string()),
            ],
        )
        .await?;
        let batch: Vec<Page> = serde_json::from_value(response)
            .map_err(|e| format!("Invalid Open Library collection catalog: {e}"))?;
        let done = batch.len() < 1000;
        pages.extend(batch);
        if done {
            break;
        }
        if pages.len() >= 10_000 {
            return Err("Open Library collection catalog exceeds the supported size".into());
        }
    }
    pages.retain(|page| valid_page(&page.key) && !page.title.trim().is_empty());
    pages.sort_by(|a, b| a.key.cmp(&b.key));
    pages.dedup_by(|a, b| a.key == b.key);
    Ok(pages)
}

async fn work_lists(work: &str) -> Result<HashSet<String>, String> {
    let mut lists = HashSet::new();
    let mut offset = 0;
    loop {
        let response = get(
            &format!("{work}/lists.json"),
            &[
                ("limit".into(), "100".into()),
                ("offset".into(), offset.to_string()),
            ],
        )
        .await?;
        let entries = response["entries"]
            .as_array()
            .ok_or("Open Library list memberships are missing")?;
        lists.extend(
            entries
                .iter()
                .filter_map(|entry| entry["url"].as_str().and_then(list_key)),
        );
        offset += entries.len();
        if entries.len() < 100
            || response["size"]
                .as_u64()
                .is_some_and(|total| offset as u64 >= total)
        {
            break;
        }
        if offset >= 10_000 {
            return Err("Open Library list memberships exceed the supported size".into());
        }
    }
    Ok(lists)
}

/// Explicit work lists need no Solr request; other queries retain Open Library's own semantics.
fn explicit_work_match(
    query: &str,
    work: &str,
) -> Option<bool> {
    static ID: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| regex::Regex::new(r"(?:/works/)?OL[0-9]+W").unwrap());
    static SYNTAX: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r#"\b(?:key|work):|\bOR\b|[\s()'"]"#).unwrap()
    });
    if !query.trim_start().starts_with("key:") && !query.trim_start().starts_with("work:") {
        return None;
    }
    let remainder = ID.replace_all(query, "");
    if !SYNTAX.replace_all(&remainder, "").is_empty() {
        return None;
    }
    Some(
        ID.find_iter(query).any(|id| {
            id.as_str().trim_start_matches("/works/") == work.trim_start_matches("works/")
        }),
    )
}

async fn matches_query(
    work: &str,
    query: &str,
) -> Result<Option<bool>, String> {
    let Some(response) = request(
        "search.json",
        &[
            ("q".into(), format!("key:/{work} AND ({query})")),
            ("fields".into(), "key".into()),
            ("limit".into(), "1".into()),
        ],
        true,
    )
    .await?
    else {
        return Ok(None);
    };
    let docs = response["docs"]
        .as_array()
        .ok_or("Open Library collection search results are missing")?;
    if docs.iter().any(|doc| {
        doc["key"]
            .as_str()
            .is_none_or(|key| key.trim_start_matches('/') != work)
    }) {
        return Err("Open Library returned a different work for a collection lookup".into());
    }
    Ok(Some(!docs.is_empty()))
}

async fn matches_group(
    work: &str,
    group: &[&str],
) -> Result<bool, String> {
    let query = group
        .iter()
        .map(|q| format!("({q})"))
        .collect::<Vec<_>>()
        .join(" OR ");
    if let Some(matched) = matches_query(work, &query).await? {
        return Ok(matched);
    }
    // One malformed carousel must not hide other valid carousels on the same page.
    let mut invalid = false;
    for query in group {
        match matches_query(work, query).await? {
            Some(true) => return Ok(true),
            None => invalid = true,
            Some(false) => {}
        }
    }
    if invalid {
        return Err(INVALID_QUERY.into());
    }
    Ok(false)
}

async fn matches_queries(
    work: &str,
    queries: &[String],
) -> Result<bool, String> {
    let mut group = Vec::new();
    let mut length = 0;
    for query in queries {
        if length + query.len() > 1500 && !group.is_empty() {
            if matches_group(work, &group).await? {
                return Ok(true);
            }
            group.clear();
            length = 0;
        }
        group.push(query.as_str());
        length += query.len() + 6;
    }
    Ok(!group.is_empty() && matches_group(work, &group).await?)
}

pub(super) async fn memberships(work: &str) -> Result<Memberships, String> {
    let definitions = catalog()
        .await?
        .into_iter()
        .map(|page| {
            let body = page
                .body
                .as_str()
                .or_else(|| page.body["value"].as_str())
                .unwrap_or_default();
            let definitions = carousels(body);
            (page, definitions)
        })
        .collect::<Vec<_>>();
    let lists = if definitions
        .iter()
        .any(|(_, definitions)| definitions.iter().any(|c| matches!(c, Carousel::List(_))))
    {
        match work_lists(work).await {
            Ok(lists) => Some(lists),
            Err(error) => {
                log::warn!("Open Library curated list memberships unavailable: {error}");
                None
            }
        }
    } else {
        Some(HashSet::new())
    };
    let mut result = Memberships::default();
    for (page, definitions) in definitions {
        let mut matched = false;
        let mut unknown = false;
        let mut queries = Vec::new();
        for definition in definitions {
            match definition {
                Carousel::List(key) => {
                    if let Some(lists) = &lists {
                        matched |= lists.contains(&key);
                    } else {
                        unknown = true;
                    }
                }
                Carousel::Query(query) => match explicit_work_match(&query, work) {
                    Some(value) => matched |= value,
                    None => queries.push(query),
                },
            }
        }
        if !matched {
            match matches_queries(work, &queries).await {
                Ok(value) => matched = value,
                Err(error) if error == INVALID_QUERY => {
                    unknown = true;
                    log::warn!("Open Library collection {} unavailable: {error}", page.key);
                }
                // Stop on outages instead of waiting for one network timeout per collection.
                Err(error) => return Err(error),
            }
        }
        if matched {
            result.collections.push(CuratedCollection {
                key: page.key,
                title: page.title.trim().to_string(),
            });
        } else if unknown {
            result.unavailable.push(page.key);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
