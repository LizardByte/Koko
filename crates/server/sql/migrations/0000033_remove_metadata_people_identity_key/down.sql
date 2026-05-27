PRAGMA foreign_keys=off;

DROP INDEX IF EXISTS idx_metadata_people_provider_locale;
DROP INDEX IF EXISTS idx_metadata_people_provider_external;
DROP INDEX IF EXISTS idx_metadata_people_provider_name_locale_without_external;
DROP INDEX IF EXISTS idx_metadata_people_provider_external_locale;

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
    id, provider_id, external_id,
    COALESCE(NULLIF(trim(external_id), ''), 'name:' || lower(trim(name))),
    locale_key, name, known_for_json,
    biography, gender, birthday, deathday, birth_place, profile_url, image_url,
    cached_image_path, updated_at
FROM metadata_people;

DROP TABLE metadata_people;
ALTER TABLE metadata_people_next RENAME TO metadata_people;

CREATE INDEX idx_metadata_people_provider_identity
    ON metadata_people (provider_id, identity_key);
CREATE INDEX idx_metadata_people_provider_identity_locale
    ON metadata_people (provider_id, identity_key, locale_key);

PRAGMA foreign_keys=on;
