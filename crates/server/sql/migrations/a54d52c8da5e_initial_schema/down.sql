PRAGMA foreign_keys = OFF;

DROP TABLE IF EXISTS metadata_extras;
DROP TABLE IF EXISTS external_media;
DROP TABLE IF EXISTS metadata_collection_items;
DROP TABLE IF EXISTS metadata_collections;
DROP TABLE IF EXISTS metadata_person_external_ids;
DROP TABLE IF EXISTS metadata_person_credits;
DROP TABLE IF EXISTS metadata_people;
DROP TABLE IF EXISTS item_metadata_people;
DROP TABLE IF EXISTS item_metadata_external_ids;
DROP TABLE IF EXISTS item_metadata_links;
DROP TABLE IF EXISTS playback_progress;
DROP TABLE IF EXISTS media_file_libraries;
DROP TABLE IF EXISTS media_files;
DROP TABLE IF EXISTS media_items;
DROP TABLE IF EXISTS scan_state;
DROP TABLE IF EXISTS media_libraries;
DROP TABLE IF EXISTS app_settings;
DROP TABLE IF EXISTS users;

PRAGMA foreign_keys = ON;
