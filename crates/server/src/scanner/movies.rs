//! Movie scanner rules.

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

pub(crate) fn scan(library: &MediaLibrarySettings) -> LibraryInspection {
    directory::scan_with_rules(
        library,
        ScannerRules::typed(
            MediaLibraryScanner::Movies,
            MediaLibraryKind::Movies,
            |_, title| display_title_from_name(title),
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
            MediaLibraryScanner::Movies,
            MediaLibraryKind::Movies,
            |_, title| display_title_from_name(title),
        ),
        sink,
    )
}

fn display_title_from_name(value: &str) -> String {
    static BRACKETED_TAG_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"[\{\[]([^\}\]]*)[\}\]]").unwrap());
    static YEAR_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"\b(19\d{2}|20\d{2}|21\d{2})\b").unwrap());
    static PARENTHETICAL_YEAR_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"[\(\[]\s*(19\d{2}|20\d{2}|21\d{2})\s*[\)\]]").unwrap());
    static DASH_FORMAT_SUFFIX_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(concat!(
            r"(?i)\s+[-–]\s+(?:bluray|blu-ray|brrip|web[- ]?dl|webrip|remux",
            r"|dvdrip|hdtv|uhd|dvd|proper|repack|extended|unrated|director'?s cut",
            r"|theatrical|final cut)?(?:[\s._-]*(?:2160p|1080p|720p|480p|4k",
            r"|uhd|hdr|dv|x264|x265|h264|h265|hevc|av1|aac|dts|truehd|atmos",
            r"|remux|bluray|blu-ray|web[- ]?dl|webrip|brrip|dvdrip))*\s*$",
        ))
        .unwrap()
    });
    static NOISE_TOKEN_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(concat!(
            r"(?i)\b(2160p|1080p|720p|480p|4k|uhd|x264|x265|h264|h265",
            r"|hevc|av1|hdr|dv|webrip|web[- ]?dl|bluray|blu-ray|brrip",
            r"|dvdrip|remux|aac|dts|truehd|atmos)\b",
        ))
        .unwrap()
    });
    static TITLE_COLON_DASH_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s*-\s+").unwrap());

    let without_tags = BRACKETED_TAG_REGEX.replace_all(value, " ");
    let mut normalized = DASH_FORMAT_SUFFIX_REGEX
        .replace(&without_tags, " ")
        .to_string();
    normalized = PARENTHETICAL_YEAR_REGEX
        .replace(&normalized, " ")
        .to_string();
    normalized = normalized.replace(['.', '_'], " ");
    if let Some(year_match) = YEAR_REGEX.find(&normalized) {
        if !normalized[..year_match.start()].trim().is_empty() {
            normalized = normalized[..year_match.start()].to_string();
        }
    }
    normalized = TITLE_COLON_DASH_REGEX
        .replace_all(&normalized, ": ")
        .to_string();
    normalized = NOISE_TOKEN_REGEX.replace_all(&normalized, " ").to_string();
    let cleaned = normalized
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|character: char| !character.is_ascii_alphanumeric())
        .to_string();

    if cleaned.is_empty() { value.to_string() } else { cleaned }
}
