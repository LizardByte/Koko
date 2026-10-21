# Audiobooks

Create a **Books** library in **Settings → Libraries**, add your folders, and scan it.
Browse **Author → Book → Audiobook version → Audio files**. Ebook files appear on the same book page.
Each narration has its own chapters and listening progress.

Koko plays unencrypted M4B, MP3, M4A, FLAC, AAC, Ogg/Opus, WAV, and other supported audio files.
DRM-protected downloads cannot be played directly.

## Organizing files

You can keep ebooks and audiobooks in separate folders within one Books library:

```text
Books/
  Audiobooks/
    Author/
      Book Title/
        CD 1/
          Track 01.mp3
          Track 02.mp3
        CD 2/
          Track 01.mp3
  Books/
    Author/
      Book Title/
        Book Title.epub
```

Or combine them in one book folder:

```text
Books/
  Author/
    Book Title/
      Book Title.epub
      Book Title.m4b
```

Add the top-level folder as a library path, or add the separate type folders as paths in the same library.
Use the same author and book folder names to bring ebook and audio versions together. Author folders are
optional; books without an author appear under **Unknown author**.

For multiple narrations, give each recording a version folder:

```text
Author/
  Book Title/
    [Reader One - Unabridged]/
      CD 1/
        Track 01.mp3
      CD 2/
        Track 01.mp3
    [Reader Two - 2020]/
      Book Title.m4b
```

`CD 1`, `CD1`, `Disc 1`, and `Disk 1` folders work for multi-disc recordings. Number files in playback
order. Embedded tags can supply titles, authors, narrators, disc numbers, and track numbers; folder names
supply the author and title when tags are missing. Local `cover.jpg`, `cover.png`, `folder.jpg`, or
`poster.jpg` can supply artwork. OPF and `ComicInfo.xml` sidecars are not imported.

## Metadata sources

New Books libraries enable these sources in this order. Choose sources and their priority under
**Settings → Libraries**; manage provider credentials under **Settings → Metadata providers**.
Existing custom source choices are kept.

| Source         | Provides                                                   | Setup                |
| -------------- | ---------------------------------------------------------- | -------------------- |
| Comic Vine     | Comic series, issues, descriptions, and covers             | Personal API key     |
| Open Library   | Books, author profiles, covers, and curated collections    | No API key           |
| LibexDB        | Audiobook details and recording choices                    | No API key           |
| LibriVox       | Volunteer narrations, readers, covers, and section markers | No API key           |
| AudioSilo Meta | Audiobook details and recording-specific chapters          | No API key           |

For a missing or incorrect match, open **More → Details and management → Search metadata**.
**Add source** adds or replaces that source. **Unlink metadata** clears the selected sources.
Author pages have their own metadata search for names, biographies, portraits, and dates.

On an audiobook, use **Choose recording** to compare narrators, language, release year, abridgement,
runtime, provider, and cover before choosing a version. Matching a book title alone may not identify
your narration. If Koko cannot confidently identify the recording automatically, choose it here.
AudioSilo can add information to a matching Libex recording, and can also be selected directly.

Metadata availability differs by source. LibriVox is useful for its own public-domain recordings;
commercial recordings may be available through LibexDB or AudioSilo instead. Comic Vine requires an API
key and has [non-commercial usage terms](https://comicvine.gamespot.com/api/).

## Chapters and listening

A CD track is not necessarily a chapter: a chapter can begin halfway through a file. M4B files often
include chapter markers, while MP3 downloads and CD rips may not. Koko uses embedded markers when
available. Provider markers need the correct recording and compatible file timings. If a recording or
timing warning appears, check **Choose recording**. You can still use the **Audio files** list and seek
normally without markers. LibriVox sections may contain more than one printed chapter.

Choose **Listen** or **Resume listening** to start the bottom player. Playback continues while browsing,
advances through audio files, and offers chapter navigation and speeds from 0.5× to 3×.
Hold or right-click the listening action on either the book or audiobook page to **Restart track** or
**Restart audiobook**. Reading progress is unaffected.

Listening progress is private to each user and narration. **Continue listening** returns to the latest
unfinished recording. Audiobook card badges show the total runtime; ebook badges show pages after
opening the file in the [reader](./BOOKS.md).

## Free audiobooks

[LibriVox](https://librivox.org/) offers downloadable public-domain recordings, including
[Common Sense](https://librivox.org/common-sense-by-thomas-paine),
[United States Historical Documents](https://librivox.org/us-historical-documents/), and
[How I Filmed the War](https://librivox.org/how-i-filmed-the-war-by-geoffrey-h-malins/).
Download the original audio files or an available M4B. Check the recording's terms for your location;
LibriVox's public-domain designation applies to the USA.
