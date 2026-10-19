ALTER TABLE media_libraries DROP COLUMN use_embedded_music_tags;
DROP TABLE music_item_genres;
DROP TABLE music_item_metadata;
DROP TABLE music_file_genres;
DROP TABLE music_file_metadata;
ALTER TABLE media_items DROP COLUMN metadata_match_attempted_at;
