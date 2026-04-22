PRAGMA foreign_keys=off;

CREATE TABLE playback_progress_prev (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER DEFAULT NULL,
    media_file_id INTEGER NOT NULL,
    position_ms BIGINT NOT NULL DEFAULT 0,
    duration_ms BIGINT DEFAULT NULL,
    completed BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (media_file_id) REFERENCES media_files(id) ON DELETE CASCADE,
    UNIQUE (user_id, media_file_id)
);

INSERT INTO playback_progress_prev (
    id,
    user_id,
    media_file_id,
    position_ms,
    duration_ms,
    completed,
    updated_at
)
SELECT
    progress.id,
    progress.user_id,
    COALESCE(files.id, progress.media_item_id),
    progress.position_ms,
    progress.duration_ms,
    progress.completed,
    progress.updated_at
FROM playback_progress AS progress
LEFT JOIN media_files AS files ON files.media_item_id = progress.media_item_id;

DROP TABLE playback_progress;
ALTER TABLE playback_progress_prev RENAME TO playback_progress;

DROP TABLE item_metadata_people;
DROP TABLE item_metadata_collections;

CREATE TABLE item_metadata_links_prev (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    media_file_id INTEGER NOT NULL,
    provider_id TEXT NOT NULL,
    external_id TEXT NOT NULL,
    title TEXT DEFAULT NULL,
    overview TEXT DEFAULT NULL,
    artwork_url TEXT DEFAULT NULL,
    backdrop_url TEXT DEFAULT NULL,
    release_year INTEGER DEFAULT NULL,
    media_type TEXT DEFAULT NULL,
    match_state TEXT NOT NULL,
    provider_payload_json TEXT DEFAULT NULL,
    cached_artwork_path TEXT DEFAULT NULL,
    cached_backdrop_path TEXT DEFAULT NULL,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (media_file_id) REFERENCES media_files(id) ON DELETE CASCADE
);

INSERT INTO item_metadata_links_prev (
    id,
    media_file_id,
    provider_id,
    external_id,
    title,
    overview,
    artwork_url,
    backdrop_url,
    release_year,
    media_type,
    match_state,
    provider_payload_json,
    cached_artwork_path,
    cached_backdrop_path,
    updated_at
)
SELECT
    links.id,
    COALESCE(files.id, links.media_item_id),
    links.provider_id,
    links.external_id,
    links.title,
    links.overview,
    links.artwork_url,
    links.backdrop_url,
    links.release_year,
    links.media_type,
    links.match_state,
    links.provider_payload_json,
    links.cached_artwork_path,
    links.cached_backdrop_path,
    links.updated_at
FROM item_metadata_links AS links
LEFT JOIN media_files AS files ON files.media_item_id = links.media_item_id;

DROP TABLE item_metadata_links;
ALTER TABLE item_metadata_links_prev RENAME TO item_metadata_links;

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
    FOREIGN KEY (library_id) REFERENCES media_libraries(id) ON DELETE CASCADE
);

INSERT INTO media_files_prev (
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
    metadata_match_attempted_at
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
    metadata_match_attempted_at
FROM media_files;

DROP TABLE media_files;
ALTER TABLE media_files_prev RENAME TO media_files;

DROP TABLE media_items;

PRAGMA foreign_keys=on;

