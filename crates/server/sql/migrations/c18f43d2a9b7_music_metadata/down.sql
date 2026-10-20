ALTER TABLE music_playlists DROP COLUMN artwork_item_id;

ALTER TABLE media_libraries DROP COLUMN exclude_from_home;

DROP TABLE music_lyrics;

DROP TABLE music_progress_context;
ALTER TABLE music_playlists DROP COLUMN repeat_mode;
ALTER TABLE music_playlists DROP COLUMN shuffle;
ALTER TABLE music_playlists DROP COLUMN completed;
ALTER TABLE music_playlists DROP COLUMN position_ms;
ALTER TABLE music_playlists DROP COLUMN current_index;
ALTER TABLE music_playlists DROP COLUMN temporary;

DROP TABLE music_playlist_entries;
DROP TABLE music_playlists;

ALTER TABLE media_libraries DROP COLUMN use_embedded_music_tags;
DROP TABLE music_item_genres;
DROP TABLE music_item_metadata;
DROP TABLE music_file_genres;
DROP TABLE music_file_metadata;
ALTER TABLE media_items DROP COLUMN metadata_match_attempted_at;
