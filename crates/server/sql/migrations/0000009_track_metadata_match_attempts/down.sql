PRAGMA foreign_keys=off;

CREATE TABLE media_files_previous (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL,
    source_root_path TEXT NOT NULL DEFAULT '',
    relative_path TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    modified_at BIGINT DEFAULT NULL,
    media_kind TEXT NOT NULL,
    fingerprint_seed TEXT NOT NULL,
    display_title TEXT DEFAULT NULL,
    container TEXT DEFAULT NULL,
    duration_ms BIGINT DEFAULT NULL,
    bit_rate BIGINT DEFAULT NULL,
    width INTEGER DEFAULT NULL,
    height INTEGER DEFAULT NULL,
    video_codec TEXT DEFAULT NULL,
    audio_codec TEXT DEFAULT NULL,
    metadata_json TEXT DEFAULT NULL,
    metadata_updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (library_id) REFERENCES media_libraries(id) ON DELETE CASCADE,
    UNIQUE (library_id, source_root_path, relative_path)
);

INSERT INTO media_files_previous (
    id,
    library_id,
    source_root_path,
    relative_path,
    file_size,
    modified_at,
    media_kind,
    fingerprint_seed,
    display_title,
    container,
    duration_ms,
    bit_rate,
    width,
    height,
    video_codec,
    audio_codec,
    metadata_json,
    metadata_updated_at
)
SELECT
    id,
    library_id,
    source_root_path,
    relative_path,
    file_size,
    modified_at,
    media_kind,
    fingerprint_seed,
    display_title,
    container,
    duration_ms,
    bit_rate,
    width,
    height,
    video_codec,
    audio_codec,
    metadata_json,
    metadata_updated_at
FROM media_files;

DROP TABLE media_files;
ALTER TABLE media_files_previous RENAME TO media_files;

PRAGMA foreign_keys=on;

