PRAGMA foreign_keys=off;

ALTER TABLE item_metadata_collections RENAME TO item_metadata_collections_old;
ALTER TABLE item_metadata_people RENAME TO item_metadata_people_old;
ALTER TABLE item_metadata_links RENAME TO item_metadata_links_old;
ALTER TABLE playback_progress RENAME TO playback_progress_old;
ALTER TABLE media_files RENAME TO media_files_old;
ALTER TABLE media_items RENAME TO media_items_old;
ALTER TABLE scan_state RENAME TO scan_state_old;
ALTER TABLE media_libraries RENAME TO media_libraries_old;

DROP INDEX IF EXISTS idx_media_items_library_parent;
DROP INDEX IF EXISTS idx_media_items_identity_key;
DROP INDEX IF EXISTS idx_media_files_media_item_id;
DROP INDEX IF EXISTS idx_item_metadata_links_media_item_id;
DROP INDEX IF EXISTS idx_item_metadata_people_link_id;
DROP INDEX IF EXISTS idx_item_metadata_collections_link_id;
DROP INDEX IF EXISTS idx_playback_progress_media_item_id;

CREATE TABLE media_libraries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    path TEXT NOT NULL UNIQUE,
    paths_json TEXT NOT NULL DEFAULT '[]',
    kind TEXT NOT NULL,
    recursive BOOLEAN NOT NULL DEFAULT TRUE,
    metadata_providers_json TEXT NOT NULL DEFAULT '["tmdb"]'
);

INSERT INTO media_libraries (
    id,
    name,
    path,
    paths_json,
    kind,
    recursive,
    metadata_providers_json
)
SELECT
    id,
    name,
    path,
    paths_json,
    kind,
    recursive,
    metadata_providers_json
FROM media_libraries_old;

CREATE TABLE scan_state (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL UNIQUE,
    last_status TEXT NOT NULL,
    last_error TEXT DEFAULT NULL,
    scan_revision BIGINT NOT NULL,
    last_scanned_at BIGINT DEFAULT NULL,
    total_files BIGINT NOT NULL DEFAULT 0,
    video_files BIGINT NOT NULL DEFAULT 0,
    audio_files BIGINT NOT NULL DEFAULT 0,
    image_files BIGINT NOT NULL DEFAULT 0,
    book_files BIGINT NOT NULL DEFAULT 0,
    other_files BIGINT NOT NULL DEFAULT 0,
    FOREIGN KEY (library_id) REFERENCES media_libraries(id) ON DELETE CASCADE
);

INSERT INTO scan_state SELECT * FROM scan_state_old;

CREATE TABLE media_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL,
    parent_id INTEGER DEFAULT NULL,
    identity_key TEXT NOT NULL UNIQUE,
    item_type TEXT NOT NULL,
    display_title TEXT NOT NULL,
    relative_path TEXT DEFAULT NULL,
    media_kind TEXT DEFAULT NULL,
    season_number INTEGER DEFAULT NULL,
    episode_number INTEGER DEFAULT NULL,
    child_count INTEGER NOT NULL DEFAULT 0,
    playable BOOLEAN NOT NULL DEFAULT FALSE,
    file_size BIGINT DEFAULT NULL,
    duration_ms BIGINT DEFAULT NULL,
    modified_at BIGINT DEFAULT NULL,
    created_at BIGINT DEFAULT NULL,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (library_id) REFERENCES media_libraries(id) ON DELETE CASCADE,
    FOREIGN KEY (parent_id) REFERENCES media_items(id) ON DELETE CASCADE
);

INSERT INTO media_items SELECT * FROM media_items_old;

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
    FOREIGN KEY (library_id) REFERENCES media_libraries(id) ON DELETE CASCADE,
    FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE SET NULL,
    UNIQUE (library_id, source_root_path, relative_path)
);

INSERT INTO media_files SELECT * FROM media_files_old;

CREATE TABLE item_metadata_links (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    media_item_id INTEGER NOT NULL,
    provider_id TEXT NOT NULL,
    external_id TEXT NOT NULL,
    title TEXT DEFAULT NULL,
    overview TEXT DEFAULT NULL,
    tagline TEXT DEFAULT NULL,
    artwork_url TEXT DEFAULT NULL,
    backdrop_url TEXT DEFAULT NULL,
    release_year INTEGER DEFAULT NULL,
    media_type TEXT DEFAULT NULL,
    relation_kind TEXT NOT NULL,
    match_state TEXT NOT NULL,
    provider_payload_json TEXT DEFAULT NULL,
    cached_artwork_path TEXT DEFAULT NULL,
    cached_backdrop_path TEXT DEFAULT NULL,
    refresh_state TEXT NOT NULL,
    refresh_interval_seconds BIGINT NOT NULL,
    last_refreshed_at BIGINT DEFAULT NULL,
    next_refresh_at BIGINT DEFAULT NULL,
    refresh_error TEXT DEFAULT NULL,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE CASCADE,
    UNIQUE (media_item_id, provider_id, relation_kind)
);

INSERT INTO item_metadata_links SELECT * FROM item_metadata_links_old;

CREATE TABLE item_metadata_people (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metadata_link_id INTEGER NOT NULL,
    external_id TEXT DEFAULT NULL,
    name TEXT NOT NULL,
    role TEXT DEFAULT NULL,
    department TEXT DEFAULT NULL,
    character_name TEXT DEFAULT NULL,
    profile_url TEXT DEFAULT NULL,
    image_url TEXT DEFAULT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (metadata_link_id) REFERENCES item_metadata_links(id) ON DELETE CASCADE
);

INSERT INTO item_metadata_people SELECT * FROM item_metadata_people_old;

CREATE TABLE item_metadata_collections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metadata_link_id INTEGER NOT NULL,
    provider_id TEXT NOT NULL,
    external_id TEXT NOT NULL,
    name TEXT NOT NULL,
    overview TEXT DEFAULT NULL,
    artwork_url TEXT DEFAULT NULL,
    backdrop_url TEXT DEFAULT NULL,
    provider_payload_json TEXT DEFAULT NULL,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (metadata_link_id) REFERENCES item_metadata_links(id) ON DELETE CASCADE,
    UNIQUE (metadata_link_id, provider_id, external_id)
);

INSERT INTO item_metadata_collections SELECT * FROM item_metadata_collections_old;

CREATE TABLE playback_progress (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER DEFAULT NULL,
    media_item_id INTEGER NOT NULL,
    position_ms BIGINT NOT NULL DEFAULT 0,
    duration_ms BIGINT DEFAULT NULL,
    completed BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE CASCADE,
    UNIQUE (user_id, media_item_id)
);

INSERT INTO playback_progress SELECT * FROM playback_progress_old;

CREATE INDEX idx_media_items_library_parent ON media_items (library_id, parent_id);
CREATE INDEX idx_media_items_identity_key ON media_items (identity_key);
CREATE INDEX idx_media_files_media_item_id ON media_files (media_item_id);
CREATE INDEX idx_item_metadata_links_media_item_id ON item_metadata_links (media_item_id);
CREATE INDEX idx_item_metadata_people_link_id ON item_metadata_people (metadata_link_id);
CREATE INDEX idx_item_metadata_collections_link_id ON item_metadata_collections (metadata_link_id);
CREATE INDEX idx_playback_progress_media_item_id ON playback_progress (media_item_id);

DROP TABLE item_metadata_collections_old;
DROP TABLE item_metadata_people_old;
DROP TABLE item_metadata_links_old;
DROP TABLE playback_progress_old;
DROP TABLE media_files_old;
DROP TABLE media_items_old;
DROP TABLE scan_state_old;
DROP TABLE media_libraries_old;

PRAGMA foreign_keys=on;
