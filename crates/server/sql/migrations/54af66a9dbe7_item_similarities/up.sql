-- Recommendations identify provider records even when no local item exists yet.
CREATE TABLE item_similarities (
    metadata_link_id INTEGER NOT NULL REFERENCES item_metadata_links(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL,
    media_type TEXT NOT NULL,
    external_id TEXT NOT NULL,
    rank INTEGER NOT NULL CHECK (rank >= 0),
    PRIMARY KEY (metadata_link_id, provider_id, media_type, external_id)
);

-- Scores are shared by all accounts; current access and availability are checked on reads.
CREATE TABLE item_recommendations (
    item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    similar_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    provider_priority INTEGER NOT NULL CHECK (provider_priority >= 0),
    provider_rank INTEGER NOT NULL CHECK (provider_rank >= 0),
    local_score INTEGER NOT NULL CHECK (local_score >= 0),
    year_distance BIGINT NOT NULL CHECK (year_distance >= 0),
    PRIMARY KEY (item_id, similar_item_id),
    CHECK (item_id <> similar_item_id)
);

CREATE INDEX item_recommendations_target ON item_recommendations(similar_item_id);
CREATE INDEX item_recommendations_order ON item_recommendations(
    item_id, provider_priority DESC, provider_rank, local_score DESC, year_distance
);
