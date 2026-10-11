//! Regression tests for bounded backwards log reads.

use std::io::{
    Cursor,
    Read,
    Seek,
    SeekFrom,
};

use super::read_filtered_log_entries;

struct CountingReader {
    cursor: Cursor<Vec<u8>>,
    bytes_read: usize,
}

impl Read for CountingReader {
    fn read(
        &mut self,
        buffer: &mut [u8],
    ) -> std::io::Result<usize> {
        let count = self.cursor.read(buffer)?;
        self.bytes_read += count;
        Ok(count)
    }
}

impl Seek for CountingReader {
    fn seek(
        &mut self,
        position: SeekFrom,
    ) -> std::io::Result<u64> {
        self.cursor.seek(position)
    }
}

#[test]
fn reads_only_the_tail_when_the_limit_is_satisfied() {
    let old = "2026-10-11T10:00:00Z [DEBUG] [koko] [src/old.rs:1] Old entry\n";
    let recent = "2026-10-11T11:00:00Z [INFO] [koko] [src/new.rs:2] Recent entry\n";
    let mut reader = CountingReader {
        cursor: Cursor::new(format!("{}{recent}", old.repeat(30_000)).into_bytes()),
        bytes_read: 0,
    };
    let entries = read_filtered_log_entries(&mut reader, None, None, None, None, None, 1).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].message, "Recent entry");
    assert_eq!(reader.bytes_read, 64 * 1024);
}

#[test]
fn preserves_long_utf8_lines_crlf_and_unterminated_final_lines() {
    let message = "é漢".repeat(20_000);
    let contents = format!(
        "2026-10-11T10:00:00Z [WARN] [koko] [src/first.rs:1] {message}\r\nmalformed \
         line\r\n2026-10-11T11:00:00Z [INFO] [koko] [src/last.rs:2] Final entry"
    );
    let entries =
        read_filtered_log_entries(Cursor::new(contents), None, None, None, None, None, 10).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].message, "Final entry");
    assert_eq!(entries[1].message, message);
    assert_eq!(entries[1].line_number, Some(1));
}

#[test]
fn filters_before_limiting_and_keeps_newest_first() {
    let contents = concat!(
        "2026-10-11T10:00:00Z [WARN] [koko::media] [src\\media.rs:1] oldest\n",
        "2026-10-11T11:00:00Z [WARN] [koko::media] [src\\media.rs:2] wanted\n",
        "2026-10-11T12:00:00Z [WARN] [koko::media] [src\\media.rs:3] newest match\n",
        "2026-10-11T13:00:00Z [INFO] [koko::web] [src\\web.rs:4] excluded\n",
        "invalid-time [WARN] [koko::media] [src\\media.rs:5] invalid timestamp\n",
    );
    let entries = read_filtered_log_entries(
        Cursor::new(contents),
        Some(" warn "),
        Some(" MEDIA "),
        Some("SRC/MEDIA.RS"),
        Some("2026-10-11T11:00:00Z"),
        Some("2026-10-11T12:00:00Z"),
        2,
    )
    .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].message, "newest match");
    assert_eq!(entries[1].message, "wanted");
    assert_eq!(entries[0].source_file_path, "src/media.rs");
}

#[test]
fn handles_empty_files_no_matches_and_zero_limit() {
    for contents in ["", "malformed\n", "\n\r\n"] {
        assert!(
            read_filtered_log_entries(Cursor::new(contents), None, None, None, None, None, 10)
                .unwrap()
                .is_empty()
        );
    }
    let contents = "2026-10-11T10:00:00Z [INFO] [koko] [src/main.rs:1] Hello";
    assert!(
        read_filtered_log_entries(Cursor::new(contents), None, None, None, None, None, 0)
            .unwrap()
            .is_empty()
    );
    assert!(
        read_filtered_log_entries(
            Cursor::new(contents),
            Some("ERROR"),
            None,
            None,
            None,
            None,
            10
        )
        .unwrap()
        .is_empty()
    );
}
