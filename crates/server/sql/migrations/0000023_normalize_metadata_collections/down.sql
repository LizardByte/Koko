CREATE TABLE item_metadata_collections (
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

INSERT OR IGNORE INTO item_metadata_collections (
    metadata_link_id, provider_id, external_id, name, overview, artwork_url,
    backdrop_url, updated_at
)
SELECT
    metadata_collection_items.metadata_link_id,
    metadata_collections.provider_id,
    metadata_collections.external_id,
    COALESCE(metadata_collections.name, metadata_collections.external_id),
    metadata_collections.overview,
    metadata_collections.artwork_url,
    metadata_collections.backdrop_url,
    metadata_collection_items.updated_at
FROM metadata_collection_items
JOIN metadata_collections
    ON metadata_collections.id = metadata_collection_items.collection_id
WHERE metadata_collections.relation_kind = 'primary';

CREATE INDEX idx_item_metadata_collections_link_id
    ON item_metadata_collections (metadata_link_id);

DROP INDEX IF EXISTS idx_metadata_collection_items_metadata_link_id;
DROP INDEX IF EXISTS idx_metadata_collection_items_media_item_id;
DROP INDEX IF EXISTS idx_metadata_collection_items_collection_id;
DROP TABLE metadata_collection_items;
DROP TABLE metadata_collections;
