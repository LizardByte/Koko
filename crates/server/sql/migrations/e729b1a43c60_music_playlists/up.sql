CREATE TABLE music_playlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    created_at BIGINT NOT NULL DEFAULT (unixepoch()),
    updated_at BIGINT NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX music_playlists_owner ON music_playlists(user_id, updated_at);
CREATE TABLE music_playlist_entries (
    playlist_id INTEGER NOT NULL REFERENCES music_playlists(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    PRIMARY KEY (playlist_id, position)
);
CREATE INDEX music_playlist_entries_item ON music_playlist_entries(media_item_id);
