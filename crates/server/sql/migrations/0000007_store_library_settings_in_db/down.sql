PRAGMA foreign_keys=off;

CREATE TABLE media_libraries_previous (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    path TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL,
    recursive BOOLEAN NOT NULL DEFAULT TRUE
);

INSERT INTO media_libraries_previous (
    id,
    name,
    path,
    kind,
    recursive
)
SELECT
    id,
    name,
    path,
    kind,
    recursive
FROM media_libraries;

DROP TABLE media_libraries;
ALTER TABLE media_libraries_previous RENAME TO media_libraries;

PRAGMA foreign_keys=on;

