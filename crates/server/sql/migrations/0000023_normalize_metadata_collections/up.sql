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
    theme_song_url TEXT DEFAULT NULL,
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

INSERT OR IGNORE INTO metadata_collections (
    provider_id, external_id, source_provider_id, source_external_id, relation_kind,
    locale_key, provider_locale_key, name, overview, artwork_url, backdrop_url, updated_at
)
SELECT
    item_metadata_collections.provider_id,
    item_metadata_collections.external_id,
    item_metadata_collections.provider_id,
    item_metadata_collections.external_id,
    'primary',
    item_metadata_links.locale_key,
    item_metadata_links.provider_locale_key,
    item_metadata_collections.name,
    item_metadata_collections.overview,
    item_metadata_collections.artwork_url,
    item_metadata_collections.backdrop_url,
    item_metadata_collections.updated_at
FROM item_metadata_collections
JOIN item_metadata_links
    ON item_metadata_links.id = item_metadata_collections.metadata_link_id
ORDER BY item_metadata_collections.updated_at DESC, item_metadata_collections.id DESC;

INSERT OR IGNORE INTO metadata_collection_items (
    collection_id, media_item_id, metadata_link_id, updated_at
)
SELECT
    metadata_collections.id,
    item_metadata_links.media_item_id,
    item_metadata_collections.metadata_link_id,
    item_metadata_collections.updated_at
FROM item_metadata_collections
JOIN metadata_collections
    ON metadata_collections.provider_id = item_metadata_collections.provider_id
   AND metadata_collections.external_id = item_metadata_collections.external_id
   AND metadata_collections.relation_kind = 'primary'
   AND metadata_collections.locale_key = item_metadata_links.locale_key
JOIN item_metadata_links
    ON item_metadata_links.id = item_metadata_collections.metadata_link_id;

DROP INDEX IF EXISTS idx_item_metadata_collections_link_id;
DROP TABLE item_metadata_collections;

CREATE INDEX idx_metadata_collection_items_collection_id
    ON metadata_collection_items (collection_id);
CREATE INDEX idx_metadata_collection_items_media_item_id
    ON metadata_collection_items (media_item_id);
CREATE INDEX idx_metadata_collection_items_metadata_link_id
    ON metadata_collection_items (metadata_link_id);
