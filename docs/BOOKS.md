# Reading books and comics

Create a **Books** library in **Settings → Libraries**, add your folders, and scan it.
Open a book and choose **Read**. If you have several ebook formats, choose one from the format selector.
Books with audio versions also offer **Listen**. Reading and listening progress are independent.

## Supported formats

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

## Reader controls

All controls are in the bottom bar. **Reading options** opens contents, search, bookmarks, highlights
and notes, text size or zoom, reading direction, and the two-page toggle.

- Turn pages with the arrow buttons, arrow keys, Page Up/Down, or Space/Shift+Space. Home/End jump to the beginning/end.
- Drag the progress slider to move through the book. PDF and comic pages also support horizontal swipes at normal zoom.
- Two-page mode places the cover on the right for left-to-right reading, followed by even/odd page pairs.
- **Read** becomes **Resume** after you start a book. Hold or right-click it to reset your position while keeping your bookmarks, highlights, and notes.
- Use the fullscreen icon to fill your screen, and the back arrow or Escape to close the reader.

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

Your bookmarks, highlights, notes, and reading position are private to your Koko account. Other users
cannot see them. They are saved automatically and kept when you reset your reading position.
Replacing a file with different content starts fresh reading state.

## Comics and metadata

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

See [metadata sources](./AUDIOBOOKS.md#metadata-sources) for the default provider order.
Comic Vine needs a personal [API key](https://comicvine.gamespot.com/api/), entered under
**Settings → Metadata providers**. On a series or issue, use
**More → Details and management → Search metadata** to choose its publishing run or issue.
Reading comics does not require a metadata account. `ComicInfo.xml` import is not supported yet.

Open Library supplies book and author details. Refreshing book metadata automatically adds any
matching [curated Open Library collections](https://openlibrary.org/collections); personal patron lists
are not imported. For folder layouts, audio versions, and listening controls, see [Audiobooks](./AUDIOBOOKS.md).
