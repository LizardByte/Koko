UPDATE media_items
SET identity_key = substr(identity_key, length('library:' || library_id || ':') + 1)
WHERE item_type IN ('show', 'season', 'episode')
  AND identity_key LIKE 'library:' || library_id || ':%';
