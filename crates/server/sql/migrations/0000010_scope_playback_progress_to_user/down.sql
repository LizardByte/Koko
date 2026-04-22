PRAGMA foreign_keys=off;

CREATE TABLE playback_progress_previous (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    media_file_id INTEGER NOT NULL UNIQUE,
    position_ms BIGINT NOT NULL DEFAULT 0,
    duration_ms BIGINT DEFAULT NULL,
    completed BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (media_file_id) REFERENCES media_files(id) ON DELETE CASCADE
);

INSERT INTO playback_progress_previous (
    id,
    media_file_id,
    position_ms,
    duration_ms,
    completed,
    updated_at
)
SELECT
    MIN(id),
    media_file_id,
    position_ms,
    duration_ms,
    completed,
    updated_at
FROM playback_progress
GROUP BY media_file_id;

DROP TABLE playback_progress;
ALTER TABLE playback_progress_previous RENAME TO playback_progress;

PRAGMA foreign_keys=on;

