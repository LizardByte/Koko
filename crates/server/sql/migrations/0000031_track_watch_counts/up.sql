ALTER TABLE playback_progress ADD COLUMN watch_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE playback_progress ADD COLUMN last_watched_at BIGINT DEFAULT NULL;

UPDATE playback_progress
SET
    watch_count = CASE WHEN completed THEN 1 ELSE 0 END,
    last_watched_at = CASE WHEN completed THEN updated_at ELSE NULL END
WHERE watch_count = 0
  AND last_watched_at IS NULL;
