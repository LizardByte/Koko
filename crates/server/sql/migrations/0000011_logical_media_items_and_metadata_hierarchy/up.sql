PRAGMA foreign_keys=off;

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

INSERT INTO media_items (
    id,
    library_id,
    parent_id,
    identity_key,
    item_type,
    display_title,
    relative_path,
    media_kind,
    season_number,
    episode_number,
    child_count,
    playable,
    file_size,
    duration_ms,
    modified_at,
    created_at,
    updated_at
)
SELECT
    id,
    library_id,
    NULL,
    'legacy-file-' || id,
    CASE
        WHEN media_kind = 'audio' THEN 'track'
        WHEN media_kind = 'image' THEN 'photo'
        WHEN media_kind = 'book' THEN 'book'
        ELSE 'movie'
    END,
    COALESCE(display_title, relative_path),
    relative_path,
    media_kind,
    NULL,
    NULL,
    0,
    CASE
        WHEN media_kind IN ('video', 'audio') THEN TRUE
        ELSE FALSE
    END,
    file_size,
    duration_ms,
    modified_at,
    modified_at,
    modified_at
FROM media_files;

CREATE TABLE media_files_next (
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
    FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE SET NULL
);

INSERT INTO media_files_next (
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
    media_item_id
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
    metadata_updated_at,
    metadata_match_attempted_at,
    id
FROM media_files;

DROP TABLE media_files;
ALTER TABLE media_files_next RENAME TO media_files;

CREATE TABLE item_metadata_links_next (
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
    relation_kind TEXT NOT NULL DEFAULT 'primary',
    match_state TEXT NOT NULL,
    provider_payload_json TEXT DEFAULT NULL,
    cached_artwork_path TEXT DEFAULT NULL,
    cached_backdrop_path TEXT DEFAULT NULL,
    refresh_state TEXT NOT NULL DEFAULT 'fresh',
    refresh_interval_seconds BIGINT NOT NULL DEFAULT 604800,
    last_refreshed_at BIGINT DEFAULT NULL,
    next_refresh_at BIGINT DEFAULT NULL,
    refresh_error TEXT DEFAULT NULL,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE CASCADE,
    UNIQUE (media_item_id, provider_id, relation_kind)
);

INSERT INTO item_metadata_links_next (
    id,
    media_item_id,
    provider_id,
    external_id,
    title,
    overview,
    tagline,
    artwork_url,
    backdrop_url,
    release_year,
    media_type,
    relation_kind,
    match_state,
    provider_payload_json,
    cached_artwork_path,
    cached_backdrop_path,
    refresh_state,
    refresh_interval_seconds,
    last_refreshed_at,
    next_refresh_at,
    refresh_error,
    updated_at
)
SELECT
    id,
    media_file_id,
    provider_id,
    external_id,
    title,
    overview,
    NULL,
    artwork_url,
    backdrop_url,
    release_year,
    media_type,
    'primary',
    match_state,
    provider_payload_json,
    cached_artwork_path,
    cached_backdrop_path,
    'fresh',
    604800,
    updated_at,
    CASE
        WHEN updated_at IS NULL THEN NULL
        ELSE updated_at + 604800
    END,
    NULL,
    updated_at
FROM item_metadata_links;

DROP TABLE item_metadata_links;
ALTER TABLE item_metadata_links_next RENAME TO item_metadata_links;

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

CREATE TABLE playback_progress_next (
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

INSERT INTO playback_progress_next (
    id,
    user_id,
    media_item_id,
    position_ms,
    duration_ms,
    completed,
    updated_at
)
SELECT
    id,
    user_id,
    media_file_id,
    position_ms,
    duration_ms,
    completed,
    updated_at
FROM playback_progress;

DROP TABLE playback_progress;
ALTER TABLE playback_progress_next RENAME TO playback_progress;

CREATE INDEX idx_media_items_library_parent ON media_items (library_id, parent_id);
CREATE INDEX idx_media_items_identity_key ON media_items (identity_key);
CREATE INDEX idx_media_files_media_item_id ON media_files (media_item_id);
CREATE INDEX idx_item_metadata_links_media_item_id ON item_metadata_links (media_item_id);
CREATE INDEX idx_item_metadata_people_link_id ON item_metadata_people (metadata_link_id);
CREATE INDEX idx_item_metadata_collections_link_id ON item_metadata_collections (metadata_link_id);
CREATE INDEX idx_playback_progress_media_item_id ON playback_progress (media_item_id);

PRAGMA foreign_keys=on;

