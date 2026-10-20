CREATE TABLE audiobook_recording_selections (
    media_item_id INTEGER PRIMARY KEY NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL,
    external_id TEXT NOT NULL
);
