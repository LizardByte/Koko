UPDATE media_items
SET identity_key = 'library:' || library_id || ':' || identity_key
WHERE item_type IN ('show', 'season', 'episode')
  AND identity_key NOT LIKE 'library:%';
