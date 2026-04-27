PRAGMA foreign_keys=off;

DELETE FROM item_metadata_external_ids
WHERE source = 'themoviedb'
  AND EXISTS (
      SELECT 1
      FROM item_metadata_external_ids AS canonical
      WHERE canonical.metadata_link_id = item_metadata_external_ids.metadata_link_id
        AND canonical.source = 'tmdb'
  );

UPDATE item_metadata_external_ids
SET source = 'tmdb'
WHERE source = 'themoviedb';

DROP INDEX IF EXISTS idx_item_metadata_links_media_item_id;

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
    logo_url TEXT DEFAULT NULL,
    cached_logo_path TEXT DEFAULT NULL,
    genres_json TEXT DEFAULT NULL,
    rating FLOAT DEFAULT NULL,
    content_rating TEXT DEFAULT NULL,
    trailer_title TEXT DEFAULT NULL,
    trailer_url TEXT DEFAULT NULL,
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
    backdrop_url, release_year, media_type, relation_kind, match_state, logo_url,
    cached_logo_path, genres_json, rating, content_rating, trailer_title, trailer_url,
    locale_key, provider_locale_key, cached_artwork_path, cached_backdrop_path,
    refresh_state, refresh_interval_seconds, last_refreshed_at, next_refresh_at,
    refresh_error, updated_at
)
SELECT
    id, media_item_id, provider_id, external_id, title, overview, tagline, artwork_url,
    backdrop_url, release_year, media_type, relation_kind, match_state, logo_url,
    CASE
        WHEN cached_logo_path IS NULL THEN NULL
        WHEN instr(cached_logo_path, '/metadata/') > 0 THEN substr(cached_logo_path, instr(cached_logo_path, '/metadata/') + 1)
        WHEN instr(cached_logo_path, char(92) || 'metadata' || char(92)) > 0 THEN replace(substr(cached_logo_path, instr(cached_logo_path, char(92) || 'metadata' || char(92)) + 1), char(92), '/')
        ELSE cached_logo_path
    END,
    genres_json, rating, content_rating, trailer_title, trailer_url,
    locale_key, provider_locale_key,
    CASE
        WHEN cached_artwork_path IS NULL THEN NULL
        WHEN instr(cached_artwork_path, '/metadata/') > 0 THEN substr(cached_artwork_path, instr(cached_artwork_path, '/metadata/') + 1)
        WHEN instr(cached_artwork_path, char(92) || 'metadata' || char(92)) > 0 THEN replace(substr(cached_artwork_path, instr(cached_artwork_path, char(92) || 'metadata' || char(92)) + 1), char(92), '/')
        ELSE cached_artwork_path
    END,
    CASE
        WHEN cached_backdrop_path IS NULL THEN NULL
        WHEN instr(cached_backdrop_path, '/metadata/') > 0 THEN substr(cached_backdrop_path, instr(cached_backdrop_path, '/metadata/') + 1)
        WHEN instr(cached_backdrop_path, char(92) || 'metadata' || char(92)) > 0 THEN replace(substr(cached_backdrop_path, instr(cached_backdrop_path, char(92) || 'metadata' || char(92)) + 1), char(92), '/')
        ELSE cached_backdrop_path
    END,
    refresh_state, refresh_interval_seconds, last_refreshed_at, next_refresh_at,
    refresh_error, updated_at
FROM item_metadata_links;

DROP TABLE item_metadata_links;
ALTER TABLE item_metadata_links_next RENAME TO item_metadata_links;

CREATE INDEX idx_item_metadata_links_media_item_id ON item_metadata_links (media_item_id);

DROP INDEX IF EXISTS idx_item_metadata_collections_link_id;

CREATE TABLE item_metadata_collections_next (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metadata_link_id INTEGER NOT NULL,
    provider_id TEXT NOT NULL,
    external_id TEXT NOT NULL,
    name TEXT NOT NULL,
    overview TEXT DEFAULT NULL,
    artwork_url TEXT DEFAULT NULL,
    backdrop_url TEXT DEFAULT NULL,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (metadata_link_id) REFERENCES item_metadata_links(id) ON DELETE CASCADE,
    UNIQUE (metadata_link_id, provider_id, external_id)
);

INSERT INTO item_metadata_collections_next (
    id, metadata_link_id, provider_id, external_id, name, overview, artwork_url,
    backdrop_url, updated_at
)
SELECT
    id, metadata_link_id, provider_id, external_id, name, overview, artwork_url,
    backdrop_url, updated_at
FROM item_metadata_collections;

DROP TABLE item_metadata_collections;
ALTER TABLE item_metadata_collections_next RENAME TO item_metadata_collections;

CREATE INDEX idx_item_metadata_collections_link_id
    ON item_metadata_collections (metadata_link_id);

DROP INDEX IF EXISTS idx_metadata_people_provider_identity_locale;
DROP INDEX IF EXISTS idx_metadata_people_provider_identity;

CREATE TABLE metadata_people_next (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id TEXT NOT NULL,
    external_id TEXT DEFAULT NULL,
    identity_key TEXT NOT NULL,
    locale_key TEXT NOT NULL DEFAULT 'en-US',
    name TEXT NOT NULL,
    known_for_json TEXT DEFAULT NULL,
    biography TEXT DEFAULT NULL,
    gender TEXT DEFAULT NULL,
    birthday TEXT DEFAULT NULL,
    deathday TEXT DEFAULT NULL,
    birth_place TEXT DEFAULT NULL,
    profile_url TEXT DEFAULT NULL,
    image_url TEXT DEFAULT NULL,
    cached_image_path TEXT DEFAULT NULL,
    updated_at BIGINT DEFAULT NULL,
    UNIQUE (provider_id, identity_key, locale_key)
);

INSERT INTO metadata_people_next (
    id, provider_id, external_id, identity_key, locale_key, name, known_for_json,
    biography, gender, birthday, deathday, birth_place, profile_url, image_url,
    cached_image_path, updated_at
)
SELECT
    id, provider_id, external_id, identity_key, locale_key, name, known_for_json,
    biography, gender, birthday, deathday, birth_place, profile_url, image_url,
    CASE
        WHEN cached_image_path IS NULL THEN NULL
        WHEN instr(cached_image_path, '/metadata/') > 0 THEN substr(cached_image_path, instr(cached_image_path, '/metadata/') + 1)
        WHEN instr(cached_image_path, char(92) || 'metadata' || char(92)) > 0 THEN replace(substr(cached_image_path, instr(cached_image_path, char(92) || 'metadata' || char(92)) + 1), char(92), '/')
        ELSE cached_image_path
    END,
    updated_at
FROM metadata_people;

DROP TABLE metadata_people;
ALTER TABLE metadata_people_next RENAME TO metadata_people;

CREATE INDEX idx_metadata_people_provider_identity
    ON metadata_people (provider_id, identity_key);
CREATE INDEX idx_metadata_people_provider_identity_locale
    ON metadata_people (provider_id, identity_key, locale_key);

PRAGMA foreign_keys=on;
