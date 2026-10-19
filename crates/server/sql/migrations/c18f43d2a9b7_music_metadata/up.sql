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
