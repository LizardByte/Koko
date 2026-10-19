ALTER TABLE media_items ADD COLUMN metadata_match_attempted_at BIGINT;

CREATE TABLE music_file_metadata (
    media_file_id INTEGER PRIMARY KEY REFERENCES media_files(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    file_hash TEXT NOT NULL,
    artist TEXT NOT NULL,
    album_artist TEXT NOT NULL,
    album TEXT NOT NULL,
    track_number INTEGER,
    disc_number INTEGER,
    release_year INTEGER,
    musicbrainz_artist_id TEXT,
    musicbrainz_release_id TEXT,
    musicbrainz_recording_id TEXT
);

CREATE TABLE music_file_genres (
    media_file_id INTEGER NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    genre TEXT NOT NULL,
    PRIMARY KEY (media_file_id, genre)
);

CREATE TABLE music_item_metadata (
    media_item_id INTEGER PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
    artist TEXT NOT NULL,
    album_artist TEXT NOT NULL,
    album TEXT NOT NULL,
    track_number INTEGER,
    disc_number INTEGER,
    release_year INTEGER,
    musicbrainz_artist_id TEXT,
    musicbrainz_release_id TEXT,
    musicbrainz_recording_id TEXT
);

CREATE TABLE music_item_genres (
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    source TEXT NOT NULL DEFAULT 'local',
    genre TEXT NOT NULL,
    PRIMARY KEY (media_item_id, source, genre)
);

ALTER TABLE media_libraries ADD COLUMN use_embedded_music_tags BOOLEAN NOT NULL DEFAULT 0;

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

ALTER TABLE media_libraries ADD COLUMN exclude_from_home BOOLEAN NOT NULL DEFAULT 0;

ALTER TABLE music_playlists ADD COLUMN artwork_item_id INTEGER REFERENCES media_items(id) ON DELETE SET NULL;

-- Older queues did not retain their seed. Freeze the first song's album as a stable fallback.
UPDATE music_playlists SET artwork_item_id = (
    SELECT CASE WHEN album.item_type = 'album' THEN album.id ELSE track.id END
    FROM music_playlist_entries AS entry
    JOIN media_items AS track ON track.id = entry.media_item_id
    LEFT JOIN media_items AS album ON album.id = track.parent_id
    WHERE entry.playlist_id = music_playlists.id
    ORDER BY entry.position LIMIT 1
) WHERE temporary = 1;
