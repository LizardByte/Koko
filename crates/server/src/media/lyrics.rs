//! Durable lyrics and lookup outcomes, stored as scalar fields rather than provider payloads.
use crate::db::schema::music_lyrics;
use diesel::{
    SqliteConnection,
    prelude::*,
};
use rocket_okapi::okapi::schemars::{
    self,
    JsonSchema,
};
use serde::Serialize;

/// Lyrics used by the player, including their provider attribution.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct MusicLyrics {
    /// Untimed lyric text when available.
    pub plain_lyrics: Option<String>,
    /// Lyric text with LRC timestamps when available.
    pub synced_lyrics: Option<String>,
    /// Whether this is an instrumental recording.
    pub instrumental: bool,
    /// Provider name.
    pub source: String,
    /// Public provider page.
    pub source_url: String,
}

impl MusicLyrics {
    pub(crate) fn empty() -> Self {
        Self {
            plain_lyrics: None,
            synced_lyrics: None,
            instrumental: false,
            source: "LRCLIB".into(),
            source_url: "https://lrclib.net/".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Lookup {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: Option<i64>,
}

impl Lookup {
    pub(crate) fn from_item(item: &super::MediaItemDetail) -> Option<Self> {
        if item.item_type != "track"
            || !item.has_metadata
            || item.linked_media_type.as_deref() != Some("recording")
            || item.metadata_refresh_state.as_deref() != Some("fresh")
        {
            return None;
        }
        let album = item
            .hierarchy
            .iter()
            .find(|parent| parent.item_type == "album" && parent.has_metadata)?;
        let music = item.music.as_ref()?;
        Some(Self {
            title: item.display_title.trim().into(),
            artist: music.artist.trim().into(),
            album: album.display_title.trim().into(),
            duration_ms: item.duration_ms.filter(|duration| *duration > 0),
        })
    }

    pub(crate) fn params(&self) -> Vec<(&'static str, String)> {
        let mut params = vec![
            ("track_name", self.title.clone()),
            ("artist_name", self.artist.clone()),
            ("album_name", self.album.clone()),
        ];
        if let Some(duration) = self.duration_ms {
            params.push(("duration", format!("{:.3}", duration as f64 / 1000.0)));
        }
        params
    }
}

#[derive(Debug, Clone, Queryable, Selectable, Insertable, AsChangeset, Identifiable)]
#[diesel(table_name = music_lyrics, primary_key(media_item_id), treat_none_as_null = true)]
pub(crate) struct StoredLyrics {
    pub media_item_id: i32,
    pub track_title: String,
    pub artist_name: String,
    pub album_name: String,
    pub duration_ms: Option<i64>,
    pub plain_lyrics: Option<String>,
    pub synced_lyrics: Option<String>,
    pub instrumental: bool,
    pub source: String,
    pub source_url: String,
    pub lookup_status: String,
    pub checked_at: i64,
    pub next_check_at: i64,
    pub last_error: Option<String>,
}

impl StoredLyrics {
    pub(crate) fn matches(
        &self,
        lookup: &Lookup,
    ) -> bool {
        self.track_title == lookup.title
            && self.artist_name == lookup.artist
            && self.album_name == lookup.album
            && self.duration_ms == lookup.duration_ms
    }

    pub(crate) fn is_fresh(
        &self,
        lookup: &Lookup,
        now: i64,
    ) -> bool {
        self.matches(lookup) && (self.lookup_status == "found" || now < self.next_check_at)
    }

    pub(crate) fn lyrics(&self) -> MusicLyrics {
        MusicLyrics {
            plain_lyrics: self.plain_lyrics.clone(),
            synced_lyrics: self.synced_lyrics.clone(),
            instrumental: self.instrumental,
            source: self.source.clone(),
            source_url: self.source_url.clone(),
        }
    }

    pub(crate) fn result(
        id: i32,
        lookup: &Lookup,
        lyrics: MusicLyrics,
        now: i64,
    ) -> Self {
        let found = lyrics.instrumental
            || lyrics
                .plain_lyrics
                .as_ref()
                .is_some_and(|text| !text.trim().is_empty())
            || lyrics
                .synced_lyrics
                .as_ref()
                .is_some_and(|text| !text.trim().is_empty());
        Self {
            media_item_id: id,
            track_title: lookup.title.clone(),
            artist_name: lookup.artist.clone(),
            album_name: lookup.album.clone(),
            duration_ms: lookup.duration_ms,
            plain_lyrics: lyrics.plain_lyrics,
            synced_lyrics: lyrics.synced_lyrics,
            instrumental: lyrics.instrumental,
            source: lyrics.source,
            source_url: lyrics.source_url,
            lookup_status: if found { "found" } else { "missing" }.into(),
            checked_at: now,
            next_check_at: now + 7 * 86400,
            last_error: None,
        }
    }

    pub(crate) fn failure(
        id: i32,
        lookup: &Lookup,
        previous: Option<Self>,
        now: i64,
    ) -> Self {
        let mut stored = previous
            .filter(|stored| stored.matches(lookup) && stored.lookup_status == "found")
            .unwrap_or_else(|| {
                Self::result(
                    id,
                    lookup,
                    MusicLyrics {
                        plain_lyrics: None,
                        synced_lyrics: None,
                        instrumental: false,
                        source: "LRCLIB".into(),
                        source_url: "https://lrclib.net/".into(),
                    },
                    now,
                )
            });
        if stored.lookup_status != "found" {
            stored.lookup_status = "error".into();
        }
        stored.checked_at = now;
        stored.next_check_at = now + 15 * 60;
        stored.last_error = Some("Lyrics provider temporarily unavailable".into());
        stored
    }
}

pub(crate) fn load(
    conn: &mut SqliteConnection,
    id: i32,
) -> QueryResult<Option<StoredLyrics>> {
    music_lyrics::table
        .find(id)
        .select(StoredLyrics::as_select())
        .first(conn)
        .optional()
}

/// Read matching saved lyrics without contacting a provider or changing lookup state.
pub(crate) fn for_item(
    conn: &mut SqliteConnection,
    item: &super::MediaItemDetail,
) -> QueryResult<Option<MusicLyrics>> {
    let Some(lookup) = Lookup::from_item(item) else {
        return Ok(None);
    };
    Ok(load(conn, item.id)?
        .filter(|stored| stored.matches(&lookup) && stored.lookup_status == "found")
        .map(|stored| stored.lyrics()))
}

/// Recheck identity under the write transaction so a rematch cannot save stale lyrics.
pub(crate) fn save(
    conn: &mut SqliteConnection,
    lookup: &Lookup,
    stored: &StoredLyrics,
) -> QueryResult<bool> {
    conn.transaction(|conn| {
        let current = super::get_media_item(conn, stored.media_item_id, "")?;
        if current.as_ref().and_then(Lookup::from_item).as_ref() != Some(lookup) {
            return Ok(false);
        }
        diesel::insert_into(music_lyrics::table)
            .values(stored)
            .on_conflict(music_lyrics::media_item_id)
            .do_update()
            .set(stored)
            .execute(conn)?;
        Ok(true)
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use diesel::connection::SimpleConnection;

    pub(crate) fn seed(
        conn: &mut SqliteConnection,
        id: i32,
    ) -> Lookup {
        crate::db::run_pending_sqlite_migrations(conn).unwrap();
        crate::db::configure_sqlite_connection(conn).unwrap();
        use crate::db::schema::{
            media_items,
            media_libraries,
            music_item_metadata,
        };
        diesel::insert_into(media_libraries::table)
            .values((
                media_libraries::id.eq(1),
                media_libraries::name.eq("Music"),
                media_libraries::path.eq("/music"),
                media_libraries::kind.eq("music"),
                media_libraries::metadata_providers_json.eq("[\"musicbrainz\",\"lrclib\"]"),
            ))
            .execute(conn)
            .unwrap();
        let album_id = id + 1;
        diesel::insert_into(media_items::table)
            .values((
                media_items::id.eq(album_id),
                media_items::library_id.eq(1),
                media_items::identity_key.eq("album"),
                media_items::item_type.eq("album"),
                media_items::display_title.eq("Album"),
            ))
            .execute(conn)
            .unwrap();
        diesel::insert_into(media_items::table)
            .values((
                media_items::id.eq(id),
                media_items::library_id.eq(1),
                media_items::parent_id.eq(album_id),
                media_items::identity_key.eq("song"),
                media_items::item_type.eq("track"),
                media_items::display_title.eq("Song"),
                media_items::duration_ms.eq(180000),
            ))
            .execute(conn)
            .unwrap();
        diesel::insert_into(music_item_metadata::table)
            .values((
                music_item_metadata::media_item_id.eq(id),
                music_item_metadata::artist.eq("Artist"),
                music_item_metadata::album_artist.eq("Artist"),
                music_item_metadata::album.eq("Album"),
            ))
            .execute(conn)
            .unwrap();
        for (item_id, external_id, media_type, title) in [
            (
                album_id,
                "d817f725-66c3-366d-8dd1-b22d49c8a766",
                "release",
                "Album",
            ),
            (
                id,
                "24cc8311-98fd-423a-bed1-97728f5eabc5",
                "recording",
                "Song",
            ),
        ] {
            crate::metadata::upsert_item_metadata_snapshot(
                conn,
                item_id,
                &crate::metadata::StoredMetadataSnapshot {
                    provider_id: crate::config::MetadataProviderId::MusicBrainz,
                    external_id: external_id.into(),
                    media_type: Some(media_type.into()),
                    title: Some(title.into()),
                    overview: None,
                    artwork_url: None,
                    backdrop_url: None,
                    release_year: None,
                    locale_key: crate::metadata::DEFAULT_METADATA_LOCALE.into(),
                    provider_locale_key: None,
                    provider_payload_json: None,
                },
            )
            .unwrap();
        }
        Lookup::from_item(&super::super::get_media_item(conn, id, "").unwrap().unwrap()).unwrap()
    }

    fn sample() -> MusicLyrics {
        MusicLyrics {
            plain_lyrics: Some("Sample lyric".into()),
            synced_lyrics: Some("[00:01.00]Sample lyric".into()),
            instrumental: false,
            source: "LRCLIB".into(),
            source_url: "https://lrclib.net/".into(),
        }
    }

    #[test]
    fn song_details_include_only_matching_saved_lyrics() {
        use crate::db::schema::music_item_metadata;

        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        let lookup = seed(&mut conn, 1);
        let detail = |conn: &mut SqliteConnection| {
            super::super::get_media_item(conn, 1, "").unwrap().unwrap()
        };
        assert!(detail(&mut conn).lyrics.is_none());
        assert!(
            save(
                &mut conn,
                &lookup,
                &StoredLyrics::result(1, &lookup, sample(), 100)
            )
            .unwrap()
        );
        assert_eq!(detail(&mut conn).lyrics, Some(sample()));
        diesel::update(music_item_metadata::table.find(1))
            .set(music_item_metadata::artist.eq("Another artist"))
            .execute(&mut conn)
            .unwrap();
        assert!(detail(&mut conn).lyrics.is_none());
    }

    #[test]
    fn lyrics_survive_reopening_database_and_deleted_tracks_cascade() {
        let path =
            std::env::temp_dir().join(format!("koko-lyrics-{}.sqlite", rand::random::<u64>()));
        {
            let mut conn = SqliteConnection::establish(path.to_str().unwrap()).unwrap();
            let lookup = seed(&mut conn, 1);
            assert!(
                save(
                    &mut conn,
                    &lookup,
                    &StoredLyrics::result(1, &lookup, sample(), 100)
                )
                .unwrap()
            );
        }
        {
            let mut conn = SqliteConnection::establish(path.to_str().unwrap()).unwrap();
            crate::db::configure_sqlite_connection(&mut conn).unwrap();
            let stored = load(&mut conn, 1).unwrap().unwrap();
            assert_eq!(stored.lyrics(), sample());
            assert_eq!(stored.lookup_status, "found");
            conn.batch_execute("DELETE FROM media_items WHERE id=1;")
                .unwrap();
            assert!(load(&mut conn, 1).unwrap().is_none());
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn lookup_freshness_distinguishes_matches_misses_errors_and_identity_changes() {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        let lookup = seed(&mut conn, 1);
        let found = StoredLyrics::result(1, &lookup, sample(), 100);
        assert!(found.is_fresh(&lookup, i64::MAX));
        let mut empty = sample();
        empty.plain_lyrics = None;
        empty.synced_lyrics = None;
        let missing = StoredLyrics::result(1, &lookup, empty.clone(), 100);
        assert_eq!(missing.lookup_status, "missing");
        assert!(missing.is_fresh(&lookup, 101));
        assert!(!missing.is_fresh(&lookup, 100 + 7 * 86400));
        let error = StoredLyrics::failure(1, &lookup, Some(missing), 100);
        assert_eq!(error.lookup_status, "error");
        assert!(error.is_fresh(&lookup, 101));
        assert!(!error.is_fresh(&lookup, 100 + 15 * 60));
        empty.instrumental = true;
        assert_eq!(
            StoredLyrics::result(1, &lookup, empty, 100).lookup_status,
            "found"
        );
        for changed in [
            Lookup {
                title: "Another song".into(),
                ..lookup.clone()
            },
            Lookup {
                artist: "Another artist".into(),
                ..lookup.clone()
            },
            Lookup {
                album: "Another album".into(),
                ..lookup.clone()
            },
            Lookup {
                duration_ms: Some(210000),
                ..lookup.clone()
            },
        ] {
            assert!(!found.is_fresh(&changed, 101));
            assert_eq!(
                StoredLyrics::failure(1, &changed, Some(found.clone()), 101).lookup_status,
                "error"
            );
        }
        let preserved = StoredLyrics::failure(1, &lookup, Some(found), 101);
        assert_eq!(preserved.lyrics(), sample());
        assert_eq!(preserved.lookup_status, "found");
        assert!(preserved.last_error.is_some());
    }

    #[test]
    fn rematches_clear_lyrics_and_obsolete_identities_cannot_restore_them() {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        let lookup = seed(&mut conn, 1);
        let stored = StoredLyrics::result(1, &lookup, sample(), 100);
        assert!(save(&mut conn, &lookup, &stored).unwrap());
        crate::metadata::clear_item_metadata_tree(&mut conn, 1, false).unwrap();
        assert!(load(&mut conn, 1).unwrap().is_none());
        conn.batch_execute(
            "UPDATE music_item_metadata SET artist='Different artist' WHERE media_item_id=1;",
        )
        .unwrap();
        assert!(!save(&mut conn, &lookup, &stored).unwrap());
        assert!(load(&mut conn, 1).unwrap().is_none());
    }
}
