CREATE TABLE live_tv_sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    config_json TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT 1,
    capacity INTEGER NOT NULL CHECK (capacity BETWEEN 1 AND 64),
    next_refresh BIGINT NOT NULL DEFAULT 0,
    last_error TEXT
);
CREATE TABLE live_tv_channels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id INTEGER NOT NULL REFERENCES live_tv_sources(id) ON DELETE CASCADE,
    source_key TEXT NOT NULL,
    name TEXT NOT NULL,
    number TEXT NOT NULL,
    group_name TEXT NOT NULL,
    logo TEXT,
    stream_url TEXT NOT NULL,
    guide_key TEXT NOT NULL,
    guide_override TEXT,
    enabled BOOLEAN NOT NULL DEFAULT 1,
    available BOOLEAN NOT NULL DEFAULT 1,
    UNIQUE(source_id, source_key)
);
CREATE TABLE live_tv_programs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id INTEGER NOT NULL REFERENCES live_tv_sources(id) ON DELETE CASCADE,
    guide_key TEXT NOT NULL,
    starts_at BIGINT NOT NULL,
    ends_at BIGINT NOT NULL CHECK (ends_at > starts_at),
    title TEXT NOT NULL,
    metadata_json TEXT NOT NULL,
    UNIQUE(source_id, guide_key, starts_at)
);
CREATE INDEX live_tv_program_window ON live_tv_programs(source_id, guide_key, ends_at, starts_at);
CREATE TABLE live_tv_stations (
    source_id INTEGER NOT NULL REFERENCES live_tv_sources(id) ON DELETE CASCADE,
    guide_key TEXT NOT NULL,
    name TEXT NOT NULL,
    number TEXT NOT NULL,
    aliases_json TEXT NOT NULL,
    logo TEXT,
    PRIMARY KEY (source_id, guide_key)
);
CREATE TABLE live_tv_series_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    channel_id INTEGER NOT NULL REFERENCES live_tv_channels(id),
    title TEXT NOT NULL,
    new_only BOOLEAN NOT NULL,
    padding_before INTEGER NOT NULL,
    padding_after INTEGER NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT 1,
    shared BOOLEAN NOT NULL DEFAULT 1,
    UNIQUE(user_id, channel_id, title)
);
CREATE TABLE live_tv_recordings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    channel_id INTEGER NOT NULL REFERENCES live_tv_channels(id),
    rule_id INTEGER REFERENCES live_tv_series_rules(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    starts_at BIGINT NOT NULL,
    ends_at BIGINT NOT NULL CHECK (ends_at > starts_at),
    padding_before INTEGER NOT NULL,
    padding_after INTEGER NOT NULL,
    state TEXT NOT NULL,
    file_name TEXT,
    last_error TEXT,
    metadata_json TEXT NOT NULL,
    shared BOOLEAN NOT NULL DEFAULT 1,
    UNIQUE(user_id, channel_id, starts_at)
);
CREATE INDEX live_tv_recording_schedule ON live_tv_recordings(state, starts_at, ends_at);
CREATE TABLE live_tv_user_permissions (
    user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    can_record BOOLEAN NOT NULL DEFAULT 1
);
