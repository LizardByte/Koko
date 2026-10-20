CREATE TABLE audiobook_metadata (
    media_item_id INTEGER PRIMARY KEY NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    metadata_json TEXT NOT NULL
);
CREATE TABLE audiobook_provider_metadata (
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL,
    external_id TEXT NOT NULL,
    metadata_json TEXT NOT NULL,
    PRIMARY KEY (media_item_id, provider_id)
);
