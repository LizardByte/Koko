ALTER TABLE item_metadata_links ADD COLUMN media_type TEXT DEFAULT NULL;
ALTER TABLE item_metadata_links ADD COLUMN provider_payload_json TEXT DEFAULT NULL;
ALTER TABLE item_metadata_links ADD COLUMN cached_artwork_path TEXT DEFAULT NULL;
ALTER TABLE item_metadata_links ADD COLUMN cached_backdrop_path TEXT DEFAULT NULL;

CREATE TABLE IF NOT EXISTS playback_progress (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    media_file_id INTEGER NOT NULL UNIQUE,
    position_ms BIGINT NOT NULL DEFAULT 0,
    duration_ms BIGINT DEFAULT NULL,
    completed BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (media_file_id) REFERENCES media_files(id) ON DELETE CASCADE
);

