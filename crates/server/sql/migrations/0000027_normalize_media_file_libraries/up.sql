PRAGMA foreign_keys=off;

ALTER TABLE media_files RENAME TO media_files_old;

CREATE TABLE media_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    modified_at BIGINT DEFAULT NULL,
    media_kind TEXT NOT NULL,
    fingerprint_seed TEXT NOT NULL,
    container TEXT DEFAULT NULL,
    duration_ms BIGINT DEFAULT NULL,
    bit_rate BIGINT DEFAULT NULL,
    width INTEGER DEFAULT NULL,
    height INTEGER DEFAULT NULL,
    video_codec TEXT DEFAULT NULL,
    audio_codec TEXT DEFAULT NULL,
    metadata_json TEXT DEFAULT NULL,
    metadata_updated_at BIGINT DEFAULT NULL,
    UNIQUE (path)
);

CREATE TABLE media_file_libraries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    media_file_id INTEGER NOT NULL,
    library_id INTEGER NOT NULL,
    source_root_path TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    display_title TEXT DEFAULT NULL,
    metadata_match_attempted_at BIGINT DEFAULT NULL,
    media_item_id INTEGER DEFAULT NULL,
    missing_since BIGINT DEFAULT NULL,
    deleted_at BIGINT DEFAULT NULL,
    FOREIGN KEY (media_file_id) REFERENCES media_files(id) ON DELETE CASCADE,
    FOREIGN KEY (library_id) REFERENCES media_libraries(id) ON DELETE CASCADE,
    FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE SET NULL,
    UNIQUE (library_id, source_root_path, relative_path)
);

INSERT INTO media_files (
    path,
    file_size,
    modified_at,
    media_kind,
    fingerprint_seed,
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
WITH source AS (
    SELECT
        id,
        source_root_path ||
            CASE
                WHEN source_root_path = '' OR relative_path = '' THEN ''
                WHEN substr(source_root_path, -1, 1) IN ('/', '\') THEN ''
                WHEN instr(source_root_path, '\') > 0 THEN '\'
                ELSE '/'
            END ||
            relative_path AS physical_path,
        file_size,
        modified_at,
        media_kind,
        fingerprint_seed,
        container,
        duration_ms,
        bit_rate,
        width,
        height,
        video_codec,
        audio_codec,
        metadata_json,
        metadata_updated_at
    FROM media_files_old
),
selected AS (
    SELECT MIN(id) AS id
    FROM source
    GROUP BY physical_path
)
SELECT
    source.physical_path,
    source.file_size,
    source.modified_at,
    source.media_kind,
    source.fingerprint_seed,
    source.container,
    source.duration_ms,
    source.bit_rate,
    source.width,
    source.height,
    source.video_codec,
    source.audio_codec,
    source.metadata_json,
    source.metadata_updated_at
FROM source
INNER JOIN selected ON selected.id = source.id;

INSERT INTO media_file_libraries (
    id,
    media_file_id,
    library_id,
    source_root_path,
    relative_path,
    display_title,
    metadata_match_attempted_at,
    media_item_id,
    missing_since,
    deleted_at
)
WITH source AS (
    SELECT
        id,
        library_id,
        source_root_path,
        relative_path,
        display_title,
        metadata_match_attempted_at,
        media_item_id,
        missing_since,
        deleted_at,
        source_root_path ||
            CASE
                WHEN source_root_path = '' OR relative_path = '' THEN ''
                WHEN substr(source_root_path, -1, 1) IN ('/', '\') THEN ''
                WHEN instr(source_root_path, '\') > 0 THEN '\'
                ELSE '/'
            END ||
            relative_path AS physical_path
    FROM media_files_old
)
SELECT
    source.id,
    files.id,
    source.library_id,
    source.source_root_path,
    source.relative_path,
    source.display_title,
    source.metadata_match_attempted_at,
    source.media_item_id,
    source.missing_since,
    source.deleted_at
FROM source
INNER JOIN media_files AS files ON files.path = source.physical_path;

CREATE INDEX idx_media_file_libraries_media_file_id ON media_file_libraries (media_file_id);
CREATE INDEX idx_media_file_libraries_library_id ON media_file_libraries (library_id);
CREATE INDEX idx_media_file_libraries_media_item_id ON media_file_libraries (media_item_id);
CREATE INDEX idx_media_file_libraries_missing_since ON media_file_libraries (missing_since);
CREATE INDEX idx_media_file_libraries_deleted_at ON media_file_libraries (deleted_at);

DROP INDEX IF EXISTS idx_media_files_media_item_id;
DROP INDEX IF EXISTS idx_media_files_missing_since;
DROP INDEX IF EXISTS idx_media_files_deleted_at;

DROP TABLE media_files_old;

PRAGMA foreign_keys=on;
