//! Ordered, private music playlists. Entries are relational rows, including repeated songs.
use super::{
    MediaItemSummary,
    accessible_library_ids,
};
use crate::db::{
    models::MediaItem,
    schema::media_items::dsl as media_items,
};
use diesel::{
    Connection,
    ExpressionMethods,
    OptionalExtension,
    QueryDsl,
    QueryableByName,
    RunQueryDsl,
    SelectableHelper,
    SqliteConnection,
    sql_types::{
        BigInt,
        Bool,
        Integer,
        Nullable,
        Text,
    },
};
use rocket_okapi::okapi::schemars::{
    self,
    JsonSchema,
};
use serde::{
    Deserialize,
    Serialize,
};
use std::collections::{
    HashMap,
    HashSet,
};

/// A saved queue owned by the current user.
#[derive(Debug, Clone, Serialize, JsonSchema, QueryableByName, PartialEq)]
pub struct MusicPlaylist {
    /// Stable playlist ID.
    #[diesel(sql_type = Integer)]
    pub id: i32,
    /// User chosen name.
    #[diesel(sql_type = Text)]
    pub name: String,
    /// Creation timestamp in Unix seconds.
    #[diesel(sql_type = BigInt)]
    pub created_at: i64,
    /// Last modification timestamp in Unix seconds.
    #[diesel(sql_type = BigInt)]
    pub updated_at: i64,
    /// Radio queues stay temporary until explicitly saved as a playlist.
    #[diesel(sql_type = Bool)]
    pub temporary: bool,
    /// Zero-based entry in saved playing order, including repeated songs.
    #[diesel(sql_type = Integer)]
    pub current_index: i32,
    /// Position in the current song.
    #[diesel(sql_type = BigInt)]
    pub position_ms: i64,
    /// Whether the queue reached its end.
    #[diesel(sql_type = Bool)]
    pub completed: bool,
    /// Shuffle preference; the entries already contain the actual playing order.
    #[diesel(sql_type = Bool)]
    pub shuffle: bool,
    /// Repeat preference: off, all, or one.
    #[diesel(sql_type = Text)]
    pub repeat_mode: String,
    /// Artist or album whose current artwork represents the radio's original seed.
    #[diesel(sql_type = Nullable<Integer>)]
    pub artwork_item_id: Option<i32>,
}

/// A live artwork reference, without copying provider images into the playlist.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct PlaylistArtwork {
    /// Stable item whose poster should be displayed.
    pub item_id: i32,
    /// Library containing the source artwork.
    pub library_id: i32,
    /// Artwork revision used to refresh the browser cache after metadata changes.
    pub updated_at: Option<i64>,
}

/// Saved playlist plus only the songs the caller can still access.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct MusicPlaylistDetail {
    /// Playlist properties.
    pub playlist: MusicPlaylist,
    /// Songs in saved order; duplicates are intentional.
    pub items: Vec<MediaItemSummary>,
    /// Available artwork from the original radio source, subject to library access.
    pub artwork: Option<PlaylistArtwork>,
}

#[derive(QueryableByName)]
struct ItemId {
    #[diesel(sql_type = Integer)]
    id: i32,
}

/// Actual queue order and progress, stored as scalar fields and relational entries.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueueState {
    /// Song IDs in actual playing order.
    pub item_ids: Vec<i32>,
    /// Current entry's index, preserving duplicates.
    pub current_index: i32,
    /// Position within that entry.
    pub position_ms: i64,
    /// True after the last entry finishes without repeat.
    pub completed: bool,
    /// Shuffle preference.
    pub shuffle: bool,
    /// off, all, or one.
    pub repeat_mode: String,
}

/// Read an owned playlist without leaking songs from libraries with revoked access.
pub fn load(
    conn: &mut SqliteConnection,
    user: i32,
    id: i32,
) -> diesel::QueryResult<MusicPlaylistDetail> {
    let mut playlist: MusicPlaylist =
        diesel::sql_query("SELECT * FROM music_playlists WHERE id=? AND user_id=?")
            .bind::<Integer, _>(id)
            .bind::<Integer, _>(user)
            .get_result(conn)?;
    let ids = diesel::sql_query(
        "SELECT media_item_id AS id FROM music_playlist_entries WHERE playlist_id=? ORDER BY \
         position",
    )
    .bind::<Integer, _>(id)
    .load::<ItemId>(conn)?;
    let ordered_ids = ids.iter().map(|entry| entry.id).collect::<Vec<_>>();
    let rows = visible_rows(conn, user, &ordered_ids)?;
    let links = super::preferred_metadata_links_by_item_id(conn, &ordered_ids, &[])?;
    let mut summaries = rows
        .into_iter()
        .map(|row| {
            let mut summary = super::to_media_item_summary(row);
            let id = summary.id;
            super::apply_primary_metadata_link(&mut summary, links.get(&id));
            summary
        })
        .collect::<Vec<_>>();
    super::music::apply_summaries(conn, &mut summaries)?;
    let by_id = summaries
        .into_iter()
        .map(|item| (item.id, item))
        .collect::<HashMap<_, _>>();
    let original_index = playlist.current_index.max(0) as usize;
    let visible_before = ordered_ids
        .iter()
        .take(original_index)
        .filter(|id| by_id.contains_key(id))
        .count();
    if ordered_ids
        .get(original_index)
        .is_none_or(|id| !by_id.contains_key(id))
    {
        playlist.position_ms = 0;
    }
    let items: Vec<_> = ordered_ids
        .into_iter()
        .filter_map(|id| by_id.get(&id).cloned())
        .collect();
    playlist.current_index = visible_before.min(items.len().saturating_sub(1)) as i32;
    let artwork = if let Some(id) = playlist.artwork_item_id {
        match super::get_media_item_summary(conn, id)? {
            Some(source)
                if super::user_can_access_library(conn, source.library_id, Some(user))? =>
            {
                Some(PlaylistArtwork {
                    item_id: source.id,
                    library_id: source.library_id,
                    updated_at: source.artwork_updated_at,
                })
            }
            _ => None,
        }
    } else {
        None
    };
    if artwork.is_none() {
        playlist.artwork_item_id = None;
    }
    Ok(MusicPlaylistDetail {
        playlist,
        items,
        artwork,
    })
}

/// List private playlists, optionally scoped to songs in one accessible library.
pub fn list(
    conn: &mut SqliteConnection,
    user: i32,
    library: Option<i32>,
) -> diesel::QueryResult<Vec<MusicPlaylistDetail>> {
    let ids = diesel::sql_query(
        "SELECT id FROM music_playlists WHERE user_id=? AND temporary=0 ORDER BY updated_at \
         DESC,id DESC",
    )
    .bind::<Integer, _>(user)
    .load::<ItemId>(conn)?;
    let mut playlists = Vec::new();
    for entry in ids {
        let mut detail = load(conn, user, entry.id)?;
        if let Some(library) = library {
            detail.items.retain(|item| item.library_id == library);
        }
        if library.is_none() || !detail.items.is_empty() {
            playlists.push(detail);
        }
    }
    Ok(playlists)
}

/// Ongoing temporary radios shown as one Continue listening entry.
pub fn ongoing(
    conn: &mut SqliteConnection,
    user: i32,
    library: Option<i32>,
) -> diesel::QueryResult<Vec<MusicPlaylistDetail>> {
    let ids = diesel::sql_query(
        "SELECT id FROM music_playlists WHERE user_id=? AND temporary=1 AND completed=0 ORDER BY \
         updated_at DESC,id DESC",
    )
    .bind::<Integer, _>(user)
    .load::<ItemId>(conn)?;
    let mut result = Vec::new();
    for id in ids {
        let detail = load(conn, user, id.id)?;
        // Keep the full mix order even when discovered from one library's home page.
        if !detail.items.is_empty()
            && library
                .is_none_or(|library| detail.items.iter().any(|item| item.library_id == library))
        {
            result.push(detail);
        }
    }
    Ok(result)
}

/// Create a radio queue using the same relational entries as saved playlists.
pub fn create_radio(
    conn: &mut SqliteConnection,
    user: i32,
    name: &str,
    ids: &[i32],
    seed_item_id: Option<i32>,
) -> diesel::QueryResult<MusicPlaylistDetail> {
    conn.transaction(|conn| {
        let source = radio_artwork_source(
            conn,
            user,
            seed_item_id
                .or_else(|| ids.first().copied())
                .ok_or(diesel::result::Error::NotFound)?,
        )?;
        diesel::sql_query(
            "DELETE FROM music_playlists WHERE user_id=? AND temporary=1 AND completed=1",
        )
        .bind::<Integer, _>(user)
        .execute(conn)?;
        let detail = create(conn, user, name, ids)?;
        diesel::sql_query("UPDATE music_playlists SET temporary=1,artwork_item_id=? WHERE id=?")
            .bind::<Integer, _>(source)
            .bind::<Integer, _>(detail.playlist.id)
            .execute(conn)?;
        load(conn, user, detail.playlist.id)
    })
}

fn radio_artwork_source(
    conn: &mut SqliteConnection,
    user: i32,
    id: i32,
) -> diesel::QueryResult<i32> {
    let source = media_items::media_items
        .find(id)
        .filter(media_items::deleted_at.is_null())
        .select(MediaItem::as_select())
        .first::<MediaItem>(conn)?;
    if !super::user_can_access_library(conn, source.library_id, Some(user))?
        || !matches!(source.item_type.as_str(), "artist" | "album" | "track")
    {
        return Err(diesel::result::Error::NotFound);
    }
    if source.item_type == "track" {
        if let Some(parent) = source.parent_id {
            let album = media_items::media_items
                .find(parent)
                .filter(media_items::library_id.eq(source.library_id))
                .filter(media_items::item_type.eq("album"))
                .filter(media_items::deleted_at.is_null())
                .select(media_items::id)
                .first::<i32>(conn)
                .optional()?;
            if let Some(album) = album {
                return Ok(album);
            }
        }
    }
    Ok(source.id)
}

/// Update queue order and cursor atomically; duplicate songs remain separate entries.
pub fn update_queue(
    conn: &mut SqliteConnection,
    user: i32,
    id: i32,
    queue: &QueueState,
) -> diesel::QueryResult<()> {
    conn.transaction(|conn| {
        diesel::sql_query("SELECT id FROM music_playlists WHERE id=? AND user_id=?")
            .bind::<Integer, _>(id)
            .bind::<Integer, _>(user)
            .get_result::<ItemId>(conn)?;
        let visible = visible_rows(conn, user, &queue.item_ids)?
            .into_iter()
            .map(|item| item.id)
            .collect::<HashSet<_>>();
        if queue.item_ids.iter().any(|id| !visible.contains(id)) {
            return Err(diesel::result::Error::NotFound);
        }
        let saved = diesel::sql_query(
            "SELECT media_item_id AS id FROM music_playlist_entries WHERE playlist_id=? ORDER BY \
             position",
        )
        .bind::<Integer, _>(id)
        .load::<ItemId>(conn)?;
        // Progress ticks only update scalars; rewrite entries only after an actual queue edit.
        if !saved
            .iter()
            .map(|item| item.id)
            .eq(queue.item_ids.iter().copied())
        {
            diesel::sql_query("DELETE FROM music_playlist_entries WHERE playlist_id=?")
                .bind::<Integer, _>(id)
                .execute(conn)?;
            for (position, item) in queue.item_ids.iter().enumerate() {
                diesel::sql_query(
                    "INSERT INTO music_playlist_entries(playlist_id,position,media_item_id) \
                     VALUES(?,?,?)",
                )
                .bind::<Integer, _>(id)
                .bind::<Integer, _>(position as i32)
                .bind::<Integer, _>(*item)
                .execute(conn)?;
            }
        }
        diesel::sql_query(
            "UPDATE music_playlists SET \
             current_index=?,position_ms=?,completed=?,shuffle=?,repeat_mode=?,\
             updated_at=unixepoch() WHERE id=? AND user_id=?",
        )
        .bind::<Integer, _>(queue.current_index)
        .bind::<BigInt, _>(queue.position_ms)
        .bind::<Bool, _>(queue.completed)
        .bind::<Bool, _>(queue.shuffle)
        .bind::<Text, _>(&queue.repeat_mode)
        .bind::<Integer, _>(id)
        .bind::<Integer, _>(user)
        .execute(conn)?;
        Ok(())
    })
}

/// Saving a radio promotes its existing row rather than leaving a duplicate temporary mix.
pub fn rename_and_save(
    conn: &mut SqliteConnection,
    user: i32,
    id: i32,
    name: &str,
) -> diesel::QueryResult<MusicPlaylistDetail> {
    if diesel::sql_query(
        "UPDATE music_playlists SET name=?,temporary=0,updated_at=unixepoch() WHERE id=? AND \
         user_id=?",
    )
    .bind::<Text, _>(name)
    .bind::<Integer, _>(id)
    .bind::<Integer, _>(user)
    .execute(conn)?
        == 0
    {
        return Err(diesel::result::Error::NotFound);
    }
    load(conn, user, id)
}

/// Remember whether the most recent playback came from a radio, independently of its lifetime.
pub fn set_radio_context(
    conn: &mut SqliteConnection,
    user: i32,
    item: i32,
    radio: bool,
) -> diesel::QueryResult<()> {
    if radio {
        diesel::sql_query(
            "INSERT OR IGNORE INTO music_progress_context(user_id,media_item_id) VALUES(?,?)",
        )
        .bind::<Integer, _>(user)
        .bind::<Integer, _>(item)
        .execute(conn)?;
    } else {
        diesel::sql_query("DELETE FROM music_progress_context WHERE user_id=? AND media_item_id=?")
            .bind::<Integer, _>(user)
            .bind::<Integer, _>(item)
            .execute(conn)?;
    }
    Ok(())
}

/// Tracks heard inside radio do not independently turn into album Continue listening entries.
pub fn radio_context_items(
    conn: &mut SqliteConnection,
    user: i32,
) -> diesel::QueryResult<HashSet<i32>> {
    Ok(
        diesel::sql_query("SELECT media_item_id AS id FROM music_progress_context WHERE user_id=?")
            .bind::<Integer, _>(user)
            .load::<ItemId>(conn)?
            .into_iter()
            .map(|item| item.id)
            .collect(),
    )
}

/// Save a queue atomically, preserving its exact order and duplicate entries.
pub fn create(
    conn: &mut SqliteConnection,
    user: i32,
    name: &str,
    ids: &[i32],
) -> diesel::QueryResult<MusicPlaylistDetail> {
    conn.transaction(|conn| {
        let visible = visible_rows(conn, user, ids)?
            .into_iter()
            .map(|item| item.id)
            .collect::<HashSet<_>>();
        if ids.iter().any(|id| !visible.contains(id)) {
            return Err(diesel::result::Error::NotFound);
        }
        diesel::sql_query("INSERT INTO music_playlists(user_id,name) VALUES (?,?)")
            .bind::<Integer, _>(user)
            .bind::<Text, _>(name)
            .execute(conn)?;
        let id = diesel::sql_query("SELECT CAST(last_insert_rowid() AS INTEGER) AS id")
            .get_result::<ItemId>(conn)?
            .id;
        for (position, item) in ids.iter().enumerate() {
            diesel::sql_query(
                "INSERT INTO music_playlist_entries(playlist_id,position,media_item_id) VALUES \
                 (?,?,?)",
            )
            .bind::<Integer, _>(id)
            .bind::<Integer, _>(position as i32)
            .bind::<Integer, _>(*item)
            .execute(conn)?;
        }
        load(conn, user, id)
    })
}

fn visible_rows(
    conn: &mut SqliteConnection,
    user: i32,
    ids: &[i32],
) -> diesel::QueryResult<Vec<MediaItem>> {
    let libraries = accessible_library_ids(conn, Some(user))?;
    let unique = ids
        .iter()
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for chunk in unique.chunks(500) {
        rows.extend(
            media_items::media_items
                .filter(media_items::id.eq_any(chunk))
                .filter(media_items::library_id.eq_any(&libraries))
                .filter(media_items::item_type.eq("track"))
                .filter(media_items::deleted_at.is_null())
                .select(MediaItem::as_select())
                .load::<MediaItem>(conn)?,
        );
    }
    Ok(rows)
}

/// Delete only a playlist belonging to the current user.
pub fn delete(
    conn: &mut SqliteConnection,
    user: i32,
    id: i32,
) -> diesel::QueryResult<()> {
    if diesel::sql_query("DELETE FROM music_playlists WHERE id=? AND user_id=?")
        .bind::<Integer, _>(id)
        .bind::<Integer, _>(user)
        .execute(conn)?
        == 0
    {
        return Err(diesel::result::Error::NotFound);
    }
    Ok(())
}
