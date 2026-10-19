# Music libraries

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

Open **Details and management → Source files** to inspect the server's full file paths, library folders, relative paths, original catalog titles, missing-file status, and available technical details. For artists, albums, series, and other groups, this includes files belonging to their children. Source files load on demand in pages, rather than adding large file lists to normal browsing responses. This panel is shared by all media types.

MusicBrainz requests share a rate limit of at most one request per second, including retries. Large libraries can therefore take time to match. Existing metadata remains available while background work runs. See the [MusicBrainz API documentation](https://musicbrainz.org/doc/MusicBrainz_API) and [Cover Art Archive API](https://musicbrainz.org/doc/Cover_Art_Archive/API).

## Fanart.tv artwork

Select **Fanart.tv** alongside **MusicBrainz** in each music library that should use it. It supplies artist portraits, backgrounds, and logos, and album covers identified by MusicBrainz release-group IDs. Use the arrows to choose provider priority, including moving Fanart.tv above MusicBrainz to prefer its artwork. MusicBrainz continues to identify artists, albums, and tracks. Local artwork takes priority over online providers. Tracks inherit artwork from their album or artist, without a separate Fanart.tv request per track. Refresh an existing library's metadata after enabling the provider to populate artwork.

In **Settings → Providers → Fanart.tv**, users may save a [personal API key](https://fanart.tv/get-an-api-key/). This key is stored using Koko's credential store and sent as `client-key`; it determines the user's Fanart.tv access tier. Builds with a bundled project key work without a personal key. Builds without a project key require a personal key. The bundled project key is never included in settings responses or browser assets.

For official builds, create the GitHub Actions repository secret **`FANART_PROJECT_API_KEY`**. The Rust release and Flatpak workflows pass it as **`KOKO_FANART_PROJECT_API_KEY`** only during server compilation. Flatpak uses `secret-env`, so the value is absent from exported manifests. Local builders can set the same environment variable before running Cargo. Missing secrets are allowed for forks and pull requests. The compiled server contains the key; it is not encrypted inside the binary. See the [Fanart.tv API documentation](https://api.fanart.tv/).

AcoustID/Chromaprint audio fingerprint matching is planned in the [roadmap](./ROADMAP.md). This implementation uses MusicBrainz searches and optional embedded identifiers.
