ALTER TABLE media_libraries ADD COLUMN scanner TEXT NOT NULL DEFAULT 'auto';

ALTER TABLE media_files ADD COLUMN file_hash TEXT NOT NULL DEFAULT '';

UPDATE media_files
SET file_hash = fingerprint_seed
WHERE file_hash = '';
