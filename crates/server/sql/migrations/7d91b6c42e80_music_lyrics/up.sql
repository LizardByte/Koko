CREATE TABLE music_lyrics (
    media_item_id INTEGER PRIMARY KEY NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    track_title TEXT NOT NULL,
    artist_name TEXT NOT NULL,
    album_name TEXT NOT NULL,
    duration_ms BIGINT,
    plain_lyrics TEXT,
    synced_lyrics TEXT,
    instrumental BOOLEAN NOT NULL DEFAULT 0,
    source TEXT NOT NULL,
    source_url TEXT NOT NULL,
    lookup_status TEXT NOT NULL CHECK (lookup_status IN ('found', 'missing', 'error')),
    checked_at BIGINT NOT NULL,
    next_check_at BIGINT NOT NULL,
    last_error TEXT
);
