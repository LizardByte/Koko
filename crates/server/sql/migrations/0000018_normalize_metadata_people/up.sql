CREATE TABLE metadata_people (
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
    provider_payload_json TEXT DEFAULT NULL,
    updated_at BIGINT DEFAULT NULL,
    UNIQUE (provider_id, identity_key, locale_key)
);

CREATE TABLE metadata_person_credits (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metadata_link_id INTEGER NOT NULL,
    person_id INTEGER NOT NULL,
    role TEXT DEFAULT NULL,
    department TEXT DEFAULT NULL,
    character_name TEXT DEFAULT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (metadata_link_id) REFERENCES item_metadata_links(id) ON DELETE CASCADE,
    FOREIGN KEY (person_id) REFERENCES metadata_people(id) ON DELETE CASCADE,
    UNIQUE (metadata_link_id, person_id, role, character_name)
);

CREATE INDEX idx_metadata_people_provider_identity ON metadata_people (provider_id, identity_key);
CREATE INDEX idx_metadata_people_provider_identity_locale ON metadata_people (provider_id, identity_key, locale_key);
CREATE INDEX idx_metadata_person_credits_person_id ON metadata_person_credits (person_id);
CREATE INDEX idx_metadata_person_credits_link_id ON metadata_person_credits (metadata_link_id);

INSERT OR IGNORE INTO metadata_people (
    provider_id,
    external_id,
    identity_key,
    locale_key,
    name,
    known_for_json,
    biography,
    gender,
    birthday,
    deathday,
    birth_place,
    profile_url,
    image_url,
    cached_image_path,
    updated_at
)
SELECT
    item_metadata_links.provider_id,
    item_metadata_people.external_id,
    COALESCE(item_metadata_people.external_id, 'name:' || lower(item_metadata_people.name)),
    item_metadata_links.locale_key,
    item_metadata_people.name,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL,
    item_metadata_people.profile_url,
    item_metadata_people.image_url,
    NULL,
    item_metadata_links.updated_at
FROM item_metadata_people
JOIN item_metadata_links ON item_metadata_links.id = item_metadata_people.metadata_link_id;

INSERT OR IGNORE INTO metadata_person_credits (
    metadata_link_id,
    person_id,
    role,
    department,
    character_name,
    sort_order
)
SELECT
    item_metadata_people.metadata_link_id,
    metadata_people.id,
    item_metadata_people.role,
    item_metadata_people.department,
    item_metadata_people.character_name,
    item_metadata_people.sort_order
FROM item_metadata_people
JOIN item_metadata_links ON item_metadata_links.id = item_metadata_people.metadata_link_id
JOIN metadata_people
  ON metadata_people.provider_id = item_metadata_links.provider_id
 AND metadata_people.identity_key = COALESCE(item_metadata_people.external_id, 'name:' || lower(item_metadata_people.name))
 AND metadata_people.locale_key = item_metadata_links.locale_key;
