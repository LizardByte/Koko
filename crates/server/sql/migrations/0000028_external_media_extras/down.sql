ALTER TABLE item_metadata_links ADD COLUMN trailer_title TEXT DEFAULT NULL;
ALTER TABLE item_metadata_links ADD COLUMN trailer_url TEXT DEFAULT NULL;
ALTER TABLE item_metadata_links ADD COLUMN theme_song_url TEXT DEFAULT NULL;
ALTER TABLE metadata_collections ADD COLUMN theme_song_url TEXT DEFAULT NULL;

UPDATE item_metadata_links
SET
    trailer_title = (
        SELECT metadata_extras.title
        FROM metadata_extras
        JOIN external_media ON external_media.id = metadata_extras.external_media_id
        WHERE metadata_extras.metadata_link_id = item_metadata_links.id
          AND metadata_extras.extra_type = 'trailer'
        ORDER BY metadata_extras.sort_order ASC, metadata_extras.id ASC
        LIMIT 1
    ),
    trailer_url = (
        SELECT external_media.url
        FROM metadata_extras
        JOIN external_media ON external_media.id = metadata_extras.external_media_id
        WHERE metadata_extras.metadata_link_id = item_metadata_links.id
          AND metadata_extras.extra_type = 'trailer'
        ORDER BY metadata_extras.sort_order ASC, metadata_extras.id ASC
        LIMIT 1
    ),
    theme_song_url = (
        SELECT external_media.url
        FROM metadata_extras
        JOIN external_media ON external_media.id = metadata_extras.external_media_id
        WHERE metadata_extras.metadata_link_id = item_metadata_links.id
          AND metadata_extras.extra_type = 'theme_song'
        ORDER BY metadata_extras.sort_order ASC, metadata_extras.id ASC
        LIMIT 1
    );

UPDATE metadata_collections
SET theme_song_url = (
    SELECT external_media.url
    FROM metadata_extras
    JOIN external_media ON external_media.id = metadata_extras.external_media_id
    WHERE metadata_extras.collection_id = metadata_collections.id
      AND metadata_extras.extra_type = 'theme_song'
    ORDER BY metadata_extras.sort_order ASC, metadata_extras.id ASC
    LIMIT 1
);

DROP INDEX IF EXISTS idx_metadata_extras_extra_type;
DROP INDEX IF EXISTS idx_metadata_extras_external_media_id;
DROP INDEX IF EXISTS idx_metadata_extras_collection_id;
DROP INDEX IF EXISTS idx_metadata_extras_metadata_link_id;
DROP INDEX IF EXISTS idx_external_media_source_external_id;

DROP TABLE metadata_extras;
DROP TABLE external_media;
