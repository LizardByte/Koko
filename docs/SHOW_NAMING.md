# Show naming guidelines

Koko matches shows most reliably when each show has its own folder, with seasons grouped beneath it.

## Recommended layout

```text
/TV Shows
  /Show Title
    /Season 1
      Show Title - S01E01 - Pilot.mkv
      Show Title - S01E02 - The Next Episode.mkv
```

Koko recognizes common season and episode forms:

- `Season 1`
- `Series 1`
- `S01E01`
- `1x01`
- `E01`

## Provider tags

Show folders may include provider tags in braces or square brackets:

```text
/TV Shows
  /Example Show (2020) [tmdb-12345:tvdb-67890]
    /Season 1
      Example Show - S01E01 - Pilot.mkv
```

Use these when a title is ambiguous or a provider has multiple records for the same name.

Useful tag forms:

- `{tmdb-12345}`
- `{tvdb-67890}`
- `{imdb-tt1234567}`

## Episode titles

Episode titles are optional, but helpful for browsing before external metadata is linked.

```text
Show Title - S02E03 - Episode Name.mkv
Show Title - 2x03 - Episode Name.mkv
```

If an episode title is missing, Koko falls back to the cleaned filename until provider metadata is linked.

## General tips

- Keep one show per folder.
- Keep season folders consistent within a show.
- Include season and episode numbers in every episode filename.
- Put provider IDs on the show folder rather than every episode unless an episode needs a special match.
