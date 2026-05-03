//! Media scanner selection and file inventory primitives.

pub mod books;
pub mod directory;
pub mod movies;
pub mod music;
pub mod photos;
pub mod shows;

use std::collections::HashSet;
use std::path::{
    Path,
    PathBuf,
};

use schemars::JsonSchema;
use serde::Serialize;

use crate::config::{
    MediaLibraryKind,
    MediaLibraryScanner,
    MediaLibrarySettings,
};

pub(crate) use directory::fallback_title_from_relative_path;

/// Scan status for a configured media library.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LibraryScanStatus {
    /// The library exists in configuration but has not been scanned yet.
    NeverScanned,
    /// The library path exists and was scanned successfully.
    Available,
    /// The library path was empty.
    EmptyPath,
    /// The library path does not exist.
    MissingPath,
    /// The configured path exists but is not a directory.
    NotDirectory,
    /// The library path could not be read completely.
    Unreadable,
}

/// Summary of one configured media library scan.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct LibraryScanSummary {
    /// Human-friendly library name.
    pub name: String,
    /// Configured filesystem path.
    pub path: String,
    /// Configured filesystem paths for this logical library.
    pub paths: Vec<String>,
    /// Whether the scan is recursive.
    pub recursive: bool,
    /// Intended media category for the library.
    pub kind: MediaLibraryKind,
    /// Scanner used for the library inventory.
    pub scanner: MediaLibraryScanner,
    /// Scan status for this library.
    pub status: LibraryScanStatus,
    /// Total number of files discovered.
    pub total_files: u64,
    /// Number of video files discovered.
    pub video_files: u64,
    /// Number of audio files discovered.
    pub audio_files: u64,
    /// Number of image files discovered.
    pub image_files: u64,
    /// Number of book or document files discovered.
    pub book_files: u64,
    /// Number of files that do not match known media extensions.
    pub other_files: u64,
    /// The last scan error, if any.
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LibraryInspection {
    pub(crate) summary: LibraryScanSummary,
    pub(crate) files: Vec<DiscoveredMediaFile>,
    pub(crate) scanned_root_paths: HashSet<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct DiscoveredMediaFile {
    pub(crate) full_path: PathBuf,
    pub(crate) source_root_path: String,
    pub(crate) relative_path: String,
    pub(crate) file_size: i64,
    pub(crate) modified_at: Option<i64>,
    pub(crate) media_kind: String,
    pub(crate) file_hash: String,
    pub(crate) default_title: String,
}

pub(crate) struct FileHashCandidate<'a> {
    pub(crate) full_path: &'a Path,
    pub(crate) source_root_path: &'a str,
    pub(crate) relative_path: &'a str,
    pub(crate) file_size: i64,
    pub(crate) modified_at: Option<i64>,
}

pub(crate) trait ScannerSink {
    type Error;

    fn scanned_root(
        &mut self,
        source_root_path: &str,
    ) -> Result<(), Self::Error>;

    fn file_hash(
        &mut self,
        candidate: FileHashCandidate<'_>,
    ) -> Result<Option<String>, Self::Error>;

    fn file(
        &mut self,
        file: DiscoveredMediaFile,
    ) -> Result<(), Self::Error>;
}

/// Inspect a configured library with its selected scanner.
pub(crate) fn inspect_library(library: &MediaLibrarySettings) -> LibraryInspection {
    match library.scanner.effective_for_kind(&library.kind) {
        MediaLibraryScanner::Auto => directory::scan(library),
        MediaLibraryScanner::Directory => directory::scan(library),
        MediaLibraryScanner::Movies => movies::scan(library),
        MediaLibraryScanner::Shows => shows::scan(library),
        MediaLibraryScanner::Music => music::scan(library),
        MediaLibraryScanner::Photos => photos::scan(library),
        MediaLibraryScanner::Books => books::scan(library),
    }
}

/// Inspect a configured library and stream each discovered media file to the caller.
pub(crate) fn inspect_library_streaming<S>(
    library: &MediaLibrarySettings,
    sink: &mut S,
) -> Result<LibraryInspection, S::Error>
where
    S: ScannerSink,
{
    match library.scanner.effective_for_kind(&library.kind) {
        MediaLibraryScanner::Auto => directory::scan_streaming(library, sink),
        MediaLibraryScanner::Directory => directory::scan_streaming(library, sink),
        MediaLibraryScanner::Movies => movies::scan_streaming(library, sink),
        MediaLibraryScanner::Shows => shows::scan_streaming(library, sink),
        MediaLibraryScanner::Music => music::scan_streaming(library, sink),
        MediaLibraryScanner::Photos => photos::scan_streaming(library, sink),
        MediaLibraryScanner::Books => books::scan_streaming(library, sink),
    }
}
