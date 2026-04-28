ALTER TABLE media_libraries ADD COLUMN metadata_language_mode TEXT NOT NULL DEFAULT 'auto';
ALTER TABLE media_libraries ADD COLUMN metadata_languages_json TEXT NOT NULL DEFAULT '["en-US"]';
ALTER TABLE media_libraries ADD COLUMN allowed_user_ids_json TEXT NOT NULL DEFAULT '[]';
