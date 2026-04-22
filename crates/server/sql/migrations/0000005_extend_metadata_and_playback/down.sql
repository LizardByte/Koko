PRAGMA foreign_keys=off;

DROP TABLE IF EXISTS playback_progress;

CREATE TABLE item_metadata_links_previous (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    media_file_id INTEGER NOT NULL,
    provider_id TEXT NOT NULL,
    external_id TEXT NOT NULL,
    title TEXT DEFAULT NULL,
    overview TEXT DEFAULT NULL,
    artwork_url TEXT DEFAULT NULL,
    backdrop_url TEXT DEFAULT NULL,
    release_year INTEGER DEFAULT NULL,
    match_state TEXT NOT NULL DEFAULT 'unmatched',
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (media_file_id) REFERENCES media_files(id) ON DELETE CASCADE,
    UNIQUE (media_file_id, provider_id)
);

INSERT INTO item_metadata_links_previous (
    id,
    media_file_id,
    provider_id,
    external_id,
    title,
    overview,
    artwork_url,
    backdrop_url,
    release_year,
    match_state,
    updated_at
)
SELECT
    id,
    media_file_id,
    provider_id,
    external_id,
    title,
    overview,
    artwork_url,
    backdrop_url,
    release_year,
    match_state,
    updated_at
FROM item_metadata_links;

DROP TABLE item_metadata_links;
ALTER TABLE item_metadata_links_previous RENAME TO item_metadata_links;

PRAGMA foreign_keys=on;

