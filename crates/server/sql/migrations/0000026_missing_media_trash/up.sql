ALTER TABLE media_files ADD COLUMN missing_since BIGINT DEFAULT NULL;
ALTER TABLE media_files ADD COLUMN deleted_at BIGINT DEFAULT NULL;

ALTER TABLE media_items ADD COLUMN missing_since BIGINT DEFAULT NULL;
ALTER TABLE media_items ADD COLUMN deleted_at BIGINT DEFAULT NULL;

CREATE INDEX idx_media_files_missing_since ON media_files (missing_since);
CREATE INDEX idx_media_files_deleted_at ON media_files (deleted_at);
CREATE INDEX idx_media_items_missing_since ON media_items (missing_since);
CREATE INDEX idx_media_items_deleted_at ON media_items (deleted_at);
