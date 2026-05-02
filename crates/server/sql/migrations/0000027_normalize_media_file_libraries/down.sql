PRAGMA foreign_keys=off;

ALTER TABLE media_files RENAME TO media_files_physical;
ALTER TABLE media_file_libraries RENAME TO media_file_libraries_old;

CREATE TABLE media_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL,
    source_root_path TEXT NOT NULL,
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
    metadata_match_attempted_at BIGINT DEFAULT NULL,
    media_item_id INTEGER DEFAULT NULL,
    missing_since BIGINT DEFAULT NULL,
    deleted_at BIGINT DEFAULT NULL,
    FOREIGN KEY (library_id) REFERENCES media_libraries(id) ON DELETE CASCADE,
    FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE SET NULL,
    UNIQUE (library_id, source_root_path, relative_path)
);

INSERT INTO media_files (
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
    metadata_updated_at,
    metadata_match_attempted_at,
    media_item_id,
    missing_since,
    deleted_at
)
SELECT
    memberships.id,
    memberships.library_id,
    memberships.source_root_path,
    memberships.relative_path,
    files.file_size,
    files.modified_at,
    files.media_kind,
    files.fingerprint_seed,
    memberships.display_title,
    files.container,
    files.duration_ms,
    files.bit_rate,
    files.width,
    files.height,
    files.video_codec,
    files.audio_codec,
    files.metadata_json,
    files.metadata_updated_at,
    memberships.metadata_match_attempted_at,
    memberships.media_item_id,
    memberships.missing_since,
    memberships.deleted_at
FROM media_file_libraries_old AS memberships
INNER JOIN media_files_physical AS files ON files.id = memberships.media_file_id;

CREATE INDEX idx_media_files_media_item_id ON media_files (media_item_id);
CREATE INDEX idx_media_files_missing_since ON media_files (missing_since);
CREATE INDEX idx_media_files_deleted_at ON media_files (deleted_at);

DROP TABLE media_file_libraries_old;
DROP TABLE media_files_physical;

PRAGMA foreign_keys=on;
