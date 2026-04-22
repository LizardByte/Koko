ALTER TABLE media_libraries ADD COLUMN paths_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE media_libraries ADD COLUMN metadata_providers_json TEXT NOT NULL DEFAULT '["tmdb"]';

UPDATE media_libraries
SET paths_json = json_array(path)
WHERE TRIM(COALESCE(paths_json, '')) = '' OR paths_json = '[]';

UPDATE media_libraries
SET metadata_providers_json = '["tmdb"]'
WHERE TRIM(COALESCE(metadata_providers_json, '')) = '';

