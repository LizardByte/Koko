# Music naming

Koko groups music into **artist → album → track**. Use this layout when embedded tags are absent or disabled:

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

## Embedded tags

Embedded tags are ignored by default. Enable **Use embedded music tags** in the library settings to use title, artist, album artist, album, track number, disc number, date, and genre tags. Album artist keeps compilations together when tracks credit different performers. Track and disc values can include totals, such as `3/12`.

When embedded tags are enabled, MusicBrainz identifiers can provide precise metadata matching, for example after tagging with MusicBrainz Picard:

- `MUSICBRAINZ_ALBUMARTISTID` (or `MUSICBRAINZ_ARTISTID`): album artist UUID
- `MUSICBRAINZ_ALBUMID`: release UUID, which identifies a particular album edition
- `MUSICBRAINZ_TRACKID`: recording UUID
