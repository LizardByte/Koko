# Movie naming guidelines

Koko matches movie files more reliably when each media type lives under its own top-level folder and movie files follow a predictable naming pattern.

## Recommended folder layout

Keep movies separate from shows, music, books, and photos.

```text
Media/
  Movies/
  TV Shows/
  Music/
  Books/
  Photos/
```

When you create a movie library in Koko, point it at the movie root such as `Media/Movies`.

## Preferred movie layout

The most reliable option is one folder per movie:

```text
Movies/
  Movie Title (2024)/
    Movie Title (2024).mkv
```

This layout works well when you also keep artwork, subtitles, or alternate editions alongside the movie.

Examples:

```text
Movies/
  Avatar (2009)/
    Avatar (2009).mkv
  Batman Begins (2005)/
    Batman Begins (2005).mp4
    Batman Begins (2005).en.srt
    poster.jpg
```

## Flat movie layout

Koko also supports movies stored directly inside the library root:

```text
Movies/
  Avatar (2009).mkv
  Batman Begins (2005).mp4
```

This can work well for smaller libraries, but folder-per-movie is still recommended when you have local assets.

## Optional metadata tags in braces

You can include helpful tags in curly braces after the movie title and year.

Supported examples:

```text
Batman Begins (2005) {tmdb-272}.mp4
Batman Begins (2005) {imdb-tt0372784}.mp4
Blade Runner (1982) {edition-Final Cut}.mkv
```

Useful tag forms:

- `{tmdb-272}`
- `{imdb-tt0372784}`
- `{edition-Director's Cut}`
- `{edition-Extended}`

Koko strips these tags before title matching and uses them as hints where possible.

## Multiple editions

If you keep more than one edition of the same movie, include the edition tag in the folder name, filename, or both.

```text
Movies/
  Blade Runner (1982) {edition-Director's Cut}/
    Blade Runner (1982) {edition-Director's Cut}.mp4
  Blade Runner (1982) {edition-Final Cut}/
    Blade Runner (1982) {edition-Final Cut}.mkv
```

## Split movie files

Split files are best kept in their own movie folder.

Koko recognizes common part suffixes such as:

- `cd1`, `cd2`
- `disc1`, `disc2`
- `disk1`, `disk2`
- `dvd1`, `dvd2`
- `part1`, `part2`
- `pt1`, `pt2`

Example:

```text
Movies/
  The Dark Knight (2008)/
    The Dark Knight (2008) - pt1.mp4
    The Dark Knight (2008) - pt2.mp4
```

## General naming tips

- Include the release year whenever possible.
- Prefer spaces over noisy scene-release formatting.
- Avoid leaving quality, codec, or release-group tags in the display title when you can place them elsewhere.
- Keep subtitle, poster, and backdrop files in the same movie folder when you use local assets.
- Keep folder and filename titles aligned to reduce ambiguous matches.

## Examples Koko matches well

```text
Movies/
  Alien (1979)/
    Alien (1979).mkv

Movies/
  Dune Part Two (2024) {tmdb-693134}/
    Dune Part Two (2024) {tmdb-693134}.mkv

Movies/
  Mad Max Fury Road (2015) {edition-Black and Chrome}/
    Mad Max Fury Road (2015) {edition-Black and Chrome}.mkv
```

