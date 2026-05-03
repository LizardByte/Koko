//! TV show scanner rules.

use once_cell::sync::Lazy;
use regex::Regex;

use crate::config::{
    MediaLibraryKind,
    MediaLibraryScanner,
    MediaLibrarySettings,
};
use crate::scanner::directory::{
    self,
    ScannerRules,
};
use crate::scanner::{
    LibraryInspection,
    ScannerSink,
};

/// Show, season, and episode fields derived from a library-relative episode path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedShowPath {
    pub(crate) show_title: String,
    pub(crate) show_key: String,
    pub(crate) season_title: String,
    pub(crate) season_key: String,
    pub(crate) season_number: Option<i32>,
    pub(crate) episode_title: String,
    pub(crate) episode_key: String,
    pub(crate) episode_number: Option<i32>,
}

pub(crate) fn scan(library: &MediaLibrarySettings) -> LibraryInspection {
    directory::scan_with_rules(
        library,
        ScannerRules::typed(
            MediaLibraryScanner::Shows,
            MediaLibraryKind::Shows,
            |relative_path, title| parse_show_path(relative_path, title, 0).episode_title,
        ),
    )
}

pub(crate) fn scan_streaming<S>(
    library: &MediaLibrarySettings,
    sink: &mut S,
) -> Result<LibraryInspection, S::Error>
where
    S: ScannerSink,
{
    directory::scan_with_rules_streaming(
        library,
        ScannerRules::typed(
            MediaLibraryScanner::Shows,
            MediaLibraryKind::Shows,
            |relative_path, title| parse_show_path(relative_path, title, 0).episode_title,
        ),
        sink,
    )
}

/// Parse a show episode path following the documented Koko show naming forms.
pub(crate) fn parse_show_path(
    relative_path: &str,
    fallback_title: &str,
    library_id: i32,
) -> ParsedShowPath {
    let normalized = relative_path.replace('\\', "/");
    let parts = normalized
        .split('/')
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>();
    let filename = parts.last().copied().unwrap_or(fallback_title);
    let file_stem = filename
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(filename);
    let raw_show_title = parts
        .first()
        .copied()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback_title);
    let show_title = clean_show_title(raw_show_title)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback_title.trim().to_string());
    let season_source =
        if parts.len() >= 2 { parts[parts.len().saturating_sub(2)] } else { fallback_title };
    let season_number = infer_season_number(season_source)
        .or_else(|| infer_season_number(file_stem))
        .or_else(|| infer_season_number(fallback_title))
        .filter(|value| *value > 0);
    let episode_number = infer_episode_number(file_stem)
        .or_else(|| infer_episode_number(fallback_title))
        .filter(|value| *value > 0);
    let episode_title = episode_title_from_name(file_stem, &show_title, episode_number)
        .or_else(|| episode_title_from_name(fallback_title, &show_title, episode_number))
        .unwrap_or_else(|| cleaned_episode_fallback(fallback_title));
    let season_title = season_number
        .map(|number| format!("Season {}", number))
        .unwrap_or_else(|| season_source.trim().to_string());
    let show_key = format!(
        "library:{}:show:{}",
        library_id,
        normalize_identity_segment(&show_title)
    );
    let season_key = format!(
        "{}:season:{}",
        show_key,
        season_number
            .map(|value| value.to_string())
            .unwrap_or_else(|| normalize_identity_segment(&season_title))
    );
    let episode_key = format!(
        "{}:episode:{}:{}",
        season_key,
        episode_number
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".into()),
        normalize_identity_segment(&episode_title)
    );

    ParsedShowPath {
        show_title,
        show_key,
        season_title,
        season_key,
        season_number,
        episode_title,
        episode_key,
        episode_number,
    }
}

/// Infer a season number from common folder or filename patterns.
pub fn infer_season_number(value: &str) -> Option<i32> {
    static SEASON_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
        vec![
            Regex::new(r"(?i)(?:^|[^a-z0-9])season\s*(\d{1,3})(?:[^0-9]|$)").unwrap(),
            Regex::new(r"(?i)(?:^|[^a-z0-9])series\s*(\d{1,3})(?:[^0-9]|$)").unwrap(),
            Regex::new(r"(?i)(?:^|[^a-z0-9])s(\d{1,3})(?:\s*e\d{1,3}|[^0-9]|$)").unwrap(),
            Regex::new(r"(?i)(?:^|[^a-z0-9])(\d{1,3})x\d{1,3}(?:[^0-9]|$)").unwrap(),
        ]
    });

    first_pattern_number(value, &SEASON_PATTERNS)
}

/// Infer an episode number from common filename patterns such as `S03E01` or `3x01`.
pub fn infer_episode_number(value: &str) -> Option<i32> {
    static EPISODE_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
        vec![
            Regex::new(r"(?i)(?:^|[^a-z0-9])s\d{1,3}\s*e(\d{1,3})(?:[^0-9]|$)").unwrap(),
            Regex::new(r"(?i)(?:^|[^a-z0-9])\d{1,3}x(\d{1,3})(?:[^0-9]|$)").unwrap(),
            Regex::new(r"(?i)(?:^|[^a-z0-9])e(\d{1,3})(?:[^0-9]|$)").unwrap(),
        ]
    });

    first_pattern_number(value, &EPISODE_PATTERNS)
}

fn first_pattern_number(
    value: &str,
    patterns: &[Regex],
) -> Option<i32> {
    patterns.iter().find_map(|pattern| {
        pattern
            .captures(value)
            .and_then(|captures| captures.get(1))
            .and_then(|matched| matched.as_str().parse::<i32>().ok())
    })
}

fn clean_show_title(value: &str) -> Option<String> {
    static BRACED_TAG_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"[\{\[]([^\}\]]*)[\}\]]").unwrap());
    static PARENTHETICAL_YEAR_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"[\(\[]\s*(19\d{2}|20\d{2}|21\d{2})\s*[\)\]]").unwrap());
    static YEAR_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"\b(19\d{2}|20\d{2}|21\d{2})\b").unwrap());

    let without_tags = BRACED_TAG_REGEX.replace_all(value, " ");
    let mut normalized = PARENTHETICAL_YEAR_REGEX
        .replace(&without_tags, " ")
        .replace(['.', '_'], " ");
    if let Some(year_match) = YEAR_REGEX.find(&normalized) {
        if !normalized[..year_match.start()].trim().is_empty() {
            normalized = normalized[..year_match.start()].to_string();
        }
    }
    cleaned_text(&normalized)
}

fn episode_title_from_name(
    value: &str,
    show_title: &str,
    episode_number: Option<i32>,
) -> Option<String> {
    static EPISODE_MARKER_WITH_TITLE_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)(?:s\d{1,3}\s*e\d{1,3}|\d{1,3}x\d{1,3}|e\d{1,3})\s*(?:[-–:._ ]+)\s*(.+)$")
            .unwrap()
    });

    let normalized = value.replace(['.', '_'], " ");
    if let Some(candidate) = EPISODE_MARKER_WITH_TITLE_REGEX
        .captures(&normalized)
        .and_then(|captures| captures.get(1))
        .and_then(|matched| cleaned_text(matched.as_str()))
    {
        return Some(candidate);
    }

    let mut cleaned = normalized;
    if !show_title.trim().is_empty()
        && cleaned
            .to_ascii_lowercase()
            .starts_with(&show_title.to_ascii_lowercase())
    {
        cleaned = cleaned[show_title.len()..].to_string();
    }
    if let Some(number) = episode_number {
        for marker in [
            format!(
                "S{:02}E{:02}",
                infer_season_number(value).unwrap_or_default(),
                number
            ),
            format!("E{:02}", number),
            format!("x{:02}", number),
        ] {
            cleaned = cleaned.replace(&marker, " ");
            cleaned = cleaned.replace(&marker.to_ascii_lowercase(), " ");
        }
    }

    cleaned_text(&cleaned)
}

fn cleaned_episode_fallback(value: &str) -> String {
    cleaned_text(&value.replace(['.', '_'], " ")).unwrap_or_else(|| value.trim().to_string())
}

fn cleaned_text(value: &str) -> Option<String> {
    let collapsed = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|character: char| !character.is_ascii_alphanumeric())
        .to_string();
    (!collapsed.trim().is_empty()).then_some(collapsed)
}

fn normalize_identity_segment(value: &str) -> String {
    value
        .chars()
        .map(
            |character| {
                if character.is_ascii_alphanumeric() { character.to_ascii_lowercase() } else { '-' }
            },
        )
        .collect::<String>()
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
