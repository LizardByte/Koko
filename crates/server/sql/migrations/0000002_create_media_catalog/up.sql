CREATE TABLE IF NOT EXISTS media_libraries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    path TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL,
    recursive BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TABLE IF NOT EXISTS scan_state (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL UNIQUE,
    last_status TEXT NOT NULL DEFAULT 'never_scanned',
    last_error TEXT DEFAULT NULL,
    total_files BIGINT NOT NULL DEFAULT 0,
    video_files BIGINT NOT NULL DEFAULT 0,
    audio_files BIGINT NOT NULL DEFAULT 0,
    image_files BIGINT NOT NULL DEFAULT 0,
    book_files BIGINT NOT NULL DEFAULT 0,
    other_files BIGINT NOT NULL DEFAULT 0,
    FOREIGN KEY (library_id) REFERENCES media_libraries(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS media_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL,
    relative_path TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    modified_at BIGINT DEFAULT NULL,
    media_kind TEXT NOT NULL,
    fingerprint_seed TEXT NOT NULL,
    FOREIGN KEY (library_id) REFERENCES media_libraries(id) ON DELETE CASCADE,
    UNIQUE (library_id, relative_path)
);

