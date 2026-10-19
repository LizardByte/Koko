ALTER TABLE music_playlists ADD COLUMN temporary BOOLEAN NOT NULL DEFAULT 0;
ALTER TABLE music_playlists ADD COLUMN current_index INTEGER NOT NULL DEFAULT 0;
ALTER TABLE music_playlists ADD COLUMN position_ms BIGINT NOT NULL DEFAULT 0;
ALTER TABLE music_playlists ADD COLUMN completed BOOLEAN NOT NULL DEFAULT 0;
ALTER TABLE music_playlists ADD COLUMN shuffle BOOLEAN NOT NULL DEFAULT 0;
ALTER TABLE music_playlists ADD COLUMN repeat_mode TEXT NOT NULL DEFAULT 'off';
CREATE TABLE music_progress_context (
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, media_item_id)
);
