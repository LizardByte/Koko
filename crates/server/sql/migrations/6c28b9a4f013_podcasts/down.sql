DROP TABLE podcast_items;
DROP TABLE podcast_subscriptions;
DROP TABLE podcast_feed_aliases;
DROP TABLE podcast_feeds;
DELETE FROM media_libraries WHERE kind = 'podcasts';
