ALTER TABLE music_playlists ADD COLUMN artwork_item_id INTEGER REFERENCES media_items(id) ON DELETE SET NULL;

-- Older queues did not retain their seed. Freeze the first song's album as a stable fallback.
UPDATE music_playlists SET artwork_item_id = (
    SELECT CASE WHEN album.item_type = 'album' THEN album.id ELSE track.id END
    FROM music_playlist_entries AS entry
    JOIN media_items AS track ON track.id = entry.media_item_id
    LEFT JOIN media_items AS album ON album.id = track.parent_id
    WHERE entry.playlist_id = music_playlists.id
    ORDER BY entry.position LIMIT 1
) WHERE temporary = 1;
