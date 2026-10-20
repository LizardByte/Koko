//! Normalize artist URL relationships into the same IDs as film/TV providers.

use serde::Deserialize;

use crate::metadata::ProviderExternalId;

#[derive(Debug, Deserialize)]
pub(super) struct Relation {
    url: Option<Url>,
}

#[derive(Debug, Deserialize)]
struct Url {
    resource: String,
}

pub(super) fn artist_ids(relations: &[Relation]) -> Vec<ProviderExternalId> {
    let mut ids = Vec::new();
    for url in relations
        .iter()
        .filter_map(|relation| relation.url.as_ref())
    {
        let Some((source, external_id)) = url_id(&url.resource) else { continue };
        if !ids
            .iter()
            .any(|id: &ProviderExternalId| id.source == source && id.external_id == external_id)
        {
            ids.push(ProviderExternalId {
                source: source.into(),
                external_id,
            });
        }
    }
    ids
}

fn url_id(resource: &str) -> Option<(&'static str, String)> {
    let url = reqwest::Url::parse(resource).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = url
        .host_str()?
        .strip_prefix("www.")
        .unwrap_or(url.host_str()?);
    let segments = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let (source, id) = match (host, segments.as_slice()) {
        ("imdb.com", ["name", id]) if prefixed_number(id, "nm") => ("imdb", *id),
        ("wikidata.org", ["wiki" | "entity", id]) if prefixed_number(id, "Q") => ("wikidata", *id),
        ("themoviedb.org", ["person", id]) => ("tmdb", numeric_slug(id)?),
        ("thetvdb.com", ["people", id]) => ("thetvdb", numeric_slug(id)?),
        ("discogs.com", ["artist", id]) => ("discogs", numeric_slug(id)?),
        ("isni.org", ["isni", id]) => ("isni", *id),
        ("viaf.org", ["viaf", id]) if numeric(id) => ("viaf", *id),
        ("instagram.com", [id]) => ("instagram", *id),
        ("facebook.com", [id]) => ("facebook", *id),
        ("twitter.com" | "x.com", [id]) => ("twitter", *id),
        ("youtube.com", ["channel", id]) => ("youtube", *id),
        _ => return None,
    };
    Some((source, id.to_string()))
}

fn numeric(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|character| character.is_ascii_digit())
}

fn prefixed_number(
    value: &str,
    prefix: &str,
) -> bool {
    value.strip_prefix(prefix).is_some_and(numeric)
}

fn numeric_slug(value: &str) -> Option<&str> {
    let id = value.split('-').next()?;
    numeric(id).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_person_ids_without_confusing_titles_or_lookalike_domains() {
        for (url, source, id) in [
            (
                "https://www.imdb.com/name/nm0000173/?ref_=x",
                "imdb",
                "nm0000173",
            ),
            ("https://www.wikidata.org/wiki/Q1744", "wikidata", "Q1744"),
            (
                "https://www.themoviedb.org/person/516-keanu-reeves",
                "tmdb",
                "516",
            ),
            ("https://thetvdb.com/people/123", "thetvdb", "123"),
            (
                "https://www.discogs.com/artist/123-Artist",
                "discogs",
                "123",
            ),
        ] {
            assert_eq!(url_id(url), Some((source, id.into())));
        }
        for url in [
            "https://imdb.com/title/tt12345",
            "https://imdb.com.evil/name/nm123",
            "https://imdb.com/name/nm",
            "https://wikidata.org/wiki/Qbad",
            "https://themoviedb.org/movie/516",
            "https://example.com/artist",
        ] {
            assert!(url_id(url).is_none(), "{url}");
        }
    }
}
