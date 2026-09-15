# Known Backend Issues

## Stale DB entries for moved/removed files (duplicate items)

**Symptom:** The same media file appears as two separate items with different IDs — one with metadata linked, one without. The show + season are also duplicated.

**Root cause:** When a file is moved or removed from a library path, the scanner does not clean up the old DB entry. If the same file exists in (or was moved between) two library paths, both entries remain in the DB. Metadata linked to one copy doesn't propagate to the other.

**Example:** `/Volumes/DataBucket/Media/[file].mkv` exists in library 2 (id=541). A stale entry also exists in library 1 (id=25) with the same relative path, even though the file no longer exists at `/Users/hazer/Downloads/Torrents/[file].mkv`.

**Fix needed:** The scanner (`crates/server/src/scanner/`) should detect files that no longer exist at their library path during a scan and either:
1. Mark them as missing (set `missing_since` timestamp), or
2. Remove them entirely if they've been missing for longer than the trash-cleanup threshold

The missing-item detection likely exists (the `missing_since` field + `deleteMissingItems` API) but may not be triggering correctly for files that moved between libraries. The scanner might need to check file existence by full path, not just by hash/identity key.

**Areas to investigate:**
- `crates/server/src/scanner/directory.rs` — the directory scanner that discovers files
- `crates/server/src/media.rs` — `sync_discovered_media_file` + the scan sync logic that decides what's new/missing
- `crates/server/src/db/queries/` — the queries that insert/update/remove media items
