//! Stored lyrics and radio. Audio always comes from authorized local
//! files.
pub use crate::media::lyrics::MusicLyrics;
use crate::media::lyrics::{
    self as lyrics_store,
    Lookup,
    StoredLyrics,
};
use crate::{
    auth::UserGuard,
    db::DbConn,
    media::{
        self,
        MediaItemSummary,
    },
};
use diesel::{
    ExpressionMethods,
    QueryDsl,
    RunQueryDsl,
};
use once_cell::sync::Lazy;
use rocket::{
    get,
    http::Status,
    serde::json::Json,
    tokio::sync::Mutex,
};
use rocket_okapi::{
    okapi::schemars::{
        self,
        JsonSchema,
    },
    openapi,
};
use serde::Serialize;
use std::{
    collections::{
        HashMap,
        HashSet,
    },
    sync::{
        Arc,
        Weak,
    },
    time::{
        Duration,
        Instant,
    },
};

static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(6))
        .user_agent(concat!(
            "Koko/",
            env!("CARGO_PKG_VERSION"),
            " (https://github.com/LizardByte/Koko)"
        ))
        .build()
        .expect("music discovery HTTP client")
});
type Cached = (Instant, serde_json::Value);
const RECORDING_ALGORITHM: &str = concat!(
    "session_based_days_7500_session_300_contribution_5_",
    "threshold_15_limit_50_skip_30_top_n_listeners_1000"
);
const ARTIST_ALGORITHM: &str = concat!(
    "session_based_days_7500_session_300_contribution_5_",
    "threshold_10_limit_100_filter_True_skip_30"
);
static CACHE: Lazy<Mutex<HashMap<String, Cached>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static LYRIC_LOOKUPS: Lazy<Mutex<HashMap<i32, Weak<Mutex<()>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[cfg(test)]
pub(super) use crate::metadata::TEST_LYRICS_URL;

async fn provider_json(
    url: &str,
    params: &[(&str, String)],
) -> Result<Option<serde_json::Value>, Status> {
    let request = CLIENT
        .get(url)
        .query(params)
        .build()
        .map_err(|_| Status::BadRequest)?;
    let key = request.url().to_string();
    if let Some((time, payload)) = CACHE.lock().await.get(&key) {
        if time.elapsed() < Duration::from_secs(3600) {
            return Ok(Some(payload.clone()));
        }
    }
    let first = CLIENT
        .execute(request.try_clone().ok_or(Status::InternalServerError)?)
        .await;
    let retry = first.as_ref().map_or(true, |response| {
        response.status().is_server_error()
            || response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
    });
    // Retry transient recommendation-service failures once before using local radio.
    let response = if retry {
        rocket::tokio::time::sleep(Duration::from_millis(250)).await;
        CLIENT.execute(request).await
    } else {
        first
    }
    .map_err(|error| {
        log::warn!("Music discovery request failed: {error}");
        Status::BadGateway
    })?;
    let mut response = if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    } else {
        response
            .error_for_status()
            .map_err(|_| Status::BadGateway)?
    };
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Status::BadGateway)? {
        if bytes.len() + chunk.len() > 1024 * 1024 {
            return Err(Status::BadGateway);
        }
        bytes.extend_from_slice(&chunk);
    }
    let payload =
        serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|_| Status::BadGateway)?;
    let mut cache = CACHE.lock().await;
    if cache.len() >= 128 {
        cache.clear();
    }
    cache.insert(key, (Instant::now(), payload.clone()));
    Ok(Some(payload))
}

/// Read saved lyrics without contacting a provider or modifying lookup outcomes.
#[openapi(tag = "Media")]
#[get("/api/v1/items/<id>/lyrics")]
pub async fn lyrics(
    user: UserGuard,
    db: DbConn,
    id: i32,
) -> Result<Json<MusicLyrics>, Status> {
    super::media::require_item_access(&db, id, &user).await?;
    read_track_lyrics(&db, id).await.map(Json)
}

async fn read_track_lyrics(
    db: &DbConn,
    id: i32,
) -> Result<MusicLyrics, Status> {
    db.run(move |conn| {
        let item = media::get_media_item(conn, id, "")?.ok_or(diesel::result::Error::NotFound)?;
        let Some(_lookup) = Lookup::from_item(&item) else {
            return Ok::<_, diesel::result::Error>(None);
        };
        Ok(Some(item.lyrics.unwrap_or_else(MusicLyrics::empty)))
    })
    .await
    .map_err(|error| match error {
        diesel::result::Error::NotFound => Status::NotFound,
        _ => Status::InternalServerError,
    })?
    .ok_or(Status::BadRequest)
}

async fn refresh_track_lyrics(
    db: &DbConn,
    id: i32,
    force: bool,
) -> Result<MusicLyrics, Status> {
    super::metadata_jobs::MetadataRevision::inherited(id)
        .scope(refresh_track_lyrics_inner(db, id, force))
        .await
}

async fn refresh_track_lyrics_inner(
    db: &DbConn,
    id: i32,
    force: bool,
) -> Result<MusicLyrics, Status> {
    let lock = {
        let mut locks = LYRIC_LOOKUPS.lock().await;
        locks.retain(|_, lock| lock.strong_count() > 0);
        let lock = locks.get(&id).and_then(Weak::upgrade).unwrap_or_default();
        locks.insert(id, Arc::downgrade(&lock));
        lock
    };
    let _guard = lock.lock().await;
    let (lookup, previous, selected) = db
        .run(move |conn| {
            let item =
                media::get_media_item(conn, id, "")?.ok_or(diesel::result::Error::NotFound)?;
            Ok::<_, diesel::result::Error>((
                Lookup::from_item(&item),
                lyrics_store::load(conn, id)?,
                media::get_library_metadata_providers(conn, item.library_id)?.is_some_and(
                    |providers| providers.contains(&crate::config::MetadataProviderId::Lrclib),
                ),
            ))
        })
        .await
        .map_err(|error| match error {
            diesel::result::Error::NotFound => Status::NotFound,
            _ => Status::InternalServerError,
        })?;
    let lookup = lookup.ok_or(Status::BadRequest)?;
    if !selected {
        return Ok(previous
            .filter(|stored| stored.matches(&lookup) && stored.lookup_status == "found")
            .map(|stored| stored.lyrics())
            .unwrap_or_else(MusicLyrics::empty));
    }
    let now = crate::utils::current_timestamp();
    if let Some(stored) = previous
        .as_ref()
        .filter(|stored| !force && stored.is_fresh(&lookup, now))
    {
        return if stored.lookup_status == "error" {
            Err(Status::BadGateway)
        } else {
            Ok(stored.lyrics())
        };
    }
    let stored = match crate::metadata::fetch_lyrics(&lookup).await {
        Ok(data) => StoredLyrics::result(id, &lookup, data, now),
        Err(error) => {
            log::debug!("LRCLIB lookup for track {id} failed: {error}");
            StoredLyrics::failure(id, &lookup, previous, now)
        }
    };
    let response =
        if stored.lookup_status == "error" { Err(Status::BadGateway) } else { Ok(stored.lyrics()) };
    let saved =
        super::metadata_jobs::write(db, move |conn| lyrics_store::save(conn, &lookup, &stored))
            .await
            .map_err(|error| match error {
                diesel::result::Error::RollbackTransaction => Status::Conflict,
                _ => Status::InternalServerError,
            })?;
    if !saved {
        return Err(Status::Conflict);
    }
    response
}

/// Optional lyric failures must not fail a primary metadata refresh.
pub(super) async fn refresh_metadata_lyrics(
    db: &DbConn,
    id: i32,
    force: bool,
) {
    if let Err(status) = refresh_track_lyrics(db, id, force).await {
        log::debug!("Lyrics refresh for track {id}: {status}");
    }
}

/// A radio mix contains only authorized songs already available on the server.
#[derive(Serialize, JsonSchema)]
pub struct MusicRadio {
    /// Available local songs in suggested playing order.
    pub items: Vec<MediaItemSummary>,
    /// Provider or local fallback used for recommendations.
    pub source: String,
    /// Explanation when recommendations fall back to local metadata.
    pub message: Option<String>,
}
struct RadioCatalog {
    seed: MediaItemSummary,
    items: Vec<MediaItemSummary>,
    ids: HashMap<i32, (String, String)>,
    parents: HashMap<i32, Option<i32>>,
    genres: HashMap<i32, Vec<String>>,
}

fn lineage(
    item: i32,
    catalog: &RadioCatalog,
) -> Vec<i32> {
    let mut ids = Vec::new();
    let mut current = Some(item);
    while let Some(id) = current {
        if ids.contains(&id) {
            break;
        }
        ids.push(id);
        current = catalog.parents.get(&id).copied().flatten();
    }
    ids
}

fn inherited_genres(
    item: i32,
    catalog: &RadioCatalog,
) -> HashSet<String> {
    lineage(item, catalog)
        .into_iter()
        .find_map(|id| catalog.genres.get(&id).filter(|genres| !genres.is_empty()))
        .map(|genres| genres.iter().map(|genre| genre.to_lowercase()).collect())
        .unwrap_or_default()
}

fn artist_id(
    item: &MediaItemSummary,
    catalog: &RadioCatalog,
) -> Option<String> {
    let mut current = Some(item.id);
    let mut seen = HashSet::new();
    while let Some(id) = current {
        if !seen.insert(id) {
            break;
        }
        if let Some((kind, mbid)) = catalog.ids.get(&id) {
            if kind == "artist" {
                return Some(mbid.clone());
            }
        }
        current = catalog.parents.get(&id).copied().flatten();
    }
    None
}

/// Start a radio mix from a song, album, or artist using ListenBrainz similarities.
#[openapi(tag = "Media")]
#[get("/api/v1/items/<id>/radio")]
pub async fn radio(
    user: UserGuard,
    db: DbConn,
    id: i32,
) -> Result<Json<MusicRadio>, Status> {
    super::media::require_item_access(&db, id, &user).await?;
    let owner = user
        .claims()
        .sub
        .parse::<i32>()
        .map_err(|_| Status::Unauthorized)?;
    let catalog = db
        .run(move |conn| {
            use crate::db::schema::item_metadata_links::dsl as links;
            let items = media::list_media_items_for_user_with_preferred_languages(
                conn,
                Some(owner),
                None,
                &[],
            )?;
            let seed = items
                .iter()
                .find(|item| {
                    item.id == id && ["track", "album", "artist"].contains(&item.item_type.as_str())
                })
                .cloned()
                .ok_or(diesel::result::Error::NotFound)?;
            let visible_ids = items
                .iter()
                .filter(|item| item.music.is_some())
                .map(|item| item.id)
                .collect::<Vec<_>>();
            let mut ids = HashMap::new();
            for chunk in visible_ids.chunks(500) {
                for (id, kind, mbid) in links::item_metadata_links
                    .filter(links::media_item_id.eq_any(chunk))
                    .filter(links::provider_id.eq("musicbrainz"))
                    .filter(links::relation_kind.eq("primary"))
                    .filter(links::match_state.eq("linked"))
                    .order(links::updated_at.asc())
                    .select((links::media_item_id, links::media_type, links::external_id))
                    .load::<(i32, Option<String>, String)>(conn)?
                {
                    if media::valid_mbid(&mbid) {
                        ids.insert(id, (kind.unwrap_or_default(), mbid));
                    }
                }
            }
            let parents = items.iter().map(|item| (item.id, item.parent_id)).collect();
            let genres = items
                .iter()
                .map(|item| (item.id, item.genres.clone()))
                .collect();
            Ok::<_, diesel::result::Error>(RadioCatalog {
                seed,
                items,
                ids,
                parents,
                genres,
            })
        })
        .await
        .map_err(|error| {
            if matches!(error, diesel::result::Error::NotFound) {
                Status::NotFound
            } else {
                Status::InternalServerError
            }
        })?;
    let recording = catalog
        .ids
        .get(&id)
        .filter(|(kind, _)| kind == "recording")
        .map(|(_, id)| id.clone());
    let artist = artist_id(&catalog.seed, &catalog);
    let mut recordings = HashMap::new();
    let mut artists = HashMap::new();
    let mut provider_available = true;
    if let Some(mbid) = recording {
        let data = provider_json(
            "https://labs.api.listenbrainz.org/similar-recordings/json",
            &[
                ("recording_mbids", mbid),
                ("algorithm", RECORDING_ALGORITHM.into()),
            ],
        )
        .await;
        match data {
            Ok(Some(serde_json::Value::Array(rows))) => {
                for (index, row) in rows.iter().enumerate() {
                    if let Some(id) = row["recording_mbid"].as_str() {
                        recordings.insert(id.to_string(), index);
                    }
                }
            }
            _ => provider_available = false,
        }
    }
    if let Some(mbid) = &artist {
        let data = provider_json(
            "https://labs.api.listenbrainz.org/similar-artists/json",
            &[
                ("artist_mbids", mbid.clone()),
                ("algorithm", ARTIST_ALGORITHM.into()),
            ],
        )
        .await;
        match data {
            Ok(Some(serde_json::Value::Array(rows))) => {
                for (index, row) in rows.iter().enumerate() {
                    if let Some(id) = row["artist_mbid"].as_str() {
                        artists.insert(id.to_string(), index);
                    }
                }
            }
            _ => provider_available = false,
        }
    }
    Ok(Json(select_radio(
        &catalog,
        &recordings,
        &artists,
        provider_available,
    )))
}

fn select_radio(
    catalog: &RadioCatalog,
    recordings: &HashMap<String, usize>,
    artists: &HashMap<String, usize>,
    provider_available: bool,
) -> MusicRadio {
    let artist = artist_id(&catalog.seed, catalog);
    let mut ranked = Vec::new();
    let seed_genres = inherited_genres(catalog.seed.id, catalog);
    let seed_artist = lineage(catalog.seed.id, catalog).last().copied();
    let mut used_provider = false;
    for item in &catalog.items {
        if item.item_type != "track" || !item.playable || item.missing_since.is_some() {
            continue;
        }
        let item_artist = artist_id(item, catalog);
        let recording_rank = catalog
            .ids
            .get(&item.id)
            .and_then(|(_, id)| recordings.get(id));
        let artist_rank = item_artist.as_ref().and_then(|id| artists.get(id));
        used_provider |= recording_rank.is_some() || artist_rank.is_some();
        let genres = inherited_genres(item.id, catalog)
            .intersection(&seed_genres)
            .count();
        let score = if item.id == catalog.seed.id {
            20000
        } else if let Some(rank) = recording_rank {
            10000 - *rank as i32
        } else if let Some(rank) = artist_rank {
            1000 - *rank as i32
        } else if genres > 0 {
            (genres * 20) as i32
        } else if (artist.is_some() && item_artist == artist)
            || lineage(item.id, catalog).last().copied() == seed_artist
        {
            10
        } else {
            0
        };
        if score > 0 {
            ranked.push((score, item.clone()));
        }
    }
    // Scores express relevance, but ID order groups entire artists/albums together.
    // Randomize ties and interleave artists/albums, relaxing only when the catalog requires it.
    let mut ranked = ranked
        .into_iter()
        .map(|(score, item)| (score, rand::random::<u64>(), item))
        .collect::<Vec<_>>();
    ranked.sort_by_key(|(score, tie, _)| (std::cmp::Reverse(*score), *tie));
    let items = diverse_radio(
        ranked.into_iter().map(|(_, _, item)| item).collect(),
        catalog,
    );
    MusicRadio {
        items,
        source: if used_provider { "ListenBrainz" } else { "Local library metadata" }.into(),
        message: (!used_provider || !provider_available).then(|| {
            "Using available songs from your libraries, with local artist and genre matches where \
             ListenBrainz recommendations are unavailable."
                .into()
        }),
    }
}

fn diverse_radio(
    mut remaining: Vec<MediaItemSummary>,
    catalog: &RadioCatalog,
) -> Vec<MediaItemSummary> {
    let mut result: Vec<MediaItemSummary> = Vec::new();
    let identity = |item: &MediaItemSummary| {
        artist_id(item, catalog).unwrap_or_else(|| {
            format!(
                "local:{}",
                lineage(item.id, catalog).last().copied().unwrap_or(item.id)
            )
        })
    };
    while !remaining.is_empty() && result.len() < 200 {
        let recent_artists = result
            .iter()
            .rev()
            .take(3)
            .map(&identity)
            .collect::<HashSet<_>>();
        let recent_albums = result
            .iter()
            .rev()
            .take(5)
            .filter_map(|item| item.parent_id)
            .collect::<HashSet<_>>();
        let index = remaining
            .iter()
            .position(|item| {
                !recent_artists.contains(&identity(item))
                    && item.parent_id.is_none_or(|id| !recent_albums.contains(&id))
            })
            .or_else(|| {
                remaining.iter().position(|item| {
                    result
                        .last()
                        .is_none_or(|last| identity(last) != identity(item))
                })
            })
            .unwrap_or(0);
        result.push(remaining.remove(index));
    }
    result
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use diesel::{
        Connection,
        connection::SimpleConnection,
    };

    pub(crate) async fn lyrics_database(id: i32) -> (rocket::Rocket<rocket::Ignite>, DbConn) {
        let rocket = rocket::custom(rocket::Config::figment().merge((
            "databases.sqlite_db",
            serde_json::json!({"url": ":memory:", "pool_size": 1}),
        )))
        .attach(DbConn::fairing())
        .ignite()
        .await
        .unwrap();
        let db = DbConn::get_one(&rocket).await.unwrap();
        db.run(move |conn| {
            lyrics_store::tests::seed(conn, id);
        })
        .await;
        (rocket, db)
    }

    pub(crate) fn mock_lyrics(
        responses: Vec<(&'static str, &'static str)>
    ) -> (String, std::thread::JoinHandle<()>) {
        use std::io::{
            Read,
            Write,
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/lyrics", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            for (status, body) in responses {
                // Metadata jobs can wait behind other scans in the full parallel test suite.
                let deadline = Instant::now() + Duration::from_secs(30);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error)
                            if error.kind() == std::io::ErrorKind::WouldBlock
                                && Instant::now() < deadline =>
                        {
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Err(error) => panic!("Expected a lyric request: {error}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = [0; 4096];
                let length = stream.read(&mut bytes).unwrap();
                let request = String::from_utf8_lossy(&bytes[..length]);
                assert!(request.contains("track_name=Song"));
                assert!(request.contains("artist_name=Artist"));
                assert!(request.contains("album_name=Album"));
                assert!(request.contains("duration=180.000"));
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: \
                     {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        (url, worker)
    }

    #[rocket::async_test]
    async fn playback_reads_saved_lyrics_without_lookup_even_for_missing_or_expired_results() {
        const TRACK: i32 = 192000005;
        let (_rocket, db) = lyrics_database(TRACK).await;
        TEST_LYRICS_URL
            .scope("http://127.0.0.1:1/must-not-contact".into(), async {
                assert_eq!(
                    read_track_lyrics(&db, TRACK).await.unwrap(),
                    MusicLyrics::empty()
                );
                assert!(
                    db.run(|conn| lyrics_store::load(conn, TRACK))
                        .await
                        .unwrap()
                        .is_none()
                );
                db.run(|conn| {
                    let item = media::get_media_item(conn, TRACK, "").unwrap().unwrap();
                    let lookup = Lookup::from_item(&item).unwrap();
                    lyrics_store::save(
                        conn,
                        &lookup,
                        &StoredLyrics::result(
                            TRACK,
                            &lookup,
                            MusicLyrics {
                                plain_lyrics: Some("Saved only".into()),
                                ..MusicLyrics::empty()
                            },
                            1,
                        ),
                    )
                    .unwrap();
                })
                .await;
                assert_eq!(
                    read_track_lyrics(&db, TRACK)
                        .await
                        .unwrap()
                        .plain_lyrics
                        .as_deref(),
                    Some("Saved only")
                );
                db.run(|conn| {
                    conn.batch_execute(
                        "UPDATE music_lyrics SET lookup_status='missing', next_check_at=1",
                    )
                    .unwrap()
                })
                .await;
                assert_eq!(
                    read_track_lyrics(&db, TRACK).await.unwrap(),
                    MusicLyrics::empty()
                );
                assert_eq!(
                    db.run(|conn| lyrics_store::load(conn, TRACK))
                        .await
                        .unwrap()
                        .unwrap()
                        .next_check_at,
                    1
                );
            })
            .await;
    }

    #[rocket::async_test]
    async fn lyrics_metadata_agent_selection_is_library_scoped_and_disabling_preserves_saved_text()
    {
        const TRACK: i32 = 192000006;
        let (_rocket, db) = lyrics_database(TRACK).await;
        db.run(|conn| {
            conn.batch_execute(
                "UPDATE media_libraries SET metadata_providers_json='[\"musicbrainz\"]'",
            )
            .unwrap()
        })
        .await;
        TEST_LYRICS_URL
            .scope("http://127.0.0.1:1/must-not-contact".into(), async {
                refresh_metadata_lyrics(&db, TRACK, true).await;
                assert!(
                    db.run(|conn| lyrics_store::load(conn, TRACK))
                        .await
                        .unwrap()
                        .is_none()
                );
            })
            .await;
        // No primary metadata provider or MusicBrainz match is necessary.
        db.run(|conn| {
            conn.batch_execute("UPDATE media_libraries SET metadata_providers_json='[\"lrclib\"]'")
                .unwrap()
        })
        .await;
        let (url, worker) = mock_lyrics(vec![(
            "200 OK",
            r#"{"plainLyrics":"Selected library lyrics"}"#,
        )]);
        TEST_LYRICS_URL
            .scope(url, refresh_metadata_lyrics(&db, TRACK, true))
            .await;
        worker.join().unwrap();
        let saved = db
            .run(|conn| lyrics_store::load(conn, TRACK))
            .await
            .unwrap()
            .unwrap();
        db.run(|conn| {
            conn.batch_execute(
                "UPDATE media_libraries SET metadata_providers_json='[\"musicbrainz\"]'",
            )
            .unwrap()
        })
        .await;
        TEST_LYRICS_URL
            .scope("http://127.0.0.1:1/must-not-contact".into(), async {
                refresh_metadata_lyrics(&db, TRACK, true).await;
                assert_eq!(read_track_lyrics(&db, TRACK).await.unwrap(), saved.lyrics());
                assert_eq!(
                    db.run(|conn| lyrics_store::load(conn, TRACK))
                        .await
                        .unwrap()
                        .unwrap()
                        .checked_at,
                    saved.checked_at
                );
            })
            .await;
    }

    #[rocket::async_test]
    async fn metadata_lyrics_are_saved_and_concurrent_refreshes_reuse_database() {
        let (_rocket, db) = lyrics_database(192000001).await;
        let (url, worker) = mock_lyrics(vec![(
            "200 OK",
            r#"{"plainLyrics":"Saved lyric","syncedLyrics":"[00:01.00]Saved lyric"}"#,
        )]);
        TEST_LYRICS_URL
            .scope(url, async {
                let (first, simultaneous) = tokio::join!(
                    refresh_track_lyrics(&db, 192000001, false),
                    refresh_track_lyrics(&db, 192000001, false),
                );
                assert_eq!(first.as_ref().unwrap(), simultaneous.as_ref().unwrap());
                worker.join().unwrap(); // Provider is now offline; this must come from SQL.
                let saved = db
                    .run(|conn| lyrics_store::load(conn, 192000001))
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(saved.lyrics(), first.unwrap());
                assert_eq!(
                    refresh_track_lyrics(&db, 192000001, false).await.unwrap(),
                    saved.lyrics()
                );
            })
            .await;
    }

    #[rocket::async_test]
    async fn missing_lyrics_are_saved_and_explicit_refresh_bypasses_them() {
        let (_rocket, db) = lyrics_database(192000002).await;
        let (url, worker) = mock_lyrics(vec![
            ("404 Not Found", "{}"),
            ("200 OK", r#"{"plainLyrics":"Found later"}"#),
        ]);
        TEST_LYRICS_URL
            .scope(url, async {
                assert!(
                    refresh_track_lyrics(&db, 192000002, false)
                        .await
                        .unwrap()
                        .plain_lyrics
                        .is_none()
                );
                assert!(
                    refresh_track_lyrics(&db, 192000002, false)
                        .await
                        .unwrap()
                        .plain_lyrics
                        .is_none()
                );
                let saved = db
                    .run(|conn| lyrics_store::load(conn, 192000002))
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(saved.lookup_status, "missing");
                assert_eq!(
                    refresh_track_lyrics(&db, 192000002, true)
                        .await
                        .unwrap()
                        .plain_lyrics
                        .as_deref(),
                    Some("Found later")
                );
                worker.join().unwrap();
            })
            .await;
    }

    #[rocket::async_test]
    async fn lyric_failures_have_retry_backoff_and_do_not_erase_saved_text() {
        let (_rocket, db) = lyrics_database(192000003).await;
        let (url, worker) = mock_lyrics(vec![
            ("503 Service Unavailable", "{}"),
            ("503 Service Unavailable", "{}"),
            ("200 OK", r#"{"plainLyrics":"Recovered"}"#),
            ("503 Service Unavailable", "{}"),
            ("503 Service Unavailable", "{}"),
        ]);
        TEST_LYRICS_URL
            .scope(url, async {
                assert_eq!(
                    refresh_track_lyrics(&db, 192000003, false)
                        .await
                        .unwrap_err(),
                    Status::BadGateway
                );
                assert_eq!(
                    refresh_track_lyrics(&db, 192000003, false)
                        .await
                        .unwrap_err(),
                    Status::BadGateway
                );
                let saved = db
                    .run(|conn| lyrics_store::load(conn, 192000003))
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(saved.lookup_status, "error");
                assert_eq!(saved.next_check_at - saved.checked_at, 900);
                let recovered = refresh_track_lyrics(&db, 192000003, true).await.unwrap();
                assert_eq!(recovered.plain_lyrics.as_deref(), Some("Recovered"));
                // Background refresh shares persistence, but an outage is optional and preserves
                // text.
                refresh_metadata_lyrics(&db, 192000003, true).await;
                worker.join().unwrap();
                assert_eq!(
                    refresh_track_lyrics(&db, 192000003, false).await.unwrap(),
                    recovered
                );
                assert!(
                    db.run(|conn| lyrics_store::load(conn, 192000003))
                        .await
                        .unwrap()
                        .unwrap()
                        .last_error
                        .is_some()
                );
            })
            .await;
    }

    #[rocket::async_test]
    async fn an_obsolete_lookup_cannot_restore_lyrics_after_a_same_identity_rematch() {
        let (_rocket, db) = lyrics_database(192000004).await;
        let revision = super::super::metadata_jobs::MetadataRevision::current(192000004);
        db.run(|conn| super::super::metadata_jobs::reset(conn, 192000004, false, |_| Ok(())))
            .await
            .unwrap();
        let (url, worker) = mock_lyrics(vec![("200 OK", r#"{"plainLyrics":"Obsolete"}"#)]);
        let result = TEST_LYRICS_URL
            .scope(
                url,
                revision.scope(refresh_track_lyrics(&db, 192000004, true)),
            )
            .await;
        worker.join().unwrap();
        assert_eq!(result.unwrap_err(), Status::Conflict);
        assert!(
            db.run(|conn| lyrics_store::load(conn, 192000004))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[rocket::async_test]
    async fn transient_provider_failure_is_retried_and_success_is_cached() {
        use std::io::{
            Read,
            Write,
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/lyrics", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            for status in [
                "503 Service Unavailable",
                "200 OK",
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut bytes = [0; 4096];
                assert!(stream.read(&mut bytes).unwrap() > 0);
                let body = r#"{"plainLyrics":"sample"}"#;
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: \
                     {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        let value = provider_json(&url, &[]).await.unwrap().unwrap();
        assert_eq!(value["plainLyrics"], "sample");
        worker.join().unwrap();
        assert_eq!(provider_json(&url, &[]).await.unwrap().unwrap(), value);
    }

    #[test]
    fn radio_interleaves_artists_and_albums_instead_of_exhausting_id_sorted_groups() {
        let mut conn = diesel::SqliteConnection::establish(":memory:").unwrap();
        crate::db::run_pending_sqlite_migrations(&mut conn).unwrap();
        conn.batch_execute(
            "INSERT INTO users(id,username,password,admin) VALUES(1,'viewer','hash',0);
            INSERT INTO media_libraries(id,name,path,kind) VALUES(1,'Music','/music','music');
            INSERT INTO \
             media_items(id,library_id,identity_key,item_type,display_title,playable,media_kind) \
             VALUES(1,1,'seed','track','Seed',1,'audio');",
        )
        .unwrap();
        let seed = media::list_media_items_for_user_with_preferred_languages(
            &mut conn,
            Some(1),
            None,
            &[],
        )
        .unwrap()
        .remove(0);
        let mut songs = Vec::new();
        let mut parents = HashMap::new();
        for artist in 1..=6 {
            for track in 1..=8 {
                let mut item = seed.clone();
                item.id = artist * 100 + track;
                item.parent_id = Some(artist * 10);
                parents.insert(item.id, item.parent_id);
                parents.insert(artist * 10, Some(artist * 1000));
                songs.push(item);
            }
        }
        let catalog = RadioCatalog {
            seed: songs[0].clone(),
            items: songs.clone(),
            ids: HashMap::new(),
            genres: HashMap::new(),
            parents,
        };
        let mixed = diverse_radio(songs, &catalog);
        assert_eq!(mixed.len(), 48);
        assert_eq!(
            mixed
                .iter()
                .map(|item| item.id)
                .collect::<HashSet<_>>()
                .len(),
            48
        );
        for group in mixed.chunks(6) {
            assert_eq!(
                group
                    .iter()
                    .map(|item| item.parent_id)
                    .collect::<HashSet<_>>()
                    .len(),
                6
            );
        }
        // A single-artist catalog still plays all tracks without duplicates or a deadlock.
        let one_artist = mixed
            .into_iter()
            .filter(|item| item.parent_id == Some(10))
            .collect();
        assert_eq!(diverse_radio(one_artist, &catalog).len(), 8);
    }

    #[test]
    fn radio_matches_ids_and_inherited_genres_without_missing_or_private_songs() {
        let mut conn = diesel::SqliteConnection::establish(":memory:").unwrap();
        crate::db::run_pending_sqlite_migrations(&mut conn).unwrap();
        conn.batch_execute(
            "INSERT INTO users(id,username,password,admin) VALUES(1,'viewer','hash',0);
            INSERT INTO media_libraries(id,name,path,kind,allowed_user_ids_json) VALUES
            (1,'Music','/music','music','[]'),(2,'Private','/secret','music','[3]');
            INSERT INTO \
             media_items(id,library_id,parent_id,identity_key,item_type,display_title,playable,\
             media_kind,missing_since) VALUES
            (1,1,NULL,'artist-a','artist','Artist A',0,'audio',NULL),
            (2,1,1,'album-a','album','Album A',0,'audio',NULL),
            (3,1,2,'track-a','track','Song A',1,'audio',NULL),
            (4,1,2,'missing','track','Missing song',1,'audio',1),
            (5,1,NULL,'artist-b','artist','Artist B',0,'audio',NULL),
            (6,1,5,'track-b','track','Song B',1,'audio',NULL),
            (7,2,NULL,'secret','track','Secret song',1,'audio',NULL);",
        )
        .unwrap();
        let items = media::list_media_items_for_user_with_preferred_languages(
            &mut conn,
            Some(1),
            None,
            &[],
        )
        .unwrap();
        let catalog = RadioCatalog {
            seed: items.iter().find(|item| item.id == 3).unwrap().clone(),
            parents: items.iter().map(|item| (item.id, item.parent_id)).collect(),
            genres: HashMap::from([
                (1, vec!["Rock".into()]),
                (5, vec!["rock".into()]),
            ]),
            ids: HashMap::from([
                (1, ("artist".into(), "artist-a".into())),
                (5, ("artist".into(), "artist-b".into())),
                (6, ("recording".into(), "recording-b".into())),
            ]),
            items,
        };
        let mix = select_radio(
            &catalog,
            &HashMap::from([("recording-b".into(), 0)]),
            &HashMap::new(),
            true,
        );
        assert_eq!(
            mix.items.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![3, 6]
        );
        assert_eq!(mix.source, "ListenBrainz");
        let fallback = select_radio(&catalog, &HashMap::new(), &HashMap::new(), false);
        assert_eq!(
            fallback
                .items
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            vec![3, 6]
        );
        assert_eq!(fallback.source, "Local library metadata");
        assert!(fallback.message.is_some());
    }
}
