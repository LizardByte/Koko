ALTER TABLE media_files ADD COLUMN fingerprint_seed TEXT NOT NULL DEFAULT '';

UPDATE media_files
SET fingerprint_seed = file_hash
WHERE fingerprint_seed = '';
