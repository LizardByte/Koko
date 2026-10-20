//! Authenticated, per-user music playlist endpoints.
use crate::{
    auth::UserGuard,
    db::DbConn,
    media::playlists::{
        self,
        MusicPlaylistDetail,
    },
};
use rocket::{
    delete,
    get,
    http::Status,
    post,
    put,
    serde::json::Json,
};
use rocket_okapi::{
    okapi::schemars::{
        self,
        JsonSchema,
    },
    openapi,
};
use serde::Deserialize;

fn owner(user: &UserGuard) -> Result<i32, Status> {
    user.claims().sub.parse().map_err(|_| Status::Unauthorized)
}
fn status(error: diesel::result::Error) -> Status {
    if matches!(error, diesel::result::Error::NotFound) {
        Status::NotFound
    } else {
        log::error!("Playlist operation failed: {error}");
        Status::InternalServerError
    }
}

/// Payload for saving the playing queue as a new playlist.
#[derive(Deserialize, JsonSchema)]
pub struct SavePlaylist {
    /// Playlist name, between one and 200 characters.
    pub name: String,
    /// Track IDs in playing order, including repeated tracks.
    pub item_ids: Vec<i32>,
    /// Create an unsaved, resumable radio mix.
    #[serde(default)]
    pub temporary: bool,
    /// Artist, album, or song used to start radio; determines its stable artwork source.
    #[serde(default)]
    pub seed_item_id: Option<i32>,
}

/// Return the current user's saved playlists.
#[openapi(tag = "Media")]
#[get("/api/v1/playlists?<library_id>")]
pub async fn list(
    user: UserGuard,
    db: DbConn,
    library_id: Option<i32>,
) -> Result<Json<Vec<MusicPlaylistDetail>>, Status> {
    let owner = owner(&user)?;
    db.run(move |conn| playlists::list(conn, owner, library_id))
        .await
        .map(Json)
        .map_err(status)
}

/// Read one owned playlist.
#[openapi(tag = "Media")]
#[get("/api/v1/playlists/<id>")]
pub async fn get(
    user: UserGuard,
    db: DbConn,
    id: i32,
) -> Result<Json<MusicPlaylistDetail>, Status> {
    let owner = owner(&user)?;
    db.run(move |conn| playlists::load(conn, owner, id))
        .await
        .map(Json)
        .map_err(status)
}

/// Save a playing queue as a private playlist.
#[openapi(tag = "Media")]
#[post("/api/v1/playlists", data = "<body>")]
pub async fn create(
    user: UserGuard,
    db: DbConn,
    body: Json<SavePlaylist>,
) -> Result<Json<MusicPlaylistDetail>, Status> {
    let owner = owner(&user)?;
    let body = body.into_inner();
    let name = body.name.trim().to_string();
    if name.is_empty()
        || name.chars().count() > 200
        || body.item_ids.is_empty()
        || body.item_ids.len() > 5000
    {
        return Err(Status::BadRequest);
    }
    db.run(move |conn| {
        if body.temporary {
            playlists::create_radio(conn, owner, &name, &body.item_ids, body.seed_item_id)
        } else {
            playlists::create(conn, owner, &name, &body.item_ids)
        }
    })
    .await
    .map(Json)
    .map_err(status)
}

/// Remove one of the current user's saved playlists.
#[openapi(tag = "Media")]
#[delete("/api/v1/playlists/<id>")]
pub async fn delete(
    user: UserGuard,
    db: DbConn,
    id: i32,
) -> Result<Status, Status> {
    let owner = owner(&user)?;
    db.run(move |conn| playlists::delete(conn, owner, id))
        .await
        .map(|()| Status::NoContent)
        .map_err(status)
}

/// Save actual queue order, resume position, and playback preferences.
#[openapi(tag = "Media")]
#[put("/api/v1/playlists/<id>/queue", data = "<body>")]
pub async fn update_queue(
    user: UserGuard,
    db: DbConn,
    id: i32,
    body: Json<playlists::QueueState>,
) -> Result<Status, Status> {
    let user = owner(&user)?;
    let body = body.into_inner();
    if body.item_ids.is_empty()
        || body.item_ids.len() > 5000
        || body.current_index < 0
        || body.current_index as usize >= body.item_ids.len()
        || body.position_ms < 0
        || !["off", "all", "one"].contains(&body.repeat_mode.as_str())
    {
        return Err(Status::BadRequest);
    }
    db.run(move |conn| playlists::update_queue(conn, user, id, &body))
        .await
        .map(|()| Status::NoContent)
        .map_err(status)
}

/// Convert a temporary radio into a saved playlist without making a second queue.
#[openapi(tag = "Media")]
#[put("/api/v1/playlists/<id>/name", data = "<body>")]
pub async fn save_radio(
    user: UserGuard,
    db: DbConn,
    id: i32,
    body: Json<SavePlaylistName>,
) -> Result<Json<MusicPlaylistDetail>, Status> {
    let user = owner(&user)?;
    let name = body.into_inner().name.trim().to_string();
    if name.is_empty() || name.chars().count() > 200 {
        return Err(Status::BadRequest);
    }
    db.run(move |conn| playlists::rename_and_save(conn, user, id, &name))
        .await
        .map(Json)
        .map_err(status)
}

/// A name for an existing queue.
#[derive(Deserialize, JsonSchema)]
pub struct SavePlaylistName {
    /// Nonempty playlist name of at most 200 characters.
    pub name: String,
}
