CREATE TABLE item_metadata_external_ids (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metadata_link_id INTEGER NOT NULL,
    source TEXT NOT NULL,
    external_id TEXT NOT NULL,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (metadata_link_id) REFERENCES item_metadata_links(id) ON DELETE CASCADE,
    UNIQUE (metadata_link_id, source)
);

CREATE INDEX idx_item_metadata_external_ids_link_id
    ON item_metadata_external_ids (metadata_link_id);
CREATE INDEX idx_item_metadata_external_ids_source_external_id
    ON item_metadata_external_ids (source, external_id);
