# Books, audiobooks, and comics

Create a **Books** library in **Settings → Libraries**, add your folders, and scan it.
Browse **Author → Book** to find ebook files and audiobook versions on the same book page.
Choose **Read** to open an ebook or **Listen** to play an audiobook. If you have several ebook
formats, choose one from the format selector. Reading and listening progress are independent,
and each narration has its own chapters and listening progress.

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

Embedded tags can supply titles, authors, narrators, disc numbers, and track numbers; folder names
supply the author and title when tags are missing. Local `cover.jpg`, `cover.png`, `folder.jpg`, or
`poster.jpg` can supply artwork. OPF sidecars are not imported.

### Audiobook versions

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

`CD 1`, `CD1`, `Disc 1`, and `Disk 1` folders work for multi-disc recordings. Number files in playback order.
Browse **Author → Book → Audiobook version → Audio files** to see a recording's files.

### Comics

Put each comic series in its own folder under `Comics`, for example:

```text
Books/
  Comics/
    Example Series/
      Example Series 001.cbz
      Example Series 002.cbr
```

Comics appear as **Series → Issue**. PDFs under `Comics` use this layout too. Number filenames in
reading order; `page2.jpg` comes before `page10.jpg` in an archive.

## Supported formats

### Ebooks and comics

| Files                                                                                                     | Requirements                                |
| --------------------------------------------------------------------------------------------------------- | ------------------------------------------- |
| EPUB, KEPUB, MOBI, AZW, AZW3, supported PRC variants, FB2, FBZ, FB2.ZIP                                   | No additional software                      |
| PDF                                                                                                       | Use an unlocked PDF                         |
| CBZ, CBR, CB7, CBT                                                                                        | Use an unencrypted comic archive            |
| TXT, TEXT, standalone HTML, HTM, XHTML                                                                    | Package linked images and styles in an EPUB |
| AZW4, CBC, CHM, DJVU, DJV, DOCX, HTMLZ, LIT, LRF, ODT, PDB, PML, PMLZ, RB, RTF, SNB, TCR, TXTZ            | Install Calibre on the server               |

DRM-protected books, KFX, and ACSM files cannot be read directly. Some PRC/PDB variants are not supported.
DjVu conversion needs a text layer. Search and highlighting require selectable text; scanned images do not provide it.
Comic pages can use JPEG, PNG, GIF, BMP, WebP, AVIF, or SVG, depending on your browser.

For formats that need conversion, install [Calibre](https://calibre-ebook.com/download) **on the server**.
Set **Settings → General → Calibre ebook-convert path** to its `ebook-convert` executable, or leave it
at the default if the executable is on the server's PATH. Increase the conversion timeout there if a book
needs more time. Your original library files are not changed.

### Audiobooks

Koko plays unencrypted M4B, MP3, M4A, FLAC, AAC, Ogg/Opus, WAV, and other supported audio files.
DRM-protected downloads cannot be played directly.

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

Open Library supplies book and author details. Refreshing book metadata automatically adds any
matching [curated Open Library collections](https://openlibrary.org/collections); personal patron lists
are not imported.

### Audiobook recordings

On an audiobook, use **Choose recording** to compare narrators, language, release year, abridgement,
runtime, provider, and cover before choosing a version. Matching a book title alone may not identify
your narration. If Koko cannot confidently identify the recording automatically, choose it here.
AudioSilo can add information to a matching Libex recording, and can also be selected directly.

Metadata availability differs by source. LibriVox is useful for its own public-domain recordings;
commercial recordings may be available through LibexDB or AudioSilo instead.

### Comic metadata

Comic Vine needs a personal [API key](https://comicvine.gamespot.com/api/), entered under
**Settings → Metadata providers**. On a series or issue, use
**More → Details and management → Search metadata** to choose its publishing run or issue.
Reading comics does not require a metadata account. `ComicInfo.xml` import is not supported yet.

Comic Vine has [non-commercial usage terms](https://comicvine.gamespot.com/api/).

## Reader controls

All controls are in the bottom bar. **Reading options** opens contents, search, bookmarks, highlights
and notes, text size or zoom, reading direction, and the two-page toggle.

- Turn pages with the arrow buttons, arrow keys, Page Up/Down, or Space/Shift+Space. Home/End jump to the beginning/end.
- Hover over the progress slider to preview a destination page, or drag it to move through the book. PDF and comic pages also support horizontal swipes at normal zoom.
- Two-page mode places the cover on the right for left-to-right reading, followed by even/odd page pairs.
- **Read** becomes **Resume** after you start a book. Hold or right-click it to reset your position while keeping your bookmarks, highlights, and notes.
- Use the fullscreen icon to fill your screen, and the X button or Escape to close the reader.

Ebook cards show a page count after you open the file. Text-book totals can change with the window,
text size, or layout. Wait for pagination to finish to see the current total. It can differ from a printed edition.

If a book cannot open in an embedded browser, try a full browser. A missing contents list means the
file has no usable table of contents. Comic search finds page filenames; it does not search text inside images.

## Bookmarks, highlights, and notes

Choose **Add bookmark** in reading options, enter a name, and save. Open **Bookmarks** to jump to,
rename, or remove a bookmark.

Select a passage to reveal the highlight and note buttons. **Highlight selection** marks the text;
**Note on selection** lets you write a note and choose a highlight color. To write without selecting
text, choose **Add note to page** in reading options. Open **Highlights and notes** to return to a
passage, edit a note or color, or remove a mark. You can also click a highlight to edit it.

Your reading position, bookmarks, highlights, and notes are private to your Koko account. Other users
cannot see them. Changes are saved automatically. Resetting your position keeps your bookmarks,
highlights, and notes. Replacing a file with different content starts fresh reading state.

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
unfinished recording. Audiobook card badges show the total runtime.

## Free audiobooks

[LibriVox](https://librivox.org/) offers downloadable public-domain recordings, including:

- [Common Sense](https://librivox.org/common-sense-by-thomas-paine)
- [How I Filmed the War](https://librivox.org/how-i-filmed-the-war-by-geoffrey-h-malins/)
- [United States Historical Documents](https://librivox.org/us-historical-documents/)

Download the original audio files or an available M4B. Check the recording's terms for your location;
LibriVox's public-domain designation applies to the USA.
