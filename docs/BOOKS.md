# Reading books and comics

Create a **Books** library, scan its folders, open a book, and choose **Read**. The shared book page offers a format selector when multiple ebook files are present, alongside **Listen** for audiobook versions. **Resume listening** returns to the most recently started unfinished narration at its saved audio file and position. Reading positions and bookmarks belong to each account and remain independent of listening history. Home shows **Continue reading** for unfinished books you can access.

Hold or right-click the listening action on either the Book page or audiobook page to choose
**Restart track** or **Restart audiobook**. The track action starts the current live file, or the
saved resume file, at zero; the audiobook action begins the narration's ordered audio files.
These options remain available while playing or paused. Audiobook files use the same compact
rows as music, with separate numbering, listening indicators, duration, play/pause, and a file
details action in the row menu. Disc numbers appear when the recording spans multiple discs.

## Formats

| Files | Reader |
| --- | --- |
| EPUB, KEPUB | Foliate, with chapter navigation, search, text selection, font size, and fixed layout support |
| MOBI, AZW, AZW3, supported PRC variants | Foliate's Kindle decoder |
| FB2, FBZ, FB2.ZIP | Foliate's FictionBook decoder |
| PDF | PDF.js, with text selection, search, and document outlines when present |
| CBZ, CBR, CB7, CBT | Comic archive reader for ZIP, RAR 4/5, 7-Zip, and TAR |
| TXT, TEXT, standalone HTML/HTM/XHTML | Sanitized text reader |
| AZW4, CBC, CHM, DJVU/DJV, DOCX, HTMLZ, LIT, LRF, ODT, PDB, PML/PMLZ, RB, RTF, SNB, TCR, TXTZ | Server conversion to EPUB using an optional Calibre installation |

The archive reader accepts JPEG, PNG, GIF, BMP, WebP, AVIF, and SVG pages, subject to the browser's image decoders. Pages use natural filename order, so `page2.jpg` precedes `page10.jpg`. Hidden entries and `ComicInfo.xml` are not pages. Pages are extracted on demand; oversized pages and archives with more than 20,000 image entries are rejected. Password-protected comic archives are not supported.

Not every file with a supported extension is readable: Kindle DRM and other protected ebooks need a readable DRM-free copy; KFX and ACSM are not reader formats. Calibre supports only certain PRC/PDB variants, and DjVu conversion requires an existing text layer. Image-only documents cannot gain searchable text without OCR. A chapter list requires a table of contents in the file. PDF passwords and arbitrary loose HTML asset directories are not handled; use an unlocked PDF or a packaged EPUB/HTMLZ copy.

For legacy conversion, install [Calibre](https://calibre-ebook.com/download) **on the server**, then set **Settings → General → Calibre ebook-convert path** to its `ebook-convert` executable. The default searches the server's PATH. Adjust the conversion timeout in the same section if necessary. Koko runs at most two conversions together, stores results under its data directory's `cache/reader`, and reuses them while the file fingerprint remains unchanged. Conversion copies the input into a cache directory and leaves the library file unchanged. Calibre is optional and is not bundled with Koko. See its [format support and limitations](https://manual.calibre-ebook.com/faq.html#what-formats-does-calibre-support-conversion-to-from).

## Reader controls

To check a calculated page total, use single-page mode and wait for pagination to finish. Keep
the window and text size unchanged, and start at the beginning with Home. Each settled Next action advances one rendered page; the
last page should agree with the badge's total. The background calculation cross-checks occupied
columns against the paginator's actual navigation-screen count before saving a badge. Single-page
totals come directly from those navigation screens. A different
window size or text size can legitimately produce a different total. Byte-based EPUB locations
and the page count of a printed edition are separate measurements.

- **Contents**, **Search**, and **Bookmarks** open the navigation panel. Add bookmarks from the current location and remove them from the bookmark panel. Comic search searches page filenames; it does not perform OCR.
- All controls are icons in the bottom controller. **Reading options** opens contents, bookmarks, search, size, reading direction, and the two-page toggle. PDF and comic size controls zoom the page. The reader follows Koko's appearance; size and layout preferences are remembered in this browser for the signed-in account.
- Use Previous/Next, arrow keys, Page Up/Down, Space/Shift+Space, or the mint position slider. Hover to preview a fixed page or a text percentage. Dragging moves the document and shows its actual rendered page or location; rapid movement keeps the latest destination. Home/End move to the beginning/end. Comic and PDF pages also support horizontal swipes at normal zoom. Two-page spreads put the cover on the right in left-to-right reading, followed by even/odd page pairs. Text selection remains available.
- **Read** becomes **Resume** when this account has a saved position. Hold the action (or right-click it) to reset reading progress while retaining bookmarks. Close with the bottom back arrow or Escape. Changed file content invalidates old positions. Moving the same library retains its reading history.
- Audiobook badges show the total runtime of the local narration. Ebook badges show page counts after opening a book: PDF/comic and fixed-layout EPUB pages, or a background calculation of every reflowable text section. Calculated counts use the current reader font size, window size, and layout; changing these recalculates the count. Completed totals are saved with account reading state and shown on cards. The reader uses that same total and shows the current page or spread once pagination finishes. EPUB byte-based locations and publisher page lists do not replace the rendered-page count. File formats remain in the format selector. Saved text positions use stable locations independently of the layout's page count.

Reader content and progress routes require a signed-in account with access to the item's library. Books are sanitized before rendering; executable markup and remote resources are blocked. Reader workers, fonts, and PDF resources are bundled and served locally. An embedded browser that blocks document frames displays a loading error; open Koko in a full browser in that case.

## Comics and metadata

Open Library curated collections are created automatically when book metadata is matched or
refreshed. Koko reads the pages under Open Library's `collections/` catalog and resolves their
explicit work selections, search queries, and referenced lists. Personal reading lists are not
imported unless a curated collection page uses that list. Membership uses the matched Open Library
work ID, and each collection shows the shared Book once across ebook formats and audiobook versions.
Refreshing updates memberships; unavailable collection lookups preserve existing groups and still
allow the book's other metadata to update. Catalog and membership requests use the metadata response
cache and Open Library's request limit, so initial collection discovery can take longer than a basic
book lookup.

Use a `Comics` type folder alongside Jellyfin-style `Books` and `Audiobooks` folders, or add a comic folder as a Books library root:

```text
Books/
  Comics/
    Example Series/
      Example Series 001.cbz
      Example Series 002.cbr
  Books/
    Author/
      Book/
        Book.epub
```

Comic archives appear as **Series → Issue**, separately from **Author → Book → Audiobook version**. PDFs placed under a `Comics` folder use the comic hierarchy too. Keep each series in its own folder. Files directly in a library root remain readable, but a series folder gives them a useful grouping.

Open Library remains the written-book and author source. Comic issues need issue and volume identities, so Koko also offers [Comic Vine](https://comicvine.gamespot.com/api/). Enable it in **Settings → Metadata providers**, enter a personal API key, and select it for the Books library. On a series or issue, use **More → Details and management → Search metadata** to choose the correct volume or issue. Series and issue identifiers are distinct, and rematching a series preserves independently linked issues. Search is manual to avoid selecting the wrong publishing run. Reading comics does not require a metadata account.

Comic Vine supplies titles, issue numbers, descriptions, publication years, and covers, with provider attribution. Requests are cached and throttled within the documented 200 requests per resource per hour. Its API is for non-commercial use; review the provider's terms before enabling it. Local `ComicInfo.xml` metadata import is not implemented yet.

For folder compatibility, audiobook versions, recording identity, and author profiles, see [Audiobooks](./AUDIOBOOKS.md). Reader engines: [Foliate JS](https://github.com/johnfactotum/foliate-js), [PDF.js](https://mozilla.github.io/pdf.js/), and [libarchive.js](https://github.com/nika-begiashvili/libarchivejs).
