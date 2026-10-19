# Music libraries

See [Music naming](MUSIC_NAMING.md) for folder layouts and embedded tags.

Create a library in **Settings → Libraries**, choose **Music**, add one or more folders, and select **MusicBrainz** as its metadata provider. No API key is required for MusicBrainz. New libraries scan and match metadata automatically. Subsequent scans fetch metadata only for newly discovered items; use **Refresh metadata** to refresh existing items or retry earlier failed matches. This scan behavior applies to every library type. Library access permissions apply to music browsing, artwork, and playback.

Koko groups music into **artist → album → track**. When embedded tags are enabled, album artist tags keep compilations together even when individual tracks credit different performers. Open an album to see tracks in disc and track order, and choose **Play album** to play its available tracks sequentially. Playing a track continues through the remaining tracks on that album.

**Embedded music tags are ignored by default.** Each library has a **Use embedded music tags** option. When enabled, FFprobe reads embedded title, artist, album artist, album, track number, disc number, date, and genre tags. FLAC/Vorbis-style names such as `ALBUMARTIST`, `TRACKNUMBER`, and `DISCNUMBER` and their common underscore variants are supported. Track and disc values may include totals, such as `3/12`.

When tags are excluded or absent, use this layout:

```text
Music/
  Artist/
    Album/
      01 - First track.flac
      02 - Second track.flac
    Another album/
      Disc 1/
        01 - First track.flac
      Disc 2/
        01 - Another track.flac
```

`CD 1`, `Disc 1`, and `Disk 1` folders are recognized. Disc folders prefixed by their parent album name also work, such as `Artist/Pet Sounds/Pet Sounds - CD 01/01 Song.flac`; all discs belong to one album. Untagged files without an artist or album folder appear under Unknown Artist or Unknown Album. Local `cover.jpg`, `folder.jpg`, or `poster.jpg` artwork is supported alongside files. MusicBrainz release artwork comes from the Cover Art Archive when a front cover is available.

Supported audio extensions include MP3, FLAC, AAC, WAV, Ogg, Opus, M4A, AIFF, ALAC, WMA, APE, and WavPack. Browser support determines whether playback uses the original file or FFmpeg audio transcoding.

When embedded tags are enabled, MusicBrainz identifiers can provide precise metadata matching, for example after tagging with MusicBrainz Picard:

- `MUSICBRAINZ_ALBUMARTISTID` (or `MUSICBRAINZ_ARTISTID`): album artist UUID
- `MUSICBRAINZ_ALBUMID`: release UUID, which identifies a particular album edition
- `MUSICBRAINZ_TRACKID`: recording UUID

Without identifiers, Koko searches using titles and artist credits. Album matching also considers the total number of local tracks across all discs. Automatic matching requires a unique high-confidence result with the same track count; when several editions match but all belong to the same release group, Koko links that group without guessing a pressing. Ambiguous names and releases stay unmatched. An administrator can use the item's metadata management controls to select a MusicBrainz result manually; results show track counts, flag differences from the local album, and rank count matches higher. Incomplete albums can still be linked manually. Unlinking an artist or album clears its descendant metadata and prevents automatic relinking until an explicit rematch.

Album metadata searches include an editable **Artist / group** field, initially filled from the parent artist's displayed name. Change it to search a different artist credit, or clear it for a title-only search. This search filter does not rename or rematch the parent artist.

Open **Details and management → Source files** to inspect the server's full file paths, library folders, relative paths, original catalog titles, missing-file status, and available technical details. For artists, albums, series, and other groups, this includes files belonging to their children. Source files load on demand in pages, rather than adding large file lists to normal browsing responses. This panel is shared by all media types.

The server uses the [`musicbrainz_rs`](https://crates.io/crates/musicbrainz_rs) client for MusicBrainz searches and lookups. MusicBrainz requests share a rate limit of at most one request per second, including retries. Large libraries can therefore take time to match. Existing metadata remains available while background work runs. See the [MusicBrainz API documentation](https://musicbrainz.org/doc/MusicBrainz_API) and [Cover Art Archive API](https://musicbrainz.org/doc/Cover_Art_Archive/API).

## Movies and shows featuring a music artist

An artist's **Movies and shows** section lists matching credits from your libraries. Shows expand into the credited seasons and episodes. Open a film or episode's cast or crew member to find their **Albums in your libraries** section.

Koko connects MusicBrainz artists with TMDB and TVDB people using shared registry identifiers such as IMDb and Wikidata, including connections through another provider. Names and shared social accounts are not sufficient to link people. Only items in libraries you can access are shown. This lists media in your collection, rather than a complete online filmography.

Groups show a **Members** people row when MusicBrainz lists [band membership relationships](https://musicbrainz.org/relationship/5be4c609-9afa-4ea0-910b-12ffb71e3821); solo artists show a **People** row with the performer. Open a person to see their available details and individual screen credits. Members can include instrument/vocal roles, membership dates, former membership, and separate periods when someone rejoined. The person's **Groups in your libraries** section links back to their groups. Membership is a credit, so a member's screen appearances are not attributed to the group itself. MusicBrainz's community records may be incomplete, and unavailable member details do not prevent group metadata from refreshing.

Person pages show albums from matching artists directly and reuse their available artist portraits and backgrounds. A member keeps their own portrait rather than borrowing the group's photo. Empty screen-credit sections are hidden.

The combined Home feed uses **Continue playing**. Each library retains its specific label: **Continue listening** for music, **Continue reading** for books, and **Continue watching** for video. Music progress groups tracks into one album card per user. Continue rows across all media types show the most recently played item first, with temporary radio cards ordered alongside albums and other media by their last playback activity. Open the album and choose **Continue album** to resume the most recently played track at its saved position. **Restart album** begins at the first available track. An active playing queue, including its shuffle order and previous-song history, continues unchanged while browsing other pages. Returning to an album after stopping playback starts an album queue from its saved song; it does not reconstruct a former shuffled queue.

Every library type offers **Exclude from Home**, both when adding a library and under **Settings → Libraries**. It hides the library's content from Home's continue row, recently added, recommendations, featured previews, and collections. A radio card is also hidden if its original artwork source or current song belongs to an excluded library; its saved queue remains intact. The library stays in the sidebar, remains searchable, and retains its own browse and recommendation pages.

## Audio player and playlists

Music plays in a compact bottom bar that stays mounted independently of page content. Browse movies, settings, albums, or other pages while it plays. The bar includes play/pause, previous/next song, ten-second backward/forward seeks, a timeline, volume/mute, shuffle, and repeat off/all/one. Its buttons and sliders share the video player's styling. Scroll over volume to adjust it, or over the timeline to seek. Previous restarts a song after three seconds; press it again immediately to return to the previous song. Browser media-session controls share the album artwork, playback state, position, seeking, and queue controls. The browser and operating system decide which controls are displayed.

Use a song's action menu or an album's **More** menu to **Play next** or **Add to queue**. The queue opens in a right-side panel and centers the current song. On wide screens, the panel occupies its own column and the page resizes beside it. On smaller screens it overlays the space above the player. It supports jumping to a song, reordering or removing upcoming entries, and clearing upcoming songs. Repeated songs are preserved as separate entries. Shuffle changes upcoming order without rewriting played history. Albums and individual tracks show listening progress and completed indicators; track numbers stay visible beside the indicators. The current track is highlighted and its progress updates live, including its completed checkmark. Its row and the active album button pause or resume the existing queue. Sign-out or switching accounts stops music and clears the active player.

In the queue panel, enter a name and choose **Save as playlist**. The **Playlists** tab can reopen, play, or delete private playlists on the server. Playlists preserve song order and repeated entries in relational database rows; inaccessible or deleted items are excluded when reading them. An ordinary playing queue lasts for the current browser session; save it as a playlist to revisit it after a reload or on another device. Radio queues also have temporary server-side resume state, described below.

### Lyrics and radio

MusicBrainz supplies [links to lyrics sites](https://musicbrainz.org/doc/Style/Relationships/URLs/Lyrics_whitelist), rather than lyric text. Select **LRCLIB** under a music library's **Metadata sources** to acquire lyrics during track metadata refreshes. This key-free secondary agent uses the stored name, artist, album, and duration and does not require a MusicBrainz match or another primary provider. Existing libraries must select it explicitly. Koko saves plain lyrics, synchronized LRC text, instrumental status, attribution, and lookup status in dedicated database columns. **Playback only reads saved lyrics**; starting a song and opening lyrics never contact LRCLIB or save lookup outcomes. Disabling LRCLIB prevents subsequent provider requests without deleting saved lyrics. Successful results remain saved until a metadata refresh or a change to the track's identity. Missing lyrics and temporary failures have seven-day and fifteen-minute backoffs during automatic metadata work; explicit metadata refresh bypasses them. Temporary provider failures preserve previously saved lyrics. Scans fetch lyrics only for new tracks; use **Refresh metadata** to populate or retry lyrics for existing tracks.

The current line appears automatically below the player controls and follows song changes; opening **Lyrics** expands it. The inline lyric remains a single expansion target, without a close button beside the changing text. Only actual timed lyric text appears inline; blank instrumental gaps and songs without timed lyrics do not show placeholder text. The lyrics icon can still open plain lyrics or lookup status in the side panel. The panel starts at the current lyric and follows it; scrolling manually pauses following, and **Jump to current lyrics** resumes it. Plain lyrics and instrumental recordings are also supported. Missing lyrics and provider failures do not interrupt playback or fail the primary metadata refresh.

Choose **Start radio** on a track, album, or artist to build a mix of available songs in your accessible libraries. [ListenBrainz](https://labs.api.listenbrainz.org/) provides similar recordings and artists using matched MusicBrainz IDs. Recommendations are matched back to local files; Koko does not stream missing songs from an external service. Local artist and inherited genre matches fill gaps when recommendations are unavailable, and the player states when it uses this fallback. Equal matches are randomized, and artists and albums are interleaved where the catalog allows it. Radio mixes contain up to 200 songs and can be edited, shuffled, repeated, or saved as playlists.

Radio creates a private temporary queue named after its seed, such as **Seven Nation Army radio**. Its artwork card in the shared **Continue listening** row resumes its saved order, current entry, and song position after stopping or reloading. Songs still retain their own listening progress, but listening through radio does not create separate album Continue listening entries. A finished mix disappears from Continue listening. Saving the mix promotes the same queue to a normal playlist instead of retaining a duplicate temporary mix. Existing listening history from before radio tracking was introduced is left intact.

The active album or radio card has a small playback indicator centered on its cover: animated level bars while playing, or a pause icon while paused. Metadata warnings, refresh indicators, and the progress ring retain their corner positions. Reduced-motion preferences keep the level bars still. Selecting an already active radio opens its queue detail page while preserving its current song, position, shuffle order, and paused state. Selecting an album opens its details without restarting playback.

Radio and playlist detail pages show songs in a vertical list with small album covers, listening progress, and individual playback and queue actions. Their main button pauses or resumes the active queue, or continues its saved song and position when inactive. The list follows the actual playing order after shuffle or queue edits, and repeated occurrences of a song remain distinct.

Radio artwork stays tied to its original source: an artist radio uses the artist's portrait, while an album or song radio uses the album cover. Skipping, shuffling, and editing the queue do not replace it. Refreshing the source artwork updates the radio's image too. Only an item reference is stored, and library access still applies. Older radios created before this reference was recorded receive a stable first-album fallback; recreate them to choose an exact artist or album source.

Recording matching also compares audio duration to distinguish studio versions from live recordings or remixes, within two seconds. Equally plausible candidates remain unmatched. Refresh an existing unmatched track explicitly to retry with this matching improvement.

Album and track heroes inherit missing logos and backgrounds through the artist hierarchy while retaining their own title and cover. This same fallback also applies to seasons and episodes through their show hierarchy.

Refresh existing artists' metadata once after upgrading to collect their MusicBrainz external links. Movies and shows also need stored person identifiers; refresh their metadata if those identifiers have not been collected yet. A normal file scan only processes new items.

### LRCLIB application ID

Official builds may identify Koko with a registered LRCLIB client application ID. Populate the GitHub Actions repository secret **`LRCLIB_APPLICATION_ID`**. Rust and Flatpak builds pass it as **`KOKO_LRCLIB_APPLICATION_ID`** during server compilation, and requests send it in the `Lrclib-App-Id` header. Flatpak uses `secret-env` to keep it out of exported manifests. The ID is not returned in settings responses or included in browser assets. Local development and fork builds work without it, using LRCLIB's identified anonymous access. Local builders who want to include an ID can set the same environment variable before running Cargo. Requests share sequential throttling and honor `Retry-After` with jitter; see [LRCLIB's client identification documentation](https://lrclib.net/docs#implementation-requirements-client-identification).

## Fanart.tv artwork

Select **Fanart.tv** alongside **MusicBrainz** in each music library that should use it. It supplies artist portraits, backgrounds, and logos, and album covers identified by MusicBrainz release-group IDs. Use the arrows to choose provider priority, including moving Fanart.tv above MusicBrainz to prefer its artwork. MusicBrainz continues to identify artists, albums, and tracks. Local artwork takes priority over online providers. Tracks inherit artwork from their album or artist, without a separate Fanart.tv request per track. Refresh an existing library's metadata after enabling the provider to populate artwork.

In **Settings → Providers → Fanart.tv**, users may save a [personal API key](https://fanart.tv/get-an-api-key/). This key is stored using Koko's credential store and sent as `client-key`; it determines the user's Fanart.tv access tier. Builds with a bundled project key work without a personal key. Builds without a project key require a personal key. The bundled project key is never included in settings responses or browser assets.

For official builds, create the GitHub Actions repository secret **`FANART_PROJECT_API_KEY`**. The Rust release and Flatpak workflows pass it as **`KOKO_FANART_PROJECT_API_KEY`** only during server compilation. Flatpak uses `secret-env`, so the value is absent from exported manifests. Local builders can set the same environment variable before running Cargo. Missing secrets are allowed for forks and pull requests. The compiled server contains the key; it is not encrypted inside the binary. See the [Fanart.tv API documentation](https://api.fanart.tv/).

AcoustID/Chromaprint audio fingerprint matching is planned in the [roadmap](./ROADMAP.md). This implementation uses MusicBrainz searches and optional embedded identifiers.
