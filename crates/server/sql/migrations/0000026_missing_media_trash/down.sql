DROP INDEX IF EXISTS idx_media_items_deleted_at;
DROP INDEX IF EXISTS idx_media_items_missing_since;
DROP INDEX IF EXISTS idx_media_files_deleted_at;
DROP INDEX IF EXISTS idx_media_files_missing_since;

CREATE TABLE media_items_prev (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL,
    parent_id INTEGER DEFAULT NULL,
    identity_key TEXT NOT NULL,
    item_type TEXT NOT NULL,
    display_title TEXT NOT NULL,
    relative_path TEXT DEFAULT NULL,
    media_kind TEXT DEFAULT NULL,
    season_number INTEGER DEFAULT NULL,
    episode_number INTEGER DEFAULT NULL,
    child_count INTEGER NOT NULL DEFAULT 0,
    playable BOOLEAN NOT NULL DEFAULT 0,
    file_size BIGINT DEFAULT NULL,
    duration_ms BIGINT DEFAULT NULL,
    modified_at BIGINT DEFAULT NULL,
    created_at BIGINT DEFAULT NULL,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (library_id) REFERENCES media_libraries(id) ON DELETE CASCADE,
    FOREIGN KEY (parent_id) REFERENCES media_items_prev(id) ON DELETE CASCADE
);

INSERT INTO media_items_prev (
    id, library_id, parent_id, identity_key, item_type, display_title,
    relative_path, media_kind, season_number, episode_number, child_count,
    playable, file_size, duration_ms, modified_at, created_at, updated_at
)
SELECT
    id, library_id, parent_id, identity_key, item_type, display_title,
    relative_path, media_kind, season_number, episode_number, child_count,
    playable, file_size, duration_ms, modified_at, created_at, updated_at
FROM media_items;

CREATE TABLE media_files_prev (
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
    FOREIGN KEY (library_id) REFERENCES media_libraries(id) ON DELETE CASCADE,
    FOREIGN KEY (media_item_id) REFERENCES media_items_prev(id) ON DELETE SET NULL,
    UNIQUE (library_id, source_root_path, relative_path)
);

INSERT INTO media_files_prev (
    id, library_id, source_root_path, relative_path, file_size, modified_at,
    media_kind, fingerprint_seed, display_title, container, duration_ms,
    bit_rate, width, height, video_codec, audio_codec, metadata_json,
    metadata_updated_at, metadata_match_attempted_at, media_item_id
)
SELECT
    id, library_id, source_root_path, relative_path, file_size, modified_at,
    media_kind, fingerprint_seed, display_title, container, duration_ms,
    bit_rate, width, height, video_codec, audio_codec, metadata_json,
    metadata_updated_at, metadata_match_attempted_at, media_item_id
FROM media_files;

DROP TABLE media_files;
ALTER TABLE media_files_prev RENAME TO media_files;
DROP TABLE media_items;
ALTER TABLE media_items_prev RENAME TO media_items;

CREATE INDEX idx_media_items_library_parent ON media_items (library_id, parent_id);
CREATE INDEX idx_media_items_identity_key ON media_items (identity_key);
CREATE INDEX idx_media_files_media_item_id ON media_files (media_item_id);
