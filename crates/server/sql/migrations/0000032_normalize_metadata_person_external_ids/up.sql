CREATE TABLE metadata_person_external_ids (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    person_id INTEGER NOT NULL,
    source TEXT NOT NULL,
    external_id TEXT NOT NULL,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (person_id) REFERENCES metadata_people(id) ON DELETE CASCADE,
    UNIQUE (person_id, source)
);

CREATE INDEX idx_metadata_person_external_ids_person_id
    ON metadata_person_external_ids (person_id);
CREATE INDEX idx_metadata_person_external_ids_source_external_id
    ON metadata_person_external_ids (source, external_id);

INSERT OR IGNORE INTO metadata_person_external_ids (
    person_id,
    source,
    external_id,
    updated_at
)
SELECT
    id,
    CASE lower(provider_id)
        WHEN 'tvdb' THEN 'thetvdb'
        WHEN 'themoviedb' THEN 'tmdb'
        ELSE lower(provider_id)
    END,
    trim(external_id),
    updated_at
FROM metadata_people
WHERE external_id IS NOT NULL
  AND trim(external_id) <> '';
