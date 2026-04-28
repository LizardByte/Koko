UPDATE item_metadata_links
SET trailer_url = 'https://www.youtube.com/watch?v=' || substr(
    trailer_url,
    instr(trailer_url, '/embed/') + length('/embed/'),
    11
)
WHERE trailer_url LIKE 'https://www.youtube.com/embed/___________%'
  AND length(substr(trailer_url, instr(trailer_url, '/embed/') + length('/embed/'), 11)) = 11;

UPDATE item_metadata_links
SET trailer_url = 'https://www.youtube.com/watch?v=' || substr(
    trailer_url,
    instr(trailer_url, '/embed/') + length('/embed/'),
    11
)
WHERE trailer_url LIKE 'https://www.youtube-nocookie.com/embed/___________%'
  AND length(substr(trailer_url, instr(trailer_url, '/embed/') + length('/embed/'), 11)) = 11;
