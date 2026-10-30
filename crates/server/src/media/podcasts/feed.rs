//! Parse RSS and Atom podcast enclosures and publisher extensions without fetching external XML.
use super::{
    PodcastLink,
    PodcastMetadata,
    http::normalize_url,
};
use roxmltree::Node;

const ITUNES: &str = "http://www.itunes.com/dtds/podcast-1.0.dtd";
const PODCAST: &str = "https://podcastindex.org/namespace/1.0";
const CONTENT: &str = "http://purl.org/rss/1.0/modules/content/";

pub(super) struct Feed {
    pub title: String,
    pub metadata: PodcastMetadata,
    pub episodes: Vec<Episode>,
}
pub(super) struct Episode {
    pub title: String,
    pub guid: String,
    pub url: String,
    pub mime: String,
    pub duration_ms: Option<i64>,
    pub season: Option<i32>,
    pub number: Option<i32>,
    pub metadata: PodcastMetadata,
}

fn element<'a>(
    node: Node<'a, 'a>,
    name: &str,
    namespace: Option<&str>,
) -> Option<Node<'a, 'a>> {
    node.children().find(|n| {
        n.is_element()
            && n.tag_name().name() == name
            && namespace.map_or_else(
                || {
                    n.tag_name().namespace().is_none()
                        || n.tag_name().namespace() == Some("http://www.w3.org/2005/Atom")
                },
                |ns| n.tag_name().namespace() == Some(ns),
            )
    })
}
fn text(
    node: Node<'_, '_>,
    name: &str,
    namespace: Option<&str>,
) -> Option<String> {
    element(node, name, namespace)
        .and_then(|n| n.text())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}
fn url(
    base: &str,
    value: &str,
) -> Option<String> {
    let value = reqwest::Url::parse(base).ok()?.join(value).ok()?;
    normalize_url(value.as_str()).ok()
}
fn artwork(
    node: Node<'_, '_>,
    base: &str,
) -> Option<String> {
    element(node, "image", Some(ITUNES))
        .and_then(|n| n.attribute("href"))
        .and_then(|v| url(base, v))
        .or_else(|| {
            element(node, "image", None)
                .and_then(|n| text(n, "url", None))
                .and_then(|v| url(base, &v))
        })
}
fn metadata(
    node: Node<'_, '_>,
    base: &str,
) -> PodcastMetadata {
    let categories = node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "category")
        .flat_map(|n| n.descendants())
        .filter(|n| n.is_element() && n.tag_name().name() == "category")
        .filter_map(|n| {
            n.attribute("text")
                .or_else(|| n.attribute("term"))
                .or_else(|| n.text())
        })
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let website = text(node, "link", None).or_else(|| {
        node.children()
            .find(|n| {
                n.has_tag_name(("http://www.w3.org/2005/Atom", "link"))
                    && n.attribute("rel").unwrap_or("alternate") == "alternate"
            })
            .and_then(|n| n.attribute("href"))
            .map(str::to_owned)
    });
    let published = text(node, "pubDate", None)
        .or_else(|| text(node, "published", None))
        .or_else(|| text(node, "updated", None));
    PodcastMetadata {
        author: text(node, "author", Some(ITUNES))
            .or_else(|| text(node, "author", None))
            .or_else(|| element(node, "author", None).and_then(|n| text(n, "name", None))),
        description: text(node, "encoded", Some(CONTENT))
            .or_else(|| text(node, "description", None))
            .or_else(|| text(node, "summary", Some(ITUNES)))
            .or_else(|| text(node, "summary", None))
            .or_else(|| text(node, "content", None)),
        artwork_url: artwork(node, base),
        website_url: website.and_then(|v| url(base, &v)),
        language: text(node, "language", None),
        categories,
        explicit: text(node, "explicit", Some(ITUNES)).and_then(|v| {
            match v.to_ascii_lowercase().as_str() {
                "true" | "yes" | "explicit" => Some(true),
                "false" | "no" | "clean" => Some(false),
                _ => None,
            }
        }),
        copyright: text(node, "copyright", None).or_else(|| text(node, "rights", None)),
        show_type: text(node, "type", Some(ITUNES)),
        episode_type: text(node, "episodeType", Some(ITUNES)),
        published_at: published
            .and_then(|v| {
                chrono::DateTime::parse_from_rfc2822(&v)
                    .or_else(|_| chrono::DateTime::parse_from_rfc3339(&v))
                    .ok()
            })
            .map(|v| v.timestamp()),
        guid: text(node, "guid", None).or_else(|| text(node, "id", None)),
        transcripts: node
            .children()
            .filter(|n| n.has_tag_name((PODCAST, "transcript")))
            .filter_map(|n| {
                Some(PodcastLink {
                    url: url(base, n.attribute("url")?)?,
                    mime_type: n.attribute("type").map(str::to_owned),
                    language: n.attribute("language").map(str::to_owned),
                })
            })
            .collect(),
        chapters_url: element(node, "chapters", Some(PODCAST))
            .and_then(|n| n.attribute("url"))
            .and_then(|v| url(base, v)),
        people: node
            .children()
            .filter(|n| n.has_tag_name((PODCAST, "person")))
            .filter_map(|n| n.text())
            .map(str::to_owned)
            .collect(),
        ..PodcastMetadata::default()
    }
}

pub(super) fn duration(value: &str) -> Option<i64> {
    let parts = value.trim().split(':').collect::<Vec<_>>();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    let mut seconds = 0_f64;
    for part in parts {
        let part = part.parse::<f64>().ok()?;
        if !part.is_finite() || part < 0. {
            return None;
        }
        seconds = seconds * 60. + part;
    }
    (seconds <= i64::MAX as f64 / 1000.).then_some((seconds * 1000.).round() as i64)
}

pub(super) fn parse(
    xml: &str,
    base: &str,
) -> Result<Feed, String> {
    let document = roxmltree::Document::parse(xml)
        .map_err(|_| "The URL did not return a valid podcast XML feed")?;
    let root = document.root_element();
    let channel = if root.tag_name().name() == "rss" {
        element(root, "channel", None)
    } else if root.has_tag_name(("http://www.w3.org/2005/Atom", "feed")) {
        Some(root)
    } else {
        None
    }
    .ok_or("Use a podcast RSS or Atom feed URL")?;
    let title = text(channel, "title", None).ok_or("The podcast feed has no title")?;
    let metadata = metadata(channel, base);
    let mut episodes = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for node in channel
        .children()
        .filter(|n| n.is_element() && matches!(n.tag_name().name(), "item" | "entry"))
    {
        let enclosure = element(node, "enclosure", None).or_else(|| {
            node.children().find(|n| {
                n.is_element()
                    && n.tag_name().name() == "link"
                    && n.attribute("rel") == Some("enclosure")
            })
        });
        let Some(enclosure) = enclosure else {
            continue;
        };
        let Some(audio_url) = enclosure
            .attribute("url")
            .or_else(|| enclosure.attribute("href"))
            .and_then(|v| url(base, v))
        else {
            continue;
        };
        let mime = enclosure
            .attribute("type")
            .unwrap_or("audio/mpeg")
            .to_string();
        if !mime.starts_with("audio/")
            && !mime.starts_with("video/")
            && mime != "application/octet-stream"
        {
            continue;
        }
        let mut entry = metadata_for_episode(node, base, &metadata);
        let guid = entry.guid.clone().unwrap_or_else(|| audio_url.clone());
        if !seen.insert(guid.clone()) {
            continue;
        }
        entry.guid = Some(guid.clone());
        episodes.push(Episode {
            title: text(node, "title", None).unwrap_or_else(|| "Untitled episode".into()),
            guid,
            url: audio_url,
            mime,
            duration_ms: text(node, "duration", Some(ITUNES)).and_then(|v| duration(&v)),
            season: text(node, "season", Some(ITUNES))
                .and_then(|v| v.parse::<i32>().ok())
                .filter(|v| *v > 0),
            number: text(node, "episode", Some(ITUNES))
                .and_then(|v| v.parse::<i32>().ok())
                .filter(|v| *v >= 0),
            metadata: entry,
        });
    }
    // Some valid shows publish their first episode later; accept their feed without inventing one.
    Ok(Feed {
        title,
        metadata,
        episodes,
    })
}

fn metadata_for_episode(
    node: Node<'_, '_>,
    base: &str,
    show: &PodcastMetadata,
) -> PodcastMetadata {
    let mut value = metadata(node, base);
    value.author = value.author.or_else(|| show.author.clone());
    value.artwork_url = value.artwork_url.or_else(|| show.artwork_url.clone());
    value.language = value.language.or_else(|| show.language.clone());
    value.explicit = value.explicit.or(show.explicit);
    value.show_type = value.show_type.or_else(|| show.show_type.clone());
    if value.categories.is_empty() {
        value.categories = show.categories.clone();
    }
    value
}
