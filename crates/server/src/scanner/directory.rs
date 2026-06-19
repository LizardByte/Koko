//! Shared directory scanner and common scanner hashing.

use std::collections::HashSet;
use std::convert::Infallible;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use imohash::Hasher as ImoHasher;

use crate::config::{MediaLibraryKind, MediaLibraryScanner, MediaLibrarySettings};
use crate::scanner::{
    DiscoveredMediaFile, FileHashCandidate, LibraryInspection, LibraryScanStatus,
    LibraryScanSummary, ScannerSink,
};

#[derive(Debug, Default)]
pub(crate) struct FileCounters {
    total_files: u64,
    video_files: u64,
    audio_files: u64,
    image_files: u64,
    book_files: u64,
    other_files: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileKind {
    Video,
    Audio,
    Image,
    Book,
    Other,
}

impl FileKind {
    fn as_storage_value(&self) -> &'static str {
        match self {
            FileKind::Video => "video",
            FileKind::Audio => "audio",
            FileKind::Image => "image",
            FileKind::Book => "book",
            FileKind::Other => "other",
        }
    }
}

pub(crate) struct ScannerRules {
    scanner: MediaLibraryScanner,
    include_kind: MediaLibraryKind,
    default_title: fn(&str, &str) -> String,
}

impl ScannerRules {
    pub(crate) fn for_directory(library_kind: &MediaLibraryKind) -> Self {
        Self {
            scanner: MediaLibraryScanner::Directory,
            include_kind: library_kind.clone(),
            default_title: |_, title| title.to_string(),
        }
    }

    pub(crate) fn typed(
        scanner: MediaLibraryScanner,
        include_kind: MediaLibraryKind,
        default_title: fn(&str, &str) -> String,
    ) -> Self {
        Self {
            scanner,
            include_kind,
            default_title,
        }
    }
}

struct ScannerProgress {
    library_name: String,
    root_path: String,
    hashed_files: u64,
    reused_hashes: u64,
    next_log_at: u64,
}

impl ScannerProgress {
    fn new(
        library_name: &str,
        root_path: &str,
    ) -> Self {
        Self {
            library_name: library_name.to_string(),
            root_path: root_path.to_string(),
            hashed_files: 0,
            reused_hashes: 0,
            next_log_at: 100,
        }
    }

    fn record_hashed_file(&mut self) {
        self.hashed_files += 1;
        if self.hashed_files >= self.next_log_at {
            log::info!(
                "Calculated {} scanner hash(es) for library {} under {}",
                self.hashed_files,
                self.library_name,
                self.root_path
            );
            self.next_log_at += 100;
        }
    }

    fn record_reused_hash(&mut self) {
        self.reused_hashes += 1;
        let processed_files = self.hashed_files + self.reused_hashes;
        if processed_files >= self.next_log_at {
            log::info!(
                "Processed {} media file(s) for library {} under {} ({} calculated, {} reused)",
                processed_files,
                self.library_name,
                self.root_path,
                self.hashed_files,
                self.reused_hashes
            );
            self.next_log_at += 100;
        }
    }
}

enum DirectoryScanError<E> {
    Io(io::Error),
    Handler(E),
}

impl<E> From<io::Error> for DirectoryScanError<E> {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub(crate) fn scan(library: &MediaLibrarySettings) -> LibraryInspection {
    scan_with_rules(library, ScannerRules::for_directory(&library.kind))
}

pub(crate) fn scan_with_rules(
    library: &MediaLibrarySettings,
    rules: ScannerRules,
) -> LibraryInspection {
    struct CollectingSink {
        files: Vec<DiscoveredMediaFile>,
    }

    impl ScannerSink for CollectingSink {
        type Error = Infallible;

        fn scanned_root(
            &mut self,
            _source_root_path: &str,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        fn file_hash(
            &mut self,
            _candidate: FileHashCandidate<'_>,
        ) -> Result<Option<String>, Self::Error> {
            Ok(None)
        }

        fn file(
            &mut self,
            file: DiscoveredMediaFile,
        ) -> Result<(), Self::Error> {
            self.files.push(file);
            Ok(())
        }
    }

    let mut sink = CollectingSink { files: Vec::new() };
    let result = scan_with_rules_streaming(library, rules, &mut sink);
    let mut inspection = match result {
        Ok(inspection) => inspection,
        Err(error) => match error {},
    };
    inspection.files = sink.files;
    inspection
}

pub(crate) fn scan_streaming<S>(
    library: &MediaLibrarySettings,
    sink: &mut S,
) -> Result<LibraryInspection, S::Error>
where
    S: ScannerSink,
{
    scan_with_rules_streaming(library, ScannerRules::for_directory(&library.kind), sink)
}

pub(crate) fn scan_with_rules_streaming<S>(
    library: &MediaLibrarySettings,
    rules: ScannerRules,
    sink: &mut S,
) -> Result<LibraryInspection, S::Error>
where
    S: ScannerSink,
{
    let configured_paths = library.configured_paths();
    let path = configured_paths.first().cloned().unwrap_or_default();
    let name = display_name(library, &path);

    if configured_paths.is_empty() {
        return Ok(LibraryInspection {
            summary: LibraryScanSummary {
                name,
                path,
                paths: Vec::new(),
                recursive: library.recursive,
                kind: library.kind.clone(),
                scanner: rules.scanner,
                status: LibraryScanStatus::EmptyPath,
                total_files: 0,
                video_files: 0,
                audio_files: 0,
                image_files: 0,
                book_files: 0,
                other_files: 0,
                error: Some("Library path is empty".into()),
            },
            files: Vec::new(),
            scanned_root_paths: HashSet::new(),
        });
    }

    let mut counters = FileCounters::default();
    let mut scanned_root_paths = HashSet::new();
    let mut errors = Vec::new();
    let mut first_failure_status = None;

    for configured_path in &configured_paths {
        let filesystem_path = Path::new(configured_path);
        if !filesystem_path.exists() {
            first_failure_status.get_or_insert(LibraryScanStatus::MissingPath);
            errors.push(format!("{}: path does not exist", configured_path));
            continue;
        }

        if !filesystem_path.is_dir() {
            first_failure_status.get_or_insert(LibraryScanStatus::NotDirectory);
            errors.push(format!("{}: path is not a directory", configured_path));
            continue;
        }

        match fs::read_dir(filesystem_path) {
            Ok(entries) => drop(entries),
            Err(error) => {
                first_failure_status.get_or_insert(LibraryScanStatus::Unreadable);
                log::warn!(
                    "Skipping unreadable media library root {}: {}",
                    configured_path,
                    error
                );
                errors.push(format!("{}: {}", configured_path, error));
                continue;
            }
        }

        sink.scanned_root(configured_path)?;
        log::info!(
            "Scanning library {} root {} with {:?} scanner; file hashes are imohash sample values",
            name,
            configured_path,
            rules.scanner
        );
        let mut progress = ScannerProgress::new(&name, configured_path);
        match scan_directory(
            filesystem_path,
            filesystem_path,
            library.recursive,
            &rules,
            &mut progress,
            &mut errors,
            sink,
        ) {
            Ok(nested) => {
                log::info!(
                    "Finished scanning library {} root {}: {} matched file(s)",
                    name,
                    configured_path,
                    nested.total_files
                );
                scanned_root_paths.insert(configured_path.clone());
                counters.total_files += nested.total_files;
                counters.video_files += nested.video_files;
                counters.audio_files += nested.audio_files;
                counters.image_files += nested.image_files;
                counters.book_files += nested.book_files;
                counters.other_files += nested.other_files;
            }
            Err(DirectoryScanError::Io(error)) => {
                first_failure_status.get_or_insert(LibraryScanStatus::Unreadable);
                errors.push(format!("{}: {}", configured_path, error));
            }
            Err(DirectoryScanError::Handler(error)) => return Err(error),
        }
    }

    if counters.total_files > 0 || errors.len() < configured_paths.len() {
        Ok(LibraryInspection {
            summary: LibraryScanSummary {
                name,
                path,
                paths: configured_paths,
                recursive: library.recursive,
                kind: library.kind.clone(),
                scanner: rules.scanner,
                status: LibraryScanStatus::Available,
                total_files: counters.total_files,
                video_files: counters.video_files,
                audio_files: counters.audio_files,
                image_files: counters.image_files,
                book_files: counters.book_files,
                other_files: counters.other_files,
                error: (!errors.is_empty()).then(|| errors.join("; ")),
            },
            files: Vec::new(),
            scanned_root_paths,
        })
    } else {
        Ok(LibraryInspection {
            summary: LibraryScanSummary {
                name,
                path,
                paths: configured_paths,
                recursive: library.recursive,
                kind: library.kind.clone(),
                scanner: rules.scanner,
                status: first_failure_status.unwrap_or(LibraryScanStatus::Unreadable),
                total_files: 0,
                video_files: 0,
                audio_files: 0,
                image_files: 0,
                book_files: 0,
                other_files: 0,
                error: Some(errors.join("; ")),
            },
            files: Vec::new(),
            scanned_root_paths,
        })
    }
}

fn scan_directory<S>(
    root: &Path,
    path: &Path,
    recursive: bool,
    rules: &ScannerRules,
    progress: &mut ScannerProgress,
    errors: &mut Vec<String>,
    sink: &mut S,
) -> Result<FileCounters, DirectoryScanError<S::Error>>
where
    S: ScannerSink,
{
    let mut counters = FileCounters::default();

    for entry in fs::read_dir(path)? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                record_scan_io_error(errors, path, "reading directory entry", &error);
                continue;
            }
        };
        let entry_path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                record_scan_io_error(errors, &entry_path, "reading file type", &error);
                continue;
            }
        };

        if file_type.is_dir() {
            if recursive {
                let nested =
                    match scan_directory(root, &entry_path, true, rules, progress, errors, sink) {
                        Ok(nested) => nested,
                        Err(DirectoryScanError::Io(error)) => {
                            record_scan_io_error(errors, &entry_path, "reading directory", &error);
                            continue;
                        }
                        Err(DirectoryScanError::Handler(error)) => {
                            return Err(DirectoryScanError::Handler(error));
                        }
                    };
                counters.total_files += nested.total_files;
                counters.video_files += nested.video_files;
                counters.audio_files += nested.audio_files;
                counters.image_files += nested.image_files;
                counters.book_files += nested.book_files;
                counters.other_files += nested.other_files;
            }
            continue;
        }

        if file_type.is_file() {
            let kind = classify_file(&entry_path);
            if !should_include_library_item(&entry_path, kind, &rules.include_kind) {
                continue;
            }

            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(error) => {
                    record_scan_io_error(errors, &entry_path, "reading file metadata", &error);
                    continue;
                }
            };
            let file_size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
            let modified_at = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .and_then(|duration| i64::try_from(duration.as_secs()).ok());
            let source_root_path = root.to_string_lossy().to_string();
            let relative_path = normalize_relative_path(root, &entry_path);
            let media_kind = kind.as_storage_value().to_string();
            let raw_default_title = entry_path
                .file_stem()
                .and_then(|value| value.to_str())
                .map(|value| value.to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| fallback_title_from_relative_path(&relative_path));
            let default_title = (rules.default_title)(&relative_path, &raw_default_title);
            let file_hash = if let Some(file_hash) = sink
                .file_hash(FileHashCandidate {
                    full_path: &entry_path,
                    source_root_path: &source_root_path,
                    relative_path: &relative_path,
                    file_size,
                    modified_at,
                })
                .map_err(DirectoryScanError::Handler)?
            {
                progress.record_reused_hash();
                file_hash
            } else {
                let file_hash = match hash_file_fingerprint(&entry_path) {
                    Ok(file_hash) => file_hash,
                    Err(error) => {
                        record_scan_io_error(errors, &entry_path, "hashing file", &error);
                        continue;
                    }
                };
                progress.record_hashed_file();
                file_hash
            };

            sink.file(DiscoveredMediaFile {
                full_path: entry_path.clone(),
                source_root_path,
                relative_path,
                file_size,
                modified_at,
                media_kind,
                file_hash,
                default_title,
            })
            .map_err(DirectoryScanError::Handler)?;

            counters.total_files += 1;
            match kind {
                FileKind::Video => counters.video_files += 1,
                FileKind::Audio => counters.audio_files += 1,
                FileKind::Image => counters.image_files += 1,
                FileKind::Book => counters.book_files += 1,
                FileKind::Other => counters.other_files += 1,
            }
        }
    }

    Ok(counters)
}

fn record_scan_io_error(
    errors: &mut Vec<String>,
    path: &Path,
    operation: &str,
    error: &io::Error,
) {
    let path = path.to_string_lossy();
    log::warn!(
        "Skipping {} while scanning media library: {} ({})",
        path,
        operation,
        error
    );
    errors.push(format!("{}: {}: {}", path, operation, error));
}

fn hash_file_fingerprint(path: &Path) -> io::Result<String> {
    let hash = ImoHasher::new().sum_file(&path.to_string_lossy())?;
    let hash_hex = hash
        .to_le_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!("imohash:{hash_hex}"))
}

fn display_name(
    library: &MediaLibrarySettings,
    path: &str,
) -> String {
    if !library.name.trim().is_empty() {
        return library.name.trim().to_string();
    }

    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Unnamed library".into())
}

fn classify_file(path: &Path) -> FileKind {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    match extension.as_deref() {
        Some("mkv" | "mp4" | "avi" | "mov" | "wmv" | "m4v" | "webm" | "ts") => FileKind::Video,
        Some("mp3" | "flac" | "aac" | "wav" | "ogg" | "m4a" | "opus") => FileKind::Audio,
        Some("jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tiff") => FileKind::Image,
        Some("pdf" | "epub" | "cbz" | "cbr" | "mobi") => FileKind::Book,
        _ => FileKind::Other,
    }
}

fn should_include_library_item(
    path: &Path,
    kind: FileKind,
    library_kind: &MediaLibraryKind,
) -> bool {
    match library_kind {
        MediaLibraryKind::Movies | MediaLibraryKind::Shows | MediaLibraryKind::HomeVideos => {
            kind == FileKind::Video
        }
        MediaLibraryKind::Music => kind == FileKind::Audio,
        MediaLibraryKind::Photos => kind == FileKind::Image,
        MediaLibraryKind::Books => kind == FileKind::Book,
        MediaLibraryKind::Mixed => {
            matches!(
                kind,
                FileKind::Video | FileKind::Audio | FileKind::Image | FileKind::Book
            ) && !is_named_theme_asset(path)
        }
    }
}

fn is_named_theme_asset(path: &Path) -> bool {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("theme"))
        .unwrap_or(false)
}

fn normalize_relative_path(
    root: &Path,
    path: &Path,
) -> String {
    let relative: PathBuf = path.strip_prefix(root).unwrap_or(path).to_path_buf();
    relative.to_string_lossy().replace('\\', "/")
}

pub(crate) fn fallback_title_from_relative_path(relative_path: &str) -> String {
    Path::new(relative_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| relative_path.to_string())
}
