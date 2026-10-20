//! Reconnect relocated library files without tying catalog identity to absolute paths.

use super::*;
use crate::db::schema::{
    audiobook_metadata,
    audiobook_provider_metadata,
    audiobook_recording_selections,
    item_metadata_links,
    media_file_libraries as memberships,
    media_files,
    media_items,
    metadata_collection_items,
    music_item_genres,
    music_lyrics,
    music_playlist_entries,
    music_playlists,
    music_progress_context,
    playback_progress,
};
use diesel::QueryResult;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Fingerprint {
    relative_path: Option<String>,
    hash: String,
    size: i64,
    kind: String,
}

fn fingerprint(
    file: &CatalogMediaFile,
    with_path: bool,
) -> Option<Fingerprint> {
    Some(Fingerprint {
        relative_path: with_path.then(|| file.relative_path.clone()),
        hash: reusable_scanner_hash(&file.file_hash)?,
        size: file.file_size,
        kind: file.media_kind.clone(),
    })
}

/// Only removed roots are relocation sources. Existing configured copies stay distinct.
pub(super) fn reconcile(
    conn: &mut SqliteConnection,
    library: &MediaLibrary,
    inspection: &crate::scanner::LibraryInspection,
    now: i64,
) -> QueryResult<()> {
    let configured = inspection.summary.paths.iter().collect::<HashSet<_>>();
    collapse_overlapping_roots(
        conn,
        library.id,
        &configured,
        &inspection.scanned_root_paths,
    )?;
    let files = load_catalog_files_for_library(conn, library.id, true)?;
    let removed = files
        .iter()
        .filter(|file| !configured.contains(&file.source_root_path))
        .map(|file| file.source_root_path.clone())
        .collect::<HashSet<_>>();
    if removed.is_empty() {
        return Ok(());
    }
    let mut matched = HashSet::new();
    let mut pairs = Vec::new();
    // Prefer the unchanged relative path; then allow unique renames/reparenting as well.
    for with_path in [true, false] {
        let mut groups =
            HashMap::<Fingerprint, (Vec<&CatalogMediaFile>, Vec<&CatalogMediaFile>)>::new();
        for file in &files {
            if matched.contains(&file.library_file_id) {
                continue;
            }
            let Some(key) = fingerprint(file, with_path) else {
                continue;
            };
            if removed.contains(&file.source_root_path) {
                groups.entry(key).or_default().0.push(file);
            } else if file.deleted_at.is_none()
                && file.missing_since.is_none()
                && inspection
                    .scanned_root_paths
                    .contains(&file.source_root_path)
            {
                groups.entry(key).or_default().1.push(file);
            }
        }
        for (old, current) in groups.into_values() {
            if let ([old], [current]) = (old.as_slice(), current.as_slice()) {
                matched.insert(old.library_file_id);
                matched.insert(current.library_file_id);
                pairs.push((*old, *current));
            }
        }
    }
    // Merge corresponding containers before replacing memberships, while both trees exist.
    let items = media_items::table
        .filter(media_items::library_id.eq(library.id))
        .select(MediaItem::as_select())
        .load::<MediaItem>(conn)?;
    merge_container_states(conn, &pairs, &items)?;
    for (old, current) in &pairs {
        reconnect_file(conn, old, current)?;
    }
    let complete = inspection.summary.error.is_none()
        && inspection
            .summary
            .paths
            .iter()
            .all(|root| inspection.scanned_root_paths.contains(root));
    // Unavailable replacements retain the old missing tree and its history until a good scan.
    diesel::update(
        memberships::table
            .filter(memberships::library_id.eq(library.id))
            .filter(memberships::source_root_path.eq_any(&removed))
            .filter(memberships::deleted_at.is_null())
            .filter(memberships::missing_since.is_null()),
    )
    .set(memberships::missing_since.eq(Some(now)))
    .execute(conn)?;
    if complete {
        diesel::update(
            memberships::table
                .filter(memberships::library_id.eq(library.id))
                .filter(memberships::source_root_path.eq_any(&removed))
                .filter(memberships::deleted_at.is_null()),
        )
        .set(memberships::deleted_at.eq(Some(now)))
        .execute(conn)?;
    }
    log::info!(
        "Reconnected {} relocated file(s) in library {} across {} removed root(s)",
        pairs.len(),
        library.id,
        removed.len()
    );
    Ok(())
}

fn collapse_overlapping_roots(
    conn: &mut SqliteConnection,
    library: i32,
    configured: &HashSet<&String>,
    scanned: &HashSet<String>,
) -> QueryResult<()> {
    let roots = configured
        .iter()
        .filter_map(|root| Path::new(root).canonicalize().ok())
        .collect::<Vec<_>>();
    let overlaps = roots.iter().enumerate().any(|(index, root)| {
        roots
            .iter()
            .skip(index + 1)
            .any(|other| root.starts_with(other) || other.starts_with(root))
    });
    if !overlaps {
        return Ok(());
    }
    let files = load_catalog_files_for_library(conn, library, false)?;
    let mut physical = HashMap::<PathBuf, Vec<&CatalogMediaFile>>::new();
    for file in &files {
        if file.missing_since.is_none()
            && configured.contains(&file.source_root_path)
            && scanned.contains(&file.source_root_path)
        {
            if let Ok(path) = Path::new(&file.path).canonicalize() {
                physical.entry(path).or_default().push(file);
            }
        }
    }
    let mut pairs = Vec::new();
    for mut copies in physical.into_values().filter(|copies| copies.len() > 1) {
        copies.sort_by_key(|file| file.library_file_id);
        for duplicate in copies.iter().skip(1) {
            pairs.push((*duplicate, copies[0]));
        }
    }
    if pairs.is_empty() {
        return Ok(());
    }
    let items = media_items::table
        .filter(media_items::library_id.eq(library))
        .select(MediaItem::as_select())
        .load::<MediaItem>(conn)?;
    merge_container_states(conn, &pairs, &items)?;
    for (duplicate, keep) in pairs {
        reconnect_file(conn, duplicate, keep)?;
    }
    Ok(())
}

/// Count the reconciled inventory rather than counting overlapping paths twice.
pub(super) fn inventory_counts(
    conn: &mut SqliteConnection,
    library: i32,
) -> QueryResult<HashMap<String, i64>> {
    memberships::table
        .inner_join(media_files::table)
        .filter(memberships::library_id.eq(library))
        .filter(memberships::deleted_at.is_null())
        .filter(memberships::missing_since.is_null())
        .group_by(media_files::media_kind)
        .select((media_files::media_kind, diesel::dsl::count_star()))
        .load::<(String, i64)>(conn)
        .map(|rows| rows.into_iter().collect())
}

fn reconnect_file(
    conn: &mut SqliteConnection,
    old: &CatalogMediaFile,
    current: &CatalogMediaFile,
) -> QueryResult<()> {
    let (keep, duplicate) =
        if old.library_file_id < current.library_file_id { (old, current) } else { (current, old) };
    let item_id = match (keep.media_item_id, duplicate.media_item_id) {
        (Some(keep_id), Some(duplicate_id)) if keep_id != duplicate_id => {
            merge_item_state(conn, keep_id, duplicate_id)?;
            Some(keep_id)
        }
        (keep_id, duplicate_id) => keep_id.or(duplicate_id),
    };
    // Memberships are library-specific; physical file rows may still serve another library.
    // Keep the row at the new physical path and copy reusable probe/tag data into it.
    copy_file_metadata(conn, old, current)?;
    diesel::delete(memberships::table.find(duplicate.library_file_id)).execute(conn)?;
    diesel::update(memberships::table.find(keep.library_file_id))
        .set((
            memberships::media_file_id.eq(current.id),
            memberships::source_root_path.eq(&current.source_root_path),
            memberships::relative_path.eq(&current.relative_path),
            memberships::display_title.eq(&current.display_title),
            memberships::metadata_match_attempted_at.eq(keep.metadata_match_attempted_at),
            memberships::media_item_id.eq(item_id),
            memberships::missing_since.eq(None::<i64>),
            memberships::deleted_at.eq(None::<i64>),
        ))
        .execute(conn)?;
    Ok(())
}

fn copy_file_metadata(
    conn: &mut SqliteConnection,
    old: &CatalogMediaFile,
    current: &CatalogMediaFile,
) -> QueryResult<()> {
    if old.id == current.id {
        return Ok(());
    }
    diesel::update(media_files::table.find(current.id))
        .set((
            media_files::container.eq(current.container.as_ref().or(old.container.as_ref())),
            media_files::duration_ms.eq(current.duration_ms.or(old.duration_ms)),
            media_files::bit_rate.eq(current.bit_rate.or(old.bit_rate)),
            media_files::width.eq(current.width.or(old.width)),
            media_files::height.eq(current.height.or(old.height)),
            media_files::video_codec.eq(current.video_codec.as_ref().or(old.video_codec.as_ref())),
            media_files::audio_codec.eq(current.audio_codec.as_ref().or(old.audio_codec.as_ref())),
            media_files::metadata_json.eq(current
                .metadata_json
                .as_ref()
                .or(old.metadata_json.as_ref())),
            media_files::metadata_updated_at
                .eq(current.metadata_updated_at.or(old.metadata_updated_at)),
        ))
        .execute(conn)?;
    if music::has_file(conn, old.id, &old.file_hash)? {
        if let Some((metadata, title)) = music::load_files(conn, &[old.id])?.remove(&old.id) {
            music::save_file(conn, current.id, &title, &current.file_hash, &metadata)?;
        }
    }
    Ok(())
}

fn merge_container_states(
    conn: &mut SqliteConnection,
    pairs: &[(&CatalogMediaFile, &CatalogMediaFile)],
    items: &[MediaItem],
) -> QueryResult<()> {
    let by_id = items
        .iter()
        .map(|item| (item.id, item))
        .collect::<HashMap<_, _>>();
    let mut forward = HashMap::<i32, HashSet<i32>>::new();
    let mut backward = HashMap::<i32, HashSet<i32>>::new();
    for (old, current) in pairs {
        let mut ids = old.media_item_id.zip(current.media_item_id);
        let mut seen = HashSet::new();
        while let Some((old_id, current_id)) = ids {
            if !seen.insert((old_id, current_id)) {
                break;
            }
            let (Some(old), Some(current)) = (by_id.get(&old_id), by_id.get(&current_id)) else {
                break;
            };
            if old.item_type != current.item_type {
                break;
            }
            if old_id != current_id && !old.playable && !current.playable {
                forward.entry(old_id).or_default().insert(current_id);
                backward.entry(current_id).or_default().insert(old_id);
            }
            ids = old.parent_id.zip(current.parent_id);
        }
    }
    let mut merged = HashSet::new();
    for (old, targets) in forward {
        if targets.len() != 1 {
            continue;
        }
        let current = *targets.iter().next().unwrap();
        if backward
            .get(&current)
            .is_none_or(|sources| sources.len() != 1)
        {
            continue;
        }
        let pair = (old.min(current), old.max(current));
        if merged.insert(pair) {
            merge_item_state(conn, pair.0, pair.1)?;
        }
    }
    Ok(())
}

/// Use surviving file-to-item relationships as identity evidence, including their ancestors.
/// A path can change; ambiguous splits/merges never choose a container arbitrarily.
pub(super) fn retain_item_identities(
    planned: &mut PlannedLibraryItems,
    files: &[CatalogMediaFile],
    existing: &[MediaItem],
) {
    let by_id = existing
        .iter()
        .map(|item| (item.id, item))
        .collect::<HashMap<_, _>>();
    let by_key = planned
        .items
        .iter()
        .map(|item| (item.identity_key.clone(), item))
        .collect::<HashMap<_, _>>();
    let mut candidates = HashMap::<String, HashSet<i32>>::new();
    let mut claims = HashMap::<i32, HashSet<String>>::new();
    for file in files {
        let mut pair = planned
            .leaf_identity_by_file_id
            .get(&file.library_file_id)
            .cloned()
            .zip(file.media_item_id);
        let mut seen = HashSet::new();
        while let Some((key, id)) = pair {
            if !seen.insert(id) {
                break;
            }
            let (Some(plan), Some(item)) = (by_key.get(&key), by_id.get(&id)) else {
                break;
            };
            if plan.item_type != item.item_type
                || plan.season_number != item.season_number
                || plan.episode_number != item.episode_number
            {
                break;
            }
            candidates.entry(key.clone()).or_default().insert(id);
            claims.entry(id).or_default().insert(key);
            pair = plan.parent_identity_key.clone().zip(item.parent_id);
        }
    }
    let aliases = candidates
        .into_iter()
        .filter_map(|(key, ids)| {
            if ids.len() != 1 {
                return None;
            }
            let id = *ids.iter().next()?;
            (claims.get(&id)?.len() == 1).then(|| (key, by_id[&id].identity_key.clone()))
        })
        .collect::<HashMap<_, _>>();
    for item in &mut planned.items {
        if let Some(key) = aliases.get(&item.identity_key) {
            item.identity_key = key.clone();
        }
        if let Some(parent) = item.parent_identity_key.as_mut() {
            if let Some(key) = aliases.get(parent) {
                *parent = key.clone();
            }
        }
    }
    for key in planned.leaf_identity_by_file_id.values_mut() {
        if let Some(alias) = aliases.get(key) {
            *key = alias.clone();
        }
    }
}

macro_rules! copy_item_rows {
    ($conn:ident, $keep:ident, $duplicate:ident, $table:ident,
        [$($column:ident: $ty:ty),+ $(,)?]) => {{
        let rows = $table::table.filter($table::media_item_id.eq($duplicate))
            .select(($($table::$column,)+)).load::<($($ty,)+)>($conn)?;
        for ($($column,)+) in rows {
            diesel::insert_into($table::table).values((
                $table::media_item_id.eq($keep), $($table::$column.eq($column),)+
            )).on_conflict_do_nothing().execute($conn)?;
        }
    }};
}

fn merge_item_state(
    conn: &mut SqliteConnection,
    keep: i32,
    duplicate: i32,
) -> QueryResult<()> {
    let Some(kept) = load_media_item_row(conn, keep)? else {
        return Ok(());
    };
    let Some(other) = load_media_item_row(conn, duplicate)? else {
        return Ok(());
    };
    if kept.library_id != other.library_id || kept.item_type != other.item_type {
        return Err(diesel::result::Error::RollbackTransaction);
    }
    merge_progress(conn, keep, duplicate)?;
    books::merge_progress(conn, keep, duplicate)?;
    let recording = preferred_recording(conn, keep, duplicate)?;
    let moved = merge_metadata_links(
        conn,
        keep,
        duplicate,
        recording
            .as_ref()
            .map(|(owner, provider, _)| (*owner, provider.as_str())),
    )?;
    copy_item_rows!(conn, keep, duplicate, audiobook_metadata, [metadata_json: String]);
    merge_recording_metadata(conn, keep, duplicate, &moved)?;
    if let Some((_, provider, external)) = recording {
        diesel::insert_into(audiobook_recording_selections::table)
            .values((
                audiobook_recording_selections::media_item_id.eq(keep),
                audiobook_recording_selections::provider_id.eq(&provider),
                audiobook_recording_selections::external_id.eq(&external),
            ))
            .on_conflict(audiobook_recording_selections::media_item_id)
            .do_update()
            .set((
                audiobook_recording_selections::provider_id.eq(&provider),
                audiobook_recording_selections::external_id.eq(&external),
            ))
            .execute(conn)?;
    }
    copy_item_rows!(conn, keep, duplicate, music_item_genres, [source: String, genre: String]);
    copy_item_rows!(conn, keep, duplicate, music_progress_context, [user_id: i32]);
    copy_item_rows!(conn, keep, duplicate, music_lyrics, [
        track_title: String, artist_name: String, album_name: String,
        duration_ms: Option<i64>, plain_lyrics: Option<String>, synced_lyrics: Option<String>,
        instrumental: bool, source: String, source_url: String, lookup_status: String,
        checked_at: i64, next_check_at: i64, last_error: Option<String>
    ]);
    if !music::load_items(conn, &[keep])?.contains_key(&keep) {
        if let Some((music, _)) = music::load_items(conn, &[duplicate])?.remove(&duplicate) {
            music::save_item(conn, keep, &music)?;
        }
    }
    diesel::update(
        music_playlist_entries::table.filter(music_playlist_entries::media_item_id.eq(duplicate)),
    )
    .set(music_playlist_entries::media_item_id.eq(keep))
    .execute(conn)?;
    diesel::update(music_playlists::table.filter(music_playlists::artwork_item_id.eq(duplicate)))
        .set(music_playlists::artwork_item_id.eq(keep))
        .execute(conn)?;
    diesel::update(media_items::table.filter(media_items::parent_id.eq(duplicate)))
        .set(media_items::parent_id.eq(keep))
        .execute(conn)?;
    diesel::update(
        memberships::table
            .filter(memberships::library_id.eq(kept.library_id))
            .filter(memberships::media_item_id.eq(duplicate)),
    )
    .set(memberships::media_item_id.eq(keep))
    .execute(conn)?;
    // Keep backup rows and their conflicting metadata; only the surviving tree is browsable.
    diesel::update(media_items::table.find(duplicate))
        .set(media_items::deleted_at.eq(Some(current_timestamp())))
        .execute(conn)?;
    Ok(())
}

fn merge_metadata_links(
    conn: &mut SqliteConnection,
    keep: i32,
    duplicate: i32,
    recording: Option<(i32, &str)>,
) -> QueryResult<HashSet<String>> {
    let mut moved = HashSet::new();
    let links = item_metadata_links::table
        .filter(item_metadata_links::media_item_id.eq(duplicate))
        .select(ItemMetadataLink::as_select())
        .load::<ItemMetadataLink>(conn)?;
    for link in links {
        let existing = item_metadata_links::table
            .filter(item_metadata_links::media_item_id.eq(keep))
            .filter(item_metadata_links::provider_id.eq(&link.provider_id))
            .filter(item_metadata_links::locale_key.eq(&link.locale_key))
            .select(ItemMetadataLink::as_select())
            .first::<ItemMetadataLink>(conn)
            .optional()?;
        let replace_existing = existing.as_ref().is_some_and(|existing| {
            if let Some((owner, provider)) =
                recording.filter(|(_, provider)| *provider == link.provider_id)
            {
                return owner == duplicate && provider == link.provider_id;
            }
            link.match_state == "linked"
                && link.title.is_some()
                && existing.match_state != "unlinked"
                && (existing.title.is_none() || link.updated_at > existing.updated_at)
        });
        if replace_existing {
            diesel::delete(item_metadata_links::table.find(existing.as_ref().unwrap().id))
                .execute(conn)?;
        }
        if existing.is_none() || replace_existing {
            diesel::update(item_metadata_links::table.find(link.id))
                .set(item_metadata_links::media_item_id.eq(keep))
                .execute(conn)?;
            moved.insert(link.provider_id.clone());
            let collections = metadata_collection_items::table
                .filter(metadata_collection_items::media_item_id.eq(duplicate))
                .filter(metadata_collection_items::metadata_link_id.eq(link.id))
                .select((
                    metadata_collection_items::collection_id,
                    metadata_collection_items::metadata_link_id,
                    metadata_collection_items::updated_at,
                ))
                .load::<(i32, i32, Option<i64>)>(conn)?;
            for (collection, link, updated) in collections {
                diesel::insert_into(metadata_collection_items::table)
                    .values((
                        metadata_collection_items::media_item_id.eq(keep),
                        metadata_collection_items::collection_id.eq(collection),
                        metadata_collection_items::metadata_link_id.eq(link),
                        metadata_collection_items::updated_at.eq(updated),
                    ))
                    .on_conflict_do_nothing()
                    .execute(conn)?;
            }
        }
    }
    Ok(moved)
}

type RecordingChoice = (i32, String, String);

fn preferred_recording(
    conn: &mut SqliteConnection,
    keep: i32,
    duplicate: i32,
) -> QueryResult<Option<RecordingChoice>> {
    let choices = audiobook_recording_selections::table
        .filter(audiobook_recording_selections::media_item_id.eq_any([keep, duplicate]))
        .select((
            audiobook_recording_selections::media_item_id,
            audiobook_recording_selections::provider_id,
            audiobook_recording_selections::external_id,
        ))
        .load::<RecordingChoice>(conn)?;
    let mut selected = None;
    let mut latest = None;
    for choice in choices {
        let timestamp = item_metadata_links::table
            .filter(item_metadata_links::media_item_id.eq(choice.0))
            .filter(item_metadata_links::provider_id.eq(&choice.1))
            .select(diesel::dsl::max(item_metadata_links::updated_at))
            .first::<Option<i64>>(conn)?;
        if selected.is_none() || timestamp > latest || (timestamp == latest && choice.0 == keep) {
            selected = Some(choice);
            latest = timestamp;
        }
    }
    Ok(selected)
}

fn merge_recording_metadata(
    conn: &mut SqliteConnection,
    keep: i32,
    duplicate: i32,
    moved: &HashSet<String>,
) -> QueryResult<()> {
    let rows = audiobook_provider_metadata::table
        .filter(audiobook_provider_metadata::media_item_id.eq(duplicate))
        .select((
            audiobook_provider_metadata::provider_id,
            audiobook_provider_metadata::external_id,
            audiobook_provider_metadata::metadata_json,
        ))
        .load::<(String, String, String)>(conn)?;
    for (provider, external, raw) in rows {
        // A selected provider snapshot must continue to describe the surviving link's recording.
        let retained = item_metadata_links::table
            .filter(item_metadata_links::media_item_id.eq(keep))
            .filter(item_metadata_links::provider_id.eq(&provider))
            .filter(item_metadata_links::external_id.eq(&external))
            .count()
            .get_result::<i64>(conn)?
            > 0;
        let existing = audiobook_provider_metadata::table
            .filter(audiobook_provider_metadata::media_item_id.eq(keep))
            .filter(audiobook_provider_metadata::provider_id.eq(&provider))
            .select(audiobook_provider_metadata::external_id)
            .first::<String>(conn)
            .optional()?;
        if retained && (existing.is_none() || moved.contains(&provider)) {
            diesel::insert_into(audiobook_provider_metadata::table)
                .values((
                    audiobook_provider_metadata::media_item_id.eq(keep),
                    audiobook_provider_metadata::provider_id.eq(&provider),
                    audiobook_provider_metadata::external_id.eq(&external),
                    audiobook_provider_metadata::metadata_json.eq(&raw),
                ))
                .on_conflict((
                    audiobook_provider_metadata::media_item_id,
                    audiobook_provider_metadata::provider_id,
                ))
                .do_update()
                .set((
                    audiobook_provider_metadata::external_id.eq(&external),
                    audiobook_provider_metadata::metadata_json.eq(&raw),
                ))
                .execute(conn)?;
        }
    }
    Ok(())
}

fn merge_progress(
    conn: &mut SqliteConnection,
    keep: i32,
    duplicate: i32,
) -> QueryResult<()> {
    let mut kept = playback_progress::table
        .filter(playback_progress::media_item_id.eq(keep))
        .select(PlaybackProgress::as_select())
        .load::<PlaybackProgress>(conn)?
        .into_iter()
        .map(|progress| (progress.user_id, progress))
        .collect::<HashMap<_, _>>();
    let rows = playback_progress::table
        .filter(playback_progress::media_item_id.eq(duplicate))
        .order(playback_progress::updated_at.asc())
        .select(PlaybackProgress::as_select())
        .load::<PlaybackProgress>(conn)?;
    for progress in rows {
        if let Some(current) = kept.get_mut(&progress.user_id) {
            let latest =
                if progress.updated_at > current.updated_at { &progress } else { &*current };
            diesel::update(playback_progress::table.find(current.id))
                .set((
                    playback_progress::position_ms.eq(latest.position_ms),
                    playback_progress::duration_ms.eq(latest.duration_ms),
                    playback_progress::completed.eq(latest.completed),
                    playback_progress::watch_count
                        .eq(current.watch_count.max(progress.watch_count)),
                    playback_progress::last_watched_at
                        .eq(current.last_watched_at.max(progress.last_watched_at)),
                    playback_progress::updated_at.eq(current.updated_at.max(progress.updated_at)),
                ))
                .execute(conn)?;
            *current = playback_progress::table
                .find(current.id)
                .select(PlaybackProgress::as_select())
                .first(conn)?;
        } else {
            // Keep the existing row identifier and all timestamps for listeners only on the
            // duplicate.
            diesel::update(playback_progress::table.find(progress.id))
                .set(playback_progress::media_item_id.eq(keep))
                .execute(conn)?;
            kept.insert(progress.user_id, progress);
        }
    }
    Ok(())
}
