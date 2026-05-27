PRAGMA foreign_keys=off;

DROP INDEX IF EXISTS idx_metadata_people_provider_identity_locale;
DROP INDEX IF EXISTS idx_metadata_people_provider_identity;

CREATE TABLE metadata_people_next (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id TEXT NOT NULL,
    external_id TEXT DEFAULT NULL,
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
    updated_at BIGINT DEFAULT NULL
);

INSERT INTO metadata_people_next (
    id, provider_id, external_id, locale_key, name, known_for_json,
    biography, gender, birthday, deathday, birth_place, profile_url, image_url,
    cached_image_path, updated_at
)
SELECT
    id, provider_id, external_id, locale_key, name, known_for_json,
    biography, gender, birthday, deathday, birth_place, profile_url, image_url,
    cached_image_path, updated_at
FROM metadata_people;

DROP TABLE metadata_people;
ALTER TABLE metadata_people_next RENAME TO metadata_people;

CREATE UNIQUE INDEX idx_metadata_people_provider_external_locale
    ON metadata_people (provider_id, external_id, locale_key)
    WHERE external_id IS NOT NULL;
CREATE UNIQUE INDEX idx_metadata_people_provider_name_locale_without_external
    ON metadata_people (provider_id, lower(name), locale_key)
    WHERE external_id IS NULL;
CREATE INDEX idx_metadata_people_provider_external
    ON metadata_people (provider_id, external_id);
CREATE INDEX idx_metadata_people_provider_locale
    ON metadata_people (provider_id, locale_key);

PRAGMA foreign_keys=on;
