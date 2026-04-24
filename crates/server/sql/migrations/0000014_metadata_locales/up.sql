PRAGMA foreign_keys=off;

ALTER TABLE users ADD COLUMN preferred_metadata_languages_json TEXT NOT NULL DEFAULT '["en-US"]';

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
    relation_kind TEXT NOT NULL,
    match_state TEXT NOT NULL,
    provider_payload_json TEXT DEFAULT NULL,
    locale_key TEXT NOT NULL DEFAULT 'en-US',
    provider_locale_key TEXT DEFAULT NULL,
    cached_artwork_path TEXT DEFAULT NULL,
    cached_backdrop_path TEXT DEFAULT NULL,
    refresh_state TEXT NOT NULL,
    refresh_interval_seconds BIGINT NOT NULL,
    last_refreshed_at BIGINT DEFAULT NULL,
    next_refresh_at BIGINT DEFAULT NULL,
    refresh_error TEXT DEFAULT NULL,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE CASCADE,
    UNIQUE (media_item_id, provider_id, relation_kind, locale_key)
);

INSERT INTO item_metadata_links_next (
    id, media_item_id, provider_id, external_id, title, overview, tagline, artwork_url,
    backdrop_url, release_year, media_type, relation_kind, match_state, provider_payload_json,
    locale_key, provider_locale_key, cached_artwork_path, cached_backdrop_path, refresh_state,
    refresh_interval_seconds, last_refreshed_at, next_refresh_at, refresh_error, updated_at
)
SELECT
    id, media_item_id, provider_id, external_id, title, overview, tagline, artwork_url,
    backdrop_url, release_year, media_type, relation_kind, match_state, provider_payload_json,
    'en-US', NULL, cached_artwork_path, cached_backdrop_path, refresh_state,
    refresh_interval_seconds, last_refreshed_at, next_refresh_at, refresh_error, updated_at
FROM item_metadata_links;

DROP TABLE item_metadata_links;
ALTER TABLE item_metadata_links_next RENAME TO item_metadata_links;

CREATE INDEX idx_item_metadata_links_media_item_id ON item_metadata_links (media_item_id);

PRAGMA foreign_keys=on;
