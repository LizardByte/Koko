CREATE TABLE external_media (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL,
    external_id TEXT DEFAULT NULL,
    url TEXT NOT NULL,
    media_kind TEXT NOT NULL DEFAULT 'video',
    title TEXT DEFAULT NULL,
    duration_seconds INTEGER DEFAULT NULL,
    thumbnail_url TEXT DEFAULT NULL,
    updated_at BIGINT DEFAULT NULL,
    UNIQUE (url)
);

CREATE TABLE metadata_extras (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metadata_link_id INTEGER DEFAULT NULL,
    collection_id INTEGER DEFAULT NULL,
    external_media_id INTEGER NOT NULL,
    extra_type TEXT NOT NULL,
    title TEXT DEFAULT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (metadata_link_id) REFERENCES item_metadata_links(id) ON DELETE CASCADE,
    FOREIGN KEY (collection_id) REFERENCES metadata_collections(id) ON DELETE CASCADE,
    FOREIGN KEY (external_media_id) REFERENCES external_media(id) ON DELETE CASCADE,
    CHECK (
        (metadata_link_id IS NOT NULL AND collection_id IS NULL)
        OR (metadata_link_id IS NULL AND collection_id IS NOT NULL)
    ),
    UNIQUE (metadata_link_id, extra_type, external_media_id),
    UNIQUE (collection_id, extra_type, external_media_id)
);

WITH media_urls AS (
    SELECT
        TRIM(trailer_url) AS url,
        NULLIF(TRIM(trailer_title), '') AS title,
        updated_at
    FROM item_metadata_links
    WHERE trailer_url IS NOT NULL AND TRIM(trailer_url) <> ''
    UNION ALL
    SELECT
        TRIM(theme_song_url) AS url,
        NULL AS title,
        updated_at
    FROM item_metadata_links
    WHERE theme_song_url IS NOT NULL AND TRIM(theme_song_url) <> ''
    UNION ALL
    SELECT
        TRIM(theme_song_url) AS url,
        NULL AS title,
        updated_at
    FROM metadata_collections
    WHERE theme_song_url IS NOT NULL AND TRIM(theme_song_url) <> ''
),
deduped_media_urls AS (
    SELECT
        url,
        MIN(title) AS title,
        MAX(updated_at) AS updated_at
    FROM media_urls
    GROUP BY url
)
INSERT OR IGNORE INTO external_media (
    source,
    external_id,
    url,
    media_kind,
    title,
    thumbnail_url,
    updated_at
)
SELECT
    CASE
        WHEN url LIKE 'https://www.youtube.com/watch?v=___________%' THEN 'youtube'
        WHEN url LIKE 'https://youtube.com/watch?v=___________%' THEN 'youtube'
        ELSE 'url'
    END,
    CASE
        WHEN url LIKE 'https://www.youtube.com/watch?v=___________%' THEN substr(url, instr(url, 'v=') + 2, 11)
        WHEN url LIKE 'https://youtube.com/watch?v=___________%' THEN substr(url, instr(url, 'v=') + 2, 11)
        ELSE NULL
    END,
    url,
    'video',
    title,
    CASE
        WHEN url LIKE 'https://www.youtube.com/watch?v=___________%' THEN 'https://i.ytimg.com/vi/' || substr(url, instr(url, 'v=') + 2, 11) || '/hqdefault.jpg'
        WHEN url LIKE 'https://youtube.com/watch?v=___________%' THEN 'https://i.ytimg.com/vi/' || substr(url, instr(url, 'v=') + 2, 11) || '/hqdefault.jpg'
        ELSE NULL
    END,
    updated_at
FROM deduped_media_urls;

INSERT OR IGNORE INTO metadata_extras (
    metadata_link_id,
    collection_id,
    external_media_id,
    extra_type,
    title,
    sort_order,
    updated_at
)
SELECT
    item_metadata_links.id,
    NULL,
    external_media.id,
    'trailer',
    NULLIF(TRIM(item_metadata_links.trailer_title), ''),
    0,
    item_metadata_links.updated_at
FROM item_metadata_links
JOIN external_media
    ON external_media.url = TRIM(item_metadata_links.trailer_url)
WHERE item_metadata_links.trailer_url IS NOT NULL
  AND TRIM(item_metadata_links.trailer_url) <> '';

INSERT OR IGNORE INTO metadata_extras (
    metadata_link_id,
    collection_id,
    external_media_id,
    extra_type,
    title,
    sort_order,
    updated_at
)
SELECT
    item_metadata_links.id,
    NULL,
    external_media.id,
    'theme_song',
    NULL,
    0,
    item_metadata_links.updated_at
FROM item_metadata_links
JOIN external_media
    ON external_media.url = TRIM(item_metadata_links.theme_song_url)
WHERE item_metadata_links.theme_song_url IS NOT NULL
  AND TRIM(item_metadata_links.theme_song_url) <> '';

INSERT OR IGNORE INTO metadata_extras (
    metadata_link_id,
    collection_id,
    external_media_id,
    extra_type,
    title,
    sort_order,
    updated_at
)
SELECT
    NULL,
    metadata_collections.id,
    external_media.id,
    'theme_song',
    NULL,
    0,
    metadata_collections.updated_at
FROM metadata_collections
JOIN external_media
    ON external_media.url = TRIM(metadata_collections.theme_song_url)
WHERE metadata_collections.theme_song_url IS NOT NULL
  AND TRIM(metadata_collections.theme_song_url) <> '';

CREATE INDEX idx_external_media_source_external_id
    ON external_media (source, external_id);
CREATE INDEX idx_metadata_extras_metadata_link_id
    ON metadata_extras (metadata_link_id);
CREATE INDEX idx_metadata_extras_collection_id
    ON metadata_extras (collection_id);
CREATE INDEX idx_metadata_extras_external_media_id
    ON metadata_extras (external_media_id);
CREATE INDEX idx_metadata_extras_extra_type
    ON metadata_extras (extra_type);

ALTER TABLE item_metadata_links DROP COLUMN trailer_title;
ALTER TABLE item_metadata_links DROP COLUMN trailer_url;
ALTER TABLE item_metadata_links DROP COLUMN theme_song_url;
ALTER TABLE metadata_collections DROP COLUMN theme_song_url;
