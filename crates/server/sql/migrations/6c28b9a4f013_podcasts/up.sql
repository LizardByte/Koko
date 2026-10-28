-- Feed content is shared; subscribing and playback history belong to individual accounts.
CREATE TABLE podcast_feeds (
    media_item_id INTEGER PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
    feed_url TEXT NOT NULL UNIQUE,
    checked_at BIGINT NOT NULL,
    last_error TEXT
);
CREATE TABLE podcast_feed_aliases (
    feed_url TEXT PRIMARY KEY,
    podcast_id INTEGER NOT NULL REFERENCES podcast_feeds(media_item_id) ON DELETE CASCADE
);
CREATE TABLE podcast_subscriptions (
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    podcast_id INTEGER NOT NULL REFERENCES podcast_feeds(media_item_id) ON DELETE CASCADE,
    added_at BIGINT NOT NULL,
    PRIMARY KEY (user_id, podcast_id)
);
CREATE INDEX podcast_subscriptions_feed ON podcast_subscriptions(podcast_id);
CREATE TABLE podcast_items (
    media_item_id INTEGER PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
    podcast_id INTEGER NOT NULL REFERENCES podcast_feeds(media_item_id) ON DELETE CASCADE,
    metadata_json TEXT NOT NULL,
    enclosure_url TEXT,
    mime_type TEXT
);
CREATE INDEX podcast_items_feed ON podcast_items(podcast_id);
