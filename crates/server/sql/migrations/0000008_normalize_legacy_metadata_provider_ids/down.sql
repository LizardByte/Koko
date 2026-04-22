UPDATE item_metadata_links
SET provider_id = 'music_brainz'
WHERE provider_id = 'musicbrainz';

UPDATE media_libraries
SET metadata_providers_json = REPLACE(metadata_providers_json, 'musicbrainz', 'music_brainz')
WHERE metadata_providers_json LIKE '%musicbrainz%';

