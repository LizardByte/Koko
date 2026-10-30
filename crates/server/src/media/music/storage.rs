//! Scalar music metadata and normalized genre rows. No serialized payloads are persisted.
use super::{
    HashMap,
    MediaItemDetail,
    MediaItemSummary,
    MetadataProviderId,
    MusicMetadata,
    sql_types,
};
use crate::db::schema::{
    music_file_genres as file_genres,
    music_file_metadata as files,
    music_item_genres as item_genres,
    music_item_metadata as items,
};
use diesel::{
    Connection,
    IntoSql,
    prelude::*,
};

#[derive(Queryable)]
struct MusicRow {
    owner_id: i32,
    title: String,
    artist: String,
    album_artist: String,
    album: String,
    track_number: Option<i32>,
    disc_number: Option<i32>,
    release_year: Option<i32>,
    musicbrainz_artist_id: Option<String>,
    musicbrainz_release_id: Option<String>,
    musicbrainz_recording_id: Option<String>,
}
#[derive(Queryable)]
struct GenreRow {
    owner_id: i32,
    source: String,
    genre: String,
}
#[derive(Clone, Copy)]
enum Owner {
    File,
    Item,
}
type StoredMusic = HashMap<i32, (MusicMetadata, String)>;

macro_rules! columns {
    ($table:ident, $id:ident, $title:expr) => {
        (
            $table::$id,
            $title,
            $table::artist,
            $table::album_artist,
            $table::album,
            $table::track_number,
            $table::disc_number,
            $table::release_year,
            $table::musicbrainz_artist_id,
            $table::musicbrainz_release_id,
            $table::musicbrainz_recording_id,
        )
    };
}
macro_rules! values {
    ($table:ident, $music:ident) => {
        (
            $table::artist.eq(&$music.artist),
            $table::album_artist.eq(&$music.album_artist),
            $table::album.eq(&$music.album),
            $table::track_number.eq(&$music.track_number),
            $table::disc_number.eq(&$music.disc_number),
            $table::release_year.eq(&$music.year),
            $table::musicbrainz_artist_id.eq(&$music.musicbrainz_artist_id),
            $table::musicbrainz_release_id.eq(&$music.musicbrainz_release_id),
            $table::musicbrainz_recording_id.eq(&$music.musicbrainz_recording_id),
        )
    };
}
fn load(
    conn: &mut SqliteConnection,
    owner: Owner,
    ids: &[i32],
) -> QueryResult<StoredMusic> {
    let mut music = HashMap::new();
    for chunk in ids.chunks(500) {
        let rows = match owner {
            Owner::File => files::table
                .filter(files::media_file_id.eq_any(chunk))
                .select(columns!(files, media_file_id, files::title))
                .load::<MusicRow>(conn)?,
            Owner::Item => items::table
                .filter(items::media_item_id.eq_any(chunk))
                .select(columns!(
                    items,
                    media_item_id,
                    "".into_sql::<sql_types::Text>()
                ))
                .load::<MusicRow>(conn)?,
        };
        music.extend(rows.into_iter().map(|row| {
            (
                row.owner_id,
                (
                    MusicMetadata {
                        artist: row.artist,
                        album_artist: row.album_artist,
                        album: row.album,
                        track_number: row.track_number,
                        disc_number: row.disc_number,
                        year: row.release_year,
                        musicbrainz_artist_id: row.musicbrainz_artist_id,
                        musicbrainz_release_id: row.musicbrainz_release_id,
                        musicbrainz_recording_id: row.musicbrainz_recording_id,
                        genres: Vec::new(),
                    },
                    row.title,
                ),
            )
        }));
    }
    for row in load_genres(conn, owner, ids)?
        .into_iter()
        .filter(|row| row.source == "local")
    {
        if let Some((music, _)) = music.get_mut(&row.owner_id) {
            music.genres.push(row.genre);
        }
    }
    Ok(music)
}
fn load_genres(
    conn: &mut SqliteConnection,
    owner: Owner,
    ids: &[i32],
) -> QueryResult<Vec<GenreRow>> {
    let mut rows = Vec::new();
    for chunk in ids.chunks(500) {
        rows.extend(match owner {
            Owner::File => file_genres::table
                .filter(file_genres::media_file_id.eq_any(chunk))
                .order(file_genres::genre)
                .select((
                    file_genres::media_file_id,
                    "local".into_sql::<sql_types::Text>(),
                    file_genres::genre,
                ))
                .load::<GenreRow>(conn)?,
            Owner::Item => item_genres::table
                .filter(item_genres::media_item_id.eq_any(chunk))
                .order(item_genres::genre)
                .select((
                    item_genres::media_item_id,
                    item_genres::source,
                    item_genres::genre,
                ))
                .load::<GenreRow>(conn)?,
        });
    }
    Ok(rows)
}
pub(in crate::media) fn load_files(
    conn: &mut SqliteConnection,
    ids: &[i32],
) -> QueryResult<StoredMusic> {
    load(conn, Owner::File, ids)
}
pub(in crate::media) fn load_items(
    conn: &mut SqliteConnection,
    ids: &[i32],
) -> QueryResult<StoredMusic> {
    load(conn, Owner::Item, ids)
}

fn save(
    conn: &mut SqliteConnection,
    owner: Owner,
    id: i32,
    title: &str,
    file_hash: &str,
    music: &MusicMetadata,
) -> QueryResult<()> {
    conn.transaction(|conn| {
        match owner {
            Owner::File => {
                diesel::insert_into(files::table)
                    .values((
                        files::media_file_id.eq(id),
                        files::title.eq(title),
                        files::file_hash.eq(file_hash),
                        values!(files, music),
                    ))
                    .on_conflict(files::media_file_id)
                    .do_update()
                    .set((
                        files::title.eq(title),
                        files::file_hash.eq(file_hash),
                        values!(files, music),
                    ))
                    .execute(conn)?;
            }
            Owner::Item => {
                diesel::insert_into(items::table)
                    .values((items::media_item_id.eq(id), values!(items, music)))
                    .on_conflict(items::media_item_id)
                    .do_update()
                    .set(values!(items, music))
                    .execute(conn)?;
            }
        }
        save_genres(conn, owner, id, "local", &music.genres)
    })
}
fn save_genres(
    conn: &mut SqliteConnection,
    owner: Owner,
    id: i32,
    source: &str,
    genres: &[String],
) -> QueryResult<()> {
    match owner {
        Owner::File => {
            diesel::delete(file_genres::table.filter(file_genres::media_file_id.eq(id)))
                .execute(conn)?;
        }
        Owner::Item => {
            diesel::delete(
                item_genres::table
                    .filter(item_genres::media_item_id.eq(id))
                    .filter(item_genres::source.eq(source)),
            )
            .execute(conn)?;
        }
    }
    for genre in genres
        .iter()
        .map(|genre| genre.trim())
        .filter(|genre| !genre.is_empty())
    {
        match owner {
            Owner::File => {
                diesel::insert_or_ignore_into(file_genres::table)
                    .values((
                        file_genres::media_file_id.eq(id),
                        file_genres::genre.eq(genre),
                    ))
                    .execute(conn)?;
            }
            Owner::Item => {
                diesel::insert_or_ignore_into(item_genres::table)
                    .values((
                        item_genres::media_item_id.eq(id),
                        item_genres::source.eq(source),
                        item_genres::genre.eq(genre),
                    ))
                    .execute(conn)?;
            }
        }
    }
    Ok(())
}
pub(in crate::media) fn save_file(
    conn: &mut SqliteConnection,
    id: i32,
    title: &str,
    file_hash: &str,
    music: &MusicMetadata,
) -> Result<(), diesel::result::Error> {
    save(conn, Owner::File, id, title, file_hash, music)
}
pub(in crate::media) fn save_item(
    conn: &mut SqliteConnection,
    id: i32,
    music: &MusicMetadata,
) -> Result<(), diesel::result::Error> {
    save(conn, Owner::Item, id, "", "", music)
}
pub(crate) fn save_provider_genres(
    conn: &mut SqliteConnection,
    id: i32,
    genres: &[String],
) -> Result<(), diesel::result::Error> {
    save_genres(conn, Owner::Item, id, "musicbrainz", genres)
}

pub(in crate::media) fn apply_summaries(
    conn: &mut SqliteConnection,
    items: &mut [MediaItemSummary],
) -> Result<(), diesel::result::Error> {
    let ids = items
        .iter()
        .filter(|item| matches!(item.item_type.as_str(), "artist" | "album" | "track"))
        .map(|item| item.id)
        .collect::<Vec<_>>();
    if ids.is_empty() {
        return Ok(());
    }
    apply_artwork_summaries(conn, items)?;
    let mut music = load_items(conn, &ids)?;
    let genres = load_genres(conn, Owner::Item, &ids)?;
    let mut provider_genres = HashMap::<i32, Vec<String>>::new();
    for row in genres.into_iter().filter(|row| row.source == "musicbrainz") {
        provider_genres
            .entry(row.owner_id)
            .or_default()
            .push(row.genre);
    }
    for item in items {
        if let Some((music, _)) = music.remove(&item.id) {
            if item.genres.is_empty() {
                item.genres = if item.has_metadata {
                    provider_genres
                        .remove(&item.id)
                        .unwrap_or_else(|| music.genres.clone())
                } else {
                    music.genres.clone()
                };
            }
            if item.release_year.is_none() {
                item.release_year = music.year;
            }
            item.display_subtitle = Some(music.artist.clone());
            item.music = Some(Box::new(music));
        }
    }
    Ok(())
}

fn apply_artwork_summaries(
    conn: &mut SqliteConnection,
    items: &mut [MediaItemSummary],
) -> Result<(), diesel::result::Error> {
    use crate::db::schema::item_metadata_links::dsl as links;
    // Only scalar artwork columns are needed for cards; never load provider payloads here.
    let mut enabled_libraries = HashMap::new();
    for item in items
        .iter()
        .filter(|item| matches!(item.item_type.as_str(), "artist" | "album"))
    {
        if let std::collections::hash_map::Entry::Vacant(entry) =
            enabled_libraries.entry(item.library_id)
        {
            entry.insert(
                super::super::media_library_metadata_provider_order(conn, item.library_id)?
                    .contains(&MetadataProviderId::Fanart),
            );
        }
    }
    let ids = items
        .iter()
        .filter(|item| enabled_libraries.get(&item.library_id) == Some(&true))
        .map(|item| item.id)
        .collect::<Vec<_>>();
    let mut artwork = HashMap::new();
    for chunk in ids.chunks(500) {
        let rows = links::item_metadata_links
            .filter(links::media_item_id.eq_any(chunk))
            .filter(links::provider_id.eq("fanart"))
            .filter(links::relation_kind.eq("secondary"))
            .filter(links::match_state.ne("unlinked"))
            .select((
                links::media_item_id,
                links::backdrop_url,
                links::logo_url,
                links::updated_at,
            ))
            .load::<(i32, Option<String>, Option<String>, Option<i64>)>(conn)?;
        for (id, backdrop, logo, updated_at) in rows {
            artwork.insert(id, (backdrop.is_some(), logo.is_some(), updated_at));
        }
    }
    for item in items {
        if !enabled_libraries
            .get(&item.library_id)
            .copied()
            .unwrap_or(false)
        {
            continue;
        }
        if let Some((backdrop, logo, updated_at)) = artwork.get(&item.id) {
            if *backdrop && item.backdrop_url.is_none() {
                item.backdrop_url =
                    Some(format!("/api/v1/items/{}/artwork?kind=backdrop", item.id));
            }
            if *logo && item.logo_url.is_none() {
                item.logo_url = Some(format!("/api/v1/items/{}/artwork?kind=logo", item.id));
            }
            item.artwork_updated_at = item.artwork_updated_at.max(*updated_at);
        }
    }
    Ok(())
}

pub(in crate::media) fn apply_detail(
    conn: &mut SqliteConnection,
    detail: &mut MediaItemDetail,
) -> Result<(), diesel::result::Error> {
    if !matches!(detail.item_type.as_str(), "artist" | "album" | "track") {
        return Ok(());
    }
    let Some((music, _)) = load_items(conn, &[detail.id])?.remove(&detail.id) else {
        return Ok(());
    };
    if detail.genres.is_empty() {
        let genres = if detail.has_metadata {
            load_genres(conn, Owner::Item, &[detail.id])?
                .into_iter()
                .filter(|row| row.source == "musicbrainz")
                .map(|row| row.genre)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        detail.genres = if genres.is_empty() { music.genres.clone() } else { genres };
    }
    if detail.release_year.is_none() {
        detail.release_year = music.year;
    }
    detail.music = Some(Box::new(music));
    Ok(())
}

pub(in crate::media) fn has_file(
    conn: &mut SqliteConnection,
    id: i32,
    file_hash: &str,
) -> Result<bool, diesel::result::Error> {
    use crate::db::schema::music_file_metadata::dsl as tags;
    Ok(tags::music_file_metadata
        .find(id)
        .filter(tags::file_hash.eq(file_hash))
        .select(tags::media_file_id)
        .first::<i32>(conn)
        .optional()?
        .is_some())
}

pub(crate) fn provider_genres(
    conn: &mut SqliteConnection,
    id: i32,
) -> Result<Vec<String>, diesel::result::Error> {
    Ok(load_genres(conn, Owner::Item, &[id])?
        .into_iter()
        .filter(|row| row.source == "musicbrainz")
        .map(|row| row.genre)
        .collect())
}
