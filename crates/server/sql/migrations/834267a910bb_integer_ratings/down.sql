-- Restore the previous scale, retaining the precision available after rounding.
ALTER TABLE item_metadata_links ADD COLUMN rating_decimal FLOAT DEFAULT NULL;
UPDATE item_metadata_links
SET rating_decimal = rating / 10.0
WHERE rating IS NOT NULL;
ALTER TABLE item_metadata_links DROP COLUMN rating;
ALTER TABLE item_metadata_links RENAME COLUMN rating_decimal TO rating;
