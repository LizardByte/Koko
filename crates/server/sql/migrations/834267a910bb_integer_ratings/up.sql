-- Keep the parent table and its IDs intact so dependent metadata records survive.
ALTER TABLE item_metadata_links ADD COLUMN rating_scaled SMALLINT DEFAULT NULL
    CHECK (rating_scaled IS NULL OR (
        typeof(rating_scaled) = 'integer' AND rating_scaled BETWEEN 0 AND 100
    ));

UPDATE item_metadata_links
SET rating_scaled = CASE
    WHEN typeof(rating) IN ('integer', 'real') AND rating BETWEEN 0 AND 10
        THEN CAST(round(rating * 10) AS INTEGER)
    ELSE NULL
END;

ALTER TABLE item_metadata_links DROP COLUMN rating;
ALTER TABLE item_metadata_links RENAME COLUMN rating_scaled TO rating;
