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
CREATE TABLE audiobook_recording_selections (
    media_item_id INTEGER PRIMARY KEY NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL,
    external_id TEXT NOT NULL
);
CREATE TABLE reading_progress (
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    payload_json TEXT NOT NULL,
    updated_at BIGINT NOT NULL,
    PRIMARY KEY (user_id, media_item_id)
);
CREATE INDEX idx_reading_progress_item ON reading_progress(media_item_id);
