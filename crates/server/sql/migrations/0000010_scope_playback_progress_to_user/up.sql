PRAGMA foreign_keys=off;

CREATE TABLE playback_progress_next (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER DEFAULT NULL,
    media_file_id INTEGER NOT NULL,
    position_ms BIGINT NOT NULL DEFAULT 0,
    duration_ms BIGINT DEFAULT NULL,
    completed BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at BIGINT DEFAULT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (media_file_id) REFERENCES media_files(id) ON DELETE CASCADE,
    UNIQUE (user_id, media_file_id)
);

INSERT INTO playback_progress_next (
    id,
    user_id,
    media_file_id,
    position_ms,
    duration_ms,
    completed,
    updated_at
)
SELECT
    id,
    NULL,
    media_file_id,
    position_ms,
    duration_ms,
    completed,
    updated_at
FROM playback_progress;

DROP TABLE playback_progress;
ALTER TABLE playback_progress_next RENAME TO playback_progress;

PRAGMA foreign_keys=on;

