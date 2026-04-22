UPDATE item_metadata_links
SET provider_id = 'musicbrainz'
WHERE provider_id = 'music_brainz';

UPDATE media_libraries
SET metadata_providers_json = REPLACE(metadata_providers_json, 'music_brainz', 'musicbrainz')
WHERE metadata_providers_json LIKE '%music_brainz%';

