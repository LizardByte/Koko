CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    password TEXT NOT NULL,
    pin TEXT DEFAULT NULL,
    admin BOOLEAN NOT NULL DEFAULT FALSE,
    birthday TEXT DEFAULT NULL,
    profile_image_path TEXT DEFAULT NULL,
    preferred_metadata_languages_json TEXT NOT NULL DEFAULT '["en-US"]'
);

CREATE TABLE media_libraries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    paths_json TEXT NOT NULL DEFAULT '[]',
    kind TEXT NOT NULL,
    scanner TEXT NOT NULL DEFAULT 'auto',
    recursive BOOLEAN NOT NULL DEFAULT TRUE,
    metadata_providers_json TEXT NOT NULL DEFAULT '["tmdb"]',
    metadata_language_mode TEXT NOT NULL DEFAULT 'auto',
    metadata_languages_json TEXT NOT NULL DEFAULT '["en-US"]',
    allowed_user_ids_json TEXT NOT NULL DEFAULT '[]'
);

CREATE TABLE scan_state (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL UNIQUE,
    last_status TEXT NOT NULL DEFAULT 'never_scanned',
    last_error TEXT DEFAULT NULL,
    scan_revision BIGINT NOT NULL DEFAULT 0,
    last_scanned_at BIGINT DEFAULT NULL,
    total_files BIGINT NOT NULL DEFAULT 0,
    video_files BIGINT NOT NULL DEFAULT 0,
    audio_files BIGINT NOT NULL DEFAULT 0,
    image_files BIGINT NOT NULL DEFAULT 0,
    book_files BIGINT NOT NULL DEFAULT 0,
    other_files BIGINT NOT NULL DEFAULT 0,
    FOREIGN KEY (library_id) REFERENCES media_libraries(id) ON DELETE CASCADE
);

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
    missing_since BIGINT DEFAULT NULL,
    deleted_at BIGINT DEFAULT NULL,
    FOREIGN KEY (library_id) REFERENCES media_libraries(id) ON DELETE CASCADE,
    FOREIGN KEY (parent_id) REFERENCES media_items(id) ON DELETE CASCADE
);

CREATE TABLE media_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    modified_at BIGINT DEFAULT NULL,
    media_kind TEXT NOT NULL,
    file_hash TEXT NOT NULL DEFAULT '',
    container TEXT DEFAULT NULL,
    duration_ms BIGINT DEFAULT NULL,
    bit_rate BIGINT DEFAULT NULL,
    width INTEGER DEFAULT NULL,
    height INTEGER DEFAULT NULL,
    video_codec TEXT DEFAULT NULL,
    audio_codec TEXT DEFAULT NULL,
    metadata_json TEXT DEFAULT NULL,
    metadata_updated_at BIGINT DEFAULT NULL,
    UNIQUE (path)
);

CREATE TABLE media_file_libraries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    media_file_id INTEGER NOT NULL,
    library_id INTEGER NOT NULL,
    source_root_path TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    display_title TEXT DEFAULT NULL,
    metadata_match_attempted_at BIGINT DEFAULT NULL,
    media_item_id INTEGER DEFAULT NULL,
    missing_since BIGINT DEFAULT NULL,
    deleted_at BIGINT DEFAULT NULL,
    FOREIGN KEY (media_file_id) REFERENCES media_files(id) ON DELETE CASCADE,
    FOREIGN KEY (library_id) REFERENCES media_libraries(id) ON DELETE CASCADE,
    FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE SET NULL,
    UNIQUE (library_id, source_root_path, relative_path)
);

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
    relation_kind TEXT NOT NULL DEFAULT 'primary',
    match_state TEXT NOT NULL DEFAULT 'unmatched',
    logo_url TEXT DEFAULT NULL,
    cached_logo_path TEXT DEFAULT NULL,
    genres_json TEXT DEFAULT NULL,
    rating FLOAT DEFAULT NULL,
    content_rating TEXT DEFAULT NULL,
    locale_key TEXT NOT NULL DEFAULT 'en-US',
    provider_locale_key TEXT DEFAULT NULL,
    cached_artwork_path TEXT DEFAULT NULL,
    cached_backdrop_path TEXT DEFAULT NULL,
    refresh_state TEXT NOT NULL DEFAULT 'fresh',
    refresh_interval_seconds BIGINT NOT NULL DEFAULT 604800,
    last_refreshed_at BIGINT DEFAULT NULL,
    next_refresh_at BIGINT DEFAULT NULL,
    refresh_error TEXT DEFAULT NULL,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE CASCADE,
    UNIQUE (media_item_id, provider_id, relation_kind, locale_key)
);

CREATE TABLE item_metadata_external_ids (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metadata_link_id INTEGER NOT NULL,
    source TEXT NOT NULL,
    external_id TEXT NOT NULL,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (metadata_link_id) REFERENCES item_metadata_links(id) ON DELETE CASCADE,
    UNIQUE (metadata_link_id, source)
);

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

CREATE TABLE metadata_people (
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

CREATE TABLE metadata_person_external_ids (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    person_id INTEGER NOT NULL,
    source TEXT NOT NULL,
    external_id TEXT NOT NULL,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (person_id) REFERENCES metadata_people(id) ON DELETE CASCADE,
    UNIQUE (person_id, source)
);

CREATE TABLE metadata_collections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id TEXT NOT NULL,
    external_id TEXT NOT NULL,
    source_provider_id TEXT NOT NULL,
    source_external_id TEXT NOT NULL,
    relation_kind TEXT NOT NULL,
    locale_key TEXT NOT NULL,
    provider_locale_key TEXT DEFAULT NULL,
    name TEXT DEFAULT NULL,
    overview TEXT DEFAULT NULL,
    artwork_url TEXT DEFAULT NULL,
    backdrop_url TEXT DEFAULT NULL,
    updated_at BIGINT DEFAULT NULL,
    UNIQUE (provider_id, external_id, relation_kind, locale_key)
);

CREATE TABLE metadata_collection_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    collection_id INTEGER NOT NULL,
    media_item_id INTEGER NOT NULL,
    metadata_link_id INTEGER NOT NULL,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (collection_id) REFERENCES metadata_collections(id) ON DELETE CASCADE,
    FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE CASCADE,
    FOREIGN KEY (metadata_link_id) REFERENCES item_metadata_links(id) ON DELETE CASCADE,
    UNIQUE (collection_id, media_item_id)
);

CREATE TABLE external_media (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL,
    external_id TEXT DEFAULT NULL,
    url TEXT NOT NULL,
    media_kind TEXT NOT NULL DEFAULT 'video',
    title TEXT DEFAULT NULL,
    duration_seconds INTEGER DEFAULT NULL,
    thumbnail_url TEXT DEFAULT NULL,
    updated_at BIGINT DEFAULT NULL,
    UNIQUE (url)
);

CREATE TABLE metadata_extras (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metadata_link_id INTEGER DEFAULT NULL,
    collection_id INTEGER DEFAULT NULL,
    external_media_id INTEGER NOT NULL,
    extra_type TEXT NOT NULL,
    title TEXT DEFAULT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (metadata_link_id) REFERENCES item_metadata_links(id) ON DELETE CASCADE,
    FOREIGN KEY (collection_id) REFERENCES metadata_collections(id) ON DELETE CASCADE,
    FOREIGN KEY (external_media_id) REFERENCES external_media(id) ON DELETE CASCADE,
    CHECK (
        (metadata_link_id IS NOT NULL AND collection_id IS NULL)
        OR (metadata_link_id IS NULL AND collection_id IS NOT NULL)
    ),
    UNIQUE (metadata_link_id, extra_type, external_media_id),
    UNIQUE (collection_id, extra_type, external_media_id)
);

CREATE TABLE playback_progress (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER DEFAULT NULL,
    media_item_id INTEGER NOT NULL,
    position_ms BIGINT NOT NULL DEFAULT 0,
    duration_ms BIGINT DEFAULT NULL,
    completed BOOLEAN NOT NULL DEFAULT FALSE,
    watch_count INTEGER NOT NULL DEFAULT 0,
    last_watched_at BIGINT DEFAULT NULL,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE CASCADE,
    UNIQUE (user_id, media_item_id)
);

CREATE TABLE app_settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    updated_at BIGINT DEFAULT NULL
);

CREATE INDEX idx_media_items_library_parent ON media_items (library_id, parent_id);
CREATE INDEX idx_media_items_identity_key ON media_items (identity_key);
CREATE INDEX idx_media_items_missing_since ON media_items (missing_since);
CREATE INDEX idx_media_items_deleted_at ON media_items (deleted_at);

CREATE INDEX idx_media_file_libraries_media_file_id
    ON media_file_libraries (media_file_id);
CREATE INDEX idx_media_file_libraries_library_id
    ON media_file_libraries (library_id);
CREATE INDEX idx_media_file_libraries_media_item_id
    ON media_file_libraries (media_item_id);
CREATE INDEX idx_media_file_libraries_missing_since
    ON media_file_libraries (missing_since);
CREATE INDEX idx_media_file_libraries_deleted_at
    ON media_file_libraries (deleted_at);

CREATE INDEX idx_item_metadata_links_media_item_id
    ON item_metadata_links (media_item_id);
CREATE INDEX idx_item_metadata_external_ids_link_id
    ON item_metadata_external_ids (metadata_link_id);
CREATE INDEX idx_item_metadata_external_ids_source_external_id
    ON item_metadata_external_ids (source, external_id);
CREATE INDEX idx_item_metadata_people_link_id
    ON item_metadata_people (metadata_link_id);

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

CREATE INDEX idx_metadata_person_credits_person_id
    ON metadata_person_credits (person_id);
CREATE INDEX idx_metadata_person_credits_link_id
    ON metadata_person_credits (metadata_link_id);
CREATE INDEX idx_metadata_person_external_ids_person_id
    ON metadata_person_external_ids (person_id);
CREATE INDEX idx_metadata_person_external_ids_source_external_id
    ON metadata_person_external_ids (source, external_id);

CREATE INDEX idx_metadata_collection_items_collection_id
    ON metadata_collection_items (collection_id);
CREATE INDEX idx_metadata_collection_items_media_item_id
    ON metadata_collection_items (media_item_id);
CREATE INDEX idx_metadata_collection_items_metadata_link_id
    ON metadata_collection_items (metadata_link_id);

CREATE INDEX idx_external_media_source_external_id
    ON external_media (source, external_id);
CREATE INDEX idx_metadata_extras_metadata_link_id
    ON metadata_extras (metadata_link_id);
CREATE INDEX idx_metadata_extras_collection_id
    ON metadata_extras (collection_id);
CREATE INDEX idx_metadata_extras_external_media_id
    ON metadata_extras (external_media_id);
CREATE INDEX idx_metadata_extras_extra_type
    ON metadata_extras (extra_type);

CREATE INDEX idx_playback_progress_media_item_id
    ON playback_progress (media_item_id);

PRAGMA foreign_keys = ON;
