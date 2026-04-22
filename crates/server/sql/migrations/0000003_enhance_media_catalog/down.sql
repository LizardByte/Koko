PRAGMA foreign_keys=off;

CREATE TABLE media_files_previous (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL,
    relative_path TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    modified_at BIGINT DEFAULT NULL,
    media_kind TEXT NOT NULL,
    fingerprint_seed TEXT NOT NULL,
    FOREIGN KEY (library_id) REFERENCES media_libraries(id) ON DELETE CASCADE,
    UNIQUE (library_id, relative_path)
);

INSERT INTO media_files_previous (
    id,
    library_id,
    relative_path,
    file_size,
    modified_at,
    media_kind,
    fingerprint_seed
)
SELECT
    id,
    library_id,
    relative_path,
    file_size,
    modified_at,
    media_kind,
    fingerprint_seed
FROM media_files;

DROP TABLE media_files;
ALTER TABLE media_files_previous RENAME TO media_files;

CREATE TABLE scan_state_previous (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL UNIQUE,
    last_status TEXT NOT NULL DEFAULT 'never_scanned',
    last_error TEXT DEFAULT NULL,
    total_files BIGINT NOT NULL DEFAULT 0,
    video_files BIGINT NOT NULL DEFAULT 0,
    audio_files BIGINT NOT NULL DEFAULT 0,
    image_files BIGINT NOT NULL DEFAULT 0,
    book_files BIGINT NOT NULL DEFAULT 0,
    other_files BIGINT NOT NULL DEFAULT 0,
    FOREIGN KEY (library_id) REFERENCES media_libraries(id) ON DELETE CASCADE
);

INSERT INTO scan_state_previous (
    id,
    library_id,
    last_status,
    last_error,
    total_files,
    video_files,
    audio_files,
    image_files,
    book_files,
    other_files
)
SELECT
    id,
    library_id,
    last_status,
    last_error,
    total_files,
    video_files,
    audio_files,
    image_files,
    book_files,
    other_files
FROM scan_state;

DROP TABLE scan_state;
ALTER TABLE scan_state_previous RENAME TO scan_state;

PRAGMA foreign_keys=on;
