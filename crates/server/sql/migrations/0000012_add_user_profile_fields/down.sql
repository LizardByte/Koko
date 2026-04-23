PRAGMA foreign_keys=off;

CREATE TABLE users_next (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    password TEXT NOT NULL,
    pin TEXT DEFAULT NULL,
    admin BOOLEAN NOT NULL DEFAULT FALSE
);

INSERT INTO users_next (id, username, password, pin, admin)
SELECT id, username, password, pin, admin
FROM users;

DROP TABLE users;
ALTER TABLE users_next RENAME TO users;

PRAGMA foreign_keys=on;
