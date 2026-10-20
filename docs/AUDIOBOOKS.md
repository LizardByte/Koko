# Audiobooks

Create a **Books** library in **Settings → Libraries** and add your book folders. Books libraries accept both ebooks and audio files. Browse **Author → Book → Audiobook version → Audio files**. Ebook files appear alongside audiobook versions on the book page. Each narration has its own files, chapters, and listening progress. Audiobook covers use square frames, with portrait book covers fitted without cropping.

Koko supports unencrypted M4B, MP3, M4A, FLAC, AAC, Ogg/Opus, WAV, and the other audio formats supported by its scanner. FFprobe reads duration, tags, and embedded chapters. Playback uses the browser directly when supported and FFmpeg otherwise. DRM-protected downloads are not supported.

## Organizing files

Use the same type → optional author → book folders described in [Jellyfin's books guide](https://jellyfin.org/docs/general/server/media/books/). Add the top-level `Books` folder as one Books library, or add its type folders as multiple paths in that same library. A single recording needs no extra version directory. For an untagged CD rip, this layout works without editing the audio files:

```text
Audiobooks/
  Author/
    Book Title/
      cover.jpg
      CD 1/
        Track 01.mp3
        Track 02.mp3
      CD 2/
        Track 01.mp3
        Track 02.mp3
```

`CD 1`, `CD1`, `Disc 1`, and `Disk 1` folders belong to the containing book, including when you add that book folder directly as a library root. Files play in disc order. Track tags determine file order only when every file on that disc has a unique track number; otherwise the entire disc uses natural numeric filename order. This handles partially tagged LibriVox downloads without moving one tagged file ahead of the introduction. A five-CD recording with nine tracks per CD becomes one audiobook with 45 audio files. Embedded album, author/composer/artist, narrator/read-by/performer, language, series, disc, and track tags are used when present. Folder names provide the book title and author when tags are missing.

Jellyfin-style libraries can keep ebooks and audiobooks in separate type folders; matching author and book folder names bring them together in Koko:

```text
Books/
  Audiobooks/
    Author/
      Book Title/
        [Reader One - Unabridged]/
          CD 1/
            Track 01.mp3
          CD 2/
            Track 01.mp3
        [Reader Two - 2020]/
          Book Title.m4b
    Another Book/
      Another Book.mp3
  Books/
    Author/
      Book Title/
        Book Title.epub
```

You can also combine both formats in one book directory:

```text
Books/
  Author/
    Book Title/
      Book Title.epub
      Book Title.m4b
```

Both layouts produce the same author and book hierarchy. The single recording can sit directly in the book directory, including CD subfolders. Optional narrator/version subfolders work in the combined layout too.

Folder author and title establish the shared book hierarchy. Embedded tags remain independent matching hints, with folder hints retained if tags are incorrect. Leading `Audiobooks`, `Books`, and `Comics` folders are organizational and do not become authors. The author folder is optional; tags provide the author when available, otherwise the book appears under **Unknown author**. For multiple narrations, optional version subfolders keep their files separate. Use `[Narrator - Edition]` or `Version - Narrator` when omitting the author folder, so the version cannot be mistaken for a book folder. Files directly in a library root remain supported as separate single-file recordings. Local `cover.jpg`, `cover.png`, `folder.jpg`, or `poster.jpg` artwork can accompany the recording.

The on-disk layout does not need to mirror every level of Koko's catalog. Existing single-recording Jellyfin folders do not need renaming or moving. After upgrading an existing Koko library, scan it to build the author and book containers. Existing recording and audio-file IDs, metadata links, and listening progress are preserved. Jellyfin sidecars (`content.opf`, `metadata.opf`, and `ComicInfo.xml`) are not yet imported; folder layout compatibility does not imply full metadata migration.

## Chapters and CD tracks

A CD track is a physical file boundary. A chapter is a position in the narration: it can start halfway through a track and continue across multiple files. Koko keeps these concepts separate and does not label CD tracks as chapters.

M4B files can contain a whole book with embedded chapter marks. MP3 collections and CD rips may have tags but no chapter marks, or no tags at all. Koko reads any embedded chapters reported by FFprobe, including chapter starts in the middle of a file. Without reliable chapter data, use the **Audio files** list and normal seeking controls.

## Metadata sources

New Books libraries default to **LibexDB**, **Open Library**, and **AudioSilo Meta**. None requires an API key. For an existing library, enable the desired sources under **Settings → Libraries**.

| Source | Purpose |
| --- | --- |
| LibriVox | Volunteer recording projects, readers, cover art, and supplied section timings. |
| LibexDB | Primary audiobook metadata, with separate book-only and recording matches. |
| Open Library | Author biographies and portraits, plus written-work identity, description, subjects, and book covers. |
| AudioSilo Meta | Secondary factual enrichment of a Libex match using its exact ASIN, including series and recording-specific chapters. |

**More → Details and management** offers separate sources with an author search field. **Add source** adds or replaces only that provider, preserving the other selected book and recording sources. AudioSilo remains manually searchable, including when its work or recording must be chosen explicitly. **Unlink metadata** clears all selected sources. Library order controls primary-source priority; secondary facts fill gaps rather than replacing the primary identity.

LibriVox is included in the defaults for new Books libraries. For an existing library, enable
**LibriVox** in its metadata sources and refresh metadata. Custom source choices are preserved.
Refresh also checks existing unconfirmed book-only matches for a LibriVox recording; a confirmed
narration remains selected. The public API needs no key and requests are limited to one per second.

Automatic LibriVox matching checks title and author using embedded and folder hints independently.
It confirms a project only when the runtime agrees and either the original section filenames and
individual durations match, or the narrator agrees. Otherwise it keeps a book-only match and offers
the available projects in **Choose recording**. Each project keeps its own narrator and cover.
LibriVox's `copyright_year` belongs to the source text and is not used as the recording's release year.

LibriVox section starts are the available chapter markers: a section may contain several printed
chapters. Koko does not invent boundaries within that section. Original filenames, including MP3
bitrate variants, let markers seek to the corresponding local file's actual start despite encoder
padding. Renamed or combined files use the provider timeline only when the complete runtime agrees.
Missing or incomplete timing data leaves playback available without provider markers.
See the [LibriVox API documentation](https://librivox.org/api/info).

Libex uses the public endpoint at `https://libexdb.com`. The provider's preferred locale chooses the Audible marketplace when searching, defaulting to the US. Region and ASIN remain part of the saved identity.

Libex search offers **Book details only** separately from each recording. Automatic title/author matching supplies title and author without assigning a commercial recording's narrator, cover, marketing description, release date, or chapter timeline. It accepts punctuation, publisher subtitles, and an author possessive prefix such as `Glenn Beck/Common Sense` matching `Glenn Beck’s Common Sense`; the paired author must still agree. If embedded title/author tags fail, the folder title/author pair is tried independently.

On an audiobook version, use **Choose recording** to compare narrator, language, release year, abridgement, runtime, and source. Each choice shows its provider attribution and that recording's cover when supplied. Selecting a recording confirms which provider timeline belongs to the local files. Provider priority does not change that choice. Existing links without a confirmed recording stay unconfirmed until you choose one; embedded chapters still work. Explicit recording matches use accurate chapter data; estimated timelines are ignored.

For example, LibriVox's [How I Filmed the War](https://librivox.org/how-i-filmed-the-war-by-geoffrey-h-malins/) is read by R. S. Steinberg and runs 8:23:18. Libex also lists the book, but one Audible recording is read by Felbrigg Napoleon Herriot and runs about 6:32. A matching book title does not make these the same recording.

Libex metadata is sourced from Audible and attributed to Libex/Audible. The Libex software's MIT license is separate from the provenance of the catalog metadata. See the [Libex project](https://github.com/LibexHQ/Libex).

### Open Library

[Open Library](https://openlibrary.org/) identifies the written work independently of narrator or audio-file layout. The shared book page matches through Open Library when enabled; the audiobook version matches through Libex. An explicitly selected Open Library work is preserved on subsequent refreshes. Audiobook versions inherit that written-work link. Ebook files and audiobook versions share a book through their local author/title hierarchy.

Author pages also use Open Library for the canonical name, biography, portrait, and available birth/death dates. Automatic matching accepts one exact name or documented alias; ambiguous names require **More → Details and management → Search metadata** on the author page. The author profile has its own match and refresh. Replacing or unlinking it preserves metadata and recording selections on the author's books. Author portraits are kept separate from book covers.

Open Library does not provide audio chapter times or select a narrator. Reading and listening progress remain independent; the [book reader](./BOOKS.md) provides ebook and comic reading, while manual regrouping of works remains planned. Requests are cached and limited to one per second according to the [API usage policy](https://openlibrary.org/developers/api).

### AudioSilo Meta

After a Libex match, AudioSilo resolves the ASIN to the corresponding work. A book-only Libex match imports work-level facts; an explicitly selected recording can import that recording’s narrator and chapters. An unavailable AudioSilo entry leaves the Libex match intact. Manually selected AudioSilo sources are preserved and refreshed instead of being replaced by automatic ASIN lookup.

Search results distinguish a work from its available recordings, including narrator and release information. Selecting a work supplies book metadata. Select the specific recording to obtain narrator details and its chapter timeline. Koko never assumes a narrator or recording solely from a matching book title.

Recording chapter offsets are mapped onto local audio files only when all files have known durations and the combined duration agrees with the chapter timeline within two seconds. The page distinguishes an unconfirmed recording, absent provider chapters, incomplete files, and a timing mismatch. A mismatch shows both durations; timings are not stretched to fit another edition. Matching duration alone cannot prove an identical recording, so select the correct narrator and edition. Embedded file chapters take precedence.

Factual metadata comes from [AudioSilo Meta](https://meta.audiosilo.app/) under CC0 1.0. This integration does not import the community descriptions, characters, or recaps licensed under CC BY-SA 4.0. See the [API reference](https://meta.audiosilo.app/docs/api).

## Listening

Choose **Listen** or **Continue audiobook** to start the persistent bottom player. It continues while browsing, advances to the next available audio file, offers chapter navigation and playback speeds from 0.5× to 3×, and preserves pitch through the browser's normal audio handling. Music shuffle, repeat, lyrics, and playlist controls are hidden for audiobooks.

Listening positions are stored separately for each user and recording. **Continue listening** shows one book card, resumes the latest listened file at its saved position, advances after a completed file, and removes the card after the final file finishes. **Restart audiobook** begins with the first available file. Playback speed remains a browser-session choice. Reading progress is not modified.

## Free files for testing

[LibriVox](https://librivox.org/) offers public-domain recordings with downloadable audio files. Useful examples include:

- [Common Sense](https://librivox.org/common-sense-by-thomas-paine): downloadable sections and an M4B option for comparing file-based and single-file playback.
- [United States Historical Documents](https://librivox.org/us-historical-documents/): short sections and a small M4B download.
- [The Adventures of Sherlock Holmes, version 2](https://librivox.org/the-adventures-of-sherlock-holmes-by-sir-arthur-conan-doyle/): longer multipart material, including stories split across sections.

LibriVox marks its recordings as public domain in the USA; check the recording's terms for your location. Its recordings may not have a matching AudioSilo entry, so test local scanning/playback independently from provider coverage. The original `commonsense_0_paine_64kb.mp3` through `commonsense_6_paine_64kb.mp3` files play in that order without renaming, even though only some contain track tags. Use a copy of test files when removing tags or reorganizing them into simulated CD folders.
