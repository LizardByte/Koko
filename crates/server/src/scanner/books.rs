//! Book scanner rules.

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
            MediaLibraryScanner::Books,
            MediaLibraryKind::Books,
            |_, title| title.to_string(),
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
            MediaLibraryScanner::Books,
            MediaLibraryKind::Books,
            |_, title| title.to_string(),
        ),
        sink,
    )
}
