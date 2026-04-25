ALTER TABLE item_metadata_links ADD COLUMN logo_url TEXT DEFAULT NULL;
ALTER TABLE item_metadata_links ADD COLUMN cached_logo_path TEXT DEFAULT NULL;
ALTER TABLE item_metadata_links ADD COLUMN genres_json TEXT DEFAULT NULL;
ALTER TABLE item_metadata_links ADD COLUMN rating FLOAT DEFAULT NULL;
ALTER TABLE item_metadata_links ADD COLUMN content_rating TEXT DEFAULT NULL;
ALTER TABLE item_metadata_links ADD COLUMN trailer_title TEXT DEFAULT NULL;
ALTER TABLE item_metadata_links ADD COLUMN trailer_url TEXT DEFAULT NULL;
