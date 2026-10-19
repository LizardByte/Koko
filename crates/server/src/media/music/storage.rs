//! Scalar music metadata and normalized genre rows. No serialized payloads are persisted.

use diesel::Connection;

use super::*;

#[derive(diesel::QueryableByName)]
struct MusicRow {
    #[diesel(sql_type = sql_types::Integer)]
    owner_id: i32,
    #[diesel(sql_type = sql_types::Text)]
    title: String,
    #[diesel(sql_type = sql_types::Text)]
    artist: String,
    #[diesel(sql_type = sql_types::Text)]
    album_artist: String,
    #[diesel(sql_type = sql_types::Text)]
    album: String,
    #[diesel(sql_type = sql_types::Nullable<sql_types::Integer>)]
    track_number: Option<i32>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::Integer>)]
    disc_number: Option<i32>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::Integer>)]
    release_year: Option<i32>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::Text>)]
    musicbrainz_artist_id: Option<String>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::Text>)]
    musicbrainz_release_id: Option<String>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::Text>)]
    musicbrainz_recording_id: Option<String>,
}

#[derive(diesel::QueryableByName)]
struct GenreRow {
    #[diesel(sql_type = sql_types::Integer)]
    owner_id: i32,
    #[diesel(sql_type = sql_types::Text)]
    source: String,
    #[diesel(sql_type = sql_types::Text)]
    genre: String,
}

#[derive(Clone, Copy)]
enum Owner {
    File,
    Item,
}

impl Owner {
    fn kind(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Item => "item",
        }
    }
}

type StoredMusic = HashMap<i32, (MusicMetadata, String)>;

fn load(
    conn: &mut SqliteConnection,
    owner: Owner,
    ids: &[i32],
) -> Result<StoredMusic, diesel::result::Error> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    // IDs are typed integers; table names come only from Owner, never request text.
    let ids = ids.iter().map(i32::to_string).collect::<Vec<_>>().join(",");
    let kind = owner.kind();
    let title = if matches!(owner, Owner::File) { "title" } else { "'' AS title" };
    let sql = format!(
        "SELECT media_{kind}_id AS \
         owner_id,{title},artist,album_artist,album,track_number,disc_number,release_year,\
         musicbrainz_artist_id,musicbrainz_release_id,musicbrainz_recording_id FROM \
         music_{kind}_metadata WHERE media_{kind}_id IN ({ids})"
    );
    let rows = diesel::sql_query(sql).load::<MusicRow>(conn)?;
    let mut music = rows
        .into_iter()
        .map(|row| {
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
        })
        .collect::<StoredMusic>();
    let genres = load_genres(conn, owner, &ids)?;
    for row in genres.into_iter().filter(|row| row.source == "local") {
        if let Some((music, _)) = music.get_mut(&row.owner_id) {
            music.genres.push(row.genre);
        }
    }
    Ok(music)
}

fn load_genres(
    conn: &mut SqliteConnection,
    owner: Owner,
    ids: &str,
) -> Result<Vec<GenreRow>, diesel::result::Error> {
    let kind = owner.kind();
    let source = if matches!(owner, Owner::Item) { "source" } else { "'local' AS source" };
    diesel::sql_query(format!(
        "SELECT media_{kind}_id AS owner_id,{source},genre FROM music_{kind}_genres WHERE \
         media_{kind}_id IN ({ids}) ORDER BY genre"
    ))
    .load(conn)
}

pub(in crate::media) fn load_files(
    conn: &mut SqliteConnection,
    ids: &[i32],
) -> Result<StoredMusic, diesel::result::Error> {
    load(conn, Owner::File, ids)
}
pub(in crate::media) fn load_items(
    conn: &mut SqliteConnection,
    ids: &[i32],
) -> Result<StoredMusic, diesel::result::Error> {
    load(conn, Owner::Item, ids)
}

fn save(
    conn: &mut SqliteConnection,
    owner: Owner,
    id: i32,
    title: &str,
    file_hash: &str,
    music: &MusicMetadata,
) -> Result<(), diesel::result::Error> {
    let kind = owner.kind();
    let title_column = if matches!(owner, Owner::File) { ",title,file_hash" } else { "" };
    let title_value = if matches!(owner, Owner::File) { ",?,?" } else { "" };
    let title_update = if matches!(owner, Owner::File) {
        ",title=excluded.title,file_hash=excluded.file_hash"
    } else {
        ""
    };
    conn.transaction(|conn| {
        let sql = format!(
            "INSERT INTO music_{kind}_metadata \
             (media_{kind}_id,artist,album_artist,album,track_number,disc_number,release_year,\
             musicbrainz_artist_id,musicbrainz_release_id,musicbrainz_recording_id{title_column}) \
             VALUES (?,?,?,?,?,?,?,?,?,?{title_value}) ON CONFLICT(media_{kind}_id) DO UPDATE SET \
             artist=excluded.artist,album_artist=excluded.album_artist,album=excluded.album,\
             track_number=excluded.track_number,disc_number=excluded.disc_number,\
             release_year=excluded.release_year,musicbrainz_artist_id=excluded.\
             musicbrainz_artist_id,musicbrainz_release_id=excluded.musicbrainz_release_id,\
             musicbrainz_recording_id=excluded.musicbrainz_recording_id{title_update}"
        );
        let mut query = diesel::sql_query(sql)
            .into_boxed::<diesel::sqlite::Sqlite>()
            .bind::<sql_types::Integer, _>(id)
            .bind::<sql_types::Text, _>(&music.artist)
            .bind::<sql_types::Text, _>(&music.album_artist)
            .bind::<sql_types::Text, _>(&music.album)
            .bind::<sql_types::Nullable<sql_types::Integer>, _>(music.track_number)
            .bind::<sql_types::Nullable<sql_types::Integer>, _>(music.disc_number)
            .bind::<sql_types::Nullable<sql_types::Integer>, _>(music.year)
            .bind::<sql_types::Nullable<sql_types::Text>, _>(&music.musicbrainz_artist_id)
            .bind::<sql_types::Nullable<sql_types::Text>, _>(&music.musicbrainz_release_id)
            .bind::<sql_types::Nullable<sql_types::Text>, _>(&music.musicbrainz_recording_id);
        if matches!(owner, Owner::File) {
            query = query
                .bind::<sql_types::Text, _>(title)
                .bind::<sql_types::Text, _>(file_hash);
        }
        query.execute(conn)?;
        save_genres(conn, owner, id, "local", &music.genres)
    })
}

fn save_genres(
    conn: &mut SqliteConnection,
    owner: Owner,
    id: i32,
    source: &str,
    genres: &[String],
) -> Result<(), diesel::result::Error> {
    let kind = owner.kind();
    let filter = if matches!(owner, Owner::Item) { " AND source = ?" } else { "" };
    let mut delete = diesel::sql_query(format!(
        "DELETE FROM music_{kind}_genres WHERE media_{kind}_id = ?{filter}"
    ))
    .into_boxed::<diesel::sqlite::Sqlite>()
    .bind::<sql_types::Integer, _>(id);
    if matches!(owner, Owner::Item) {
        delete = delete.bind::<sql_types::Text, _>(source);
    }
    delete.execute(conn)?;
    for genre in genres
        .iter()
        .map(|genre| genre.trim())
        .filter(|genre| !genre.is_empty())
    {
        let columns = if matches!(owner, Owner::Item) { ",source" } else { "" };
        let value = if matches!(owner, Owner::Item) { ",?" } else { "" };
        let mut insert = diesel::sql_query(format!(
            "INSERT OR IGNORE INTO music_{kind}_genres (media_{kind}_id,genre{columns}) VALUES \
             (?,?{value})"
        ))
        .into_boxed::<diesel::sqlite::Sqlite>()
        .bind::<sql_types::Integer, _>(id)
        .bind::<sql_types::Text, _>(genre);
        if matches!(owner, Owner::Item) {
            insert = insert.bind::<sql_types::Text, _>(source);
        }
        insert.execute(conn)?;
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
    let mut music = load_items(conn, &ids)?;
    let genres = load_genres(
        conn,
        Owner::Item,
        &ids.iter().map(i32::to_string).collect::<Vec<_>>().join(","),
    )?;
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
            load_genres(conn, Owner::Item, &detail.id.to_string())?
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
    Ok(load_genres(conn, Owner::Item, &id.to_string())?
        .into_iter()
        .filter(|row| row.source == "musicbrainz")
        .map(|row| row.genre)
        .collect())
}
