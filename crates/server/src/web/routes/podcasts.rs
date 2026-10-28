//! Podcast discovery, account-owned subscriptions, and bounded publisher streaming.
use crate::{
    auth::UserGuard,
    db::DbConn,
    media::{
        MediaItemSummary,
        podcasts,
    },
    web::routes::media::require_item_access,
};
use rocket::{
    Request,
    delete,
    get,
    http::Status,
    post,
    put,
    response::{
        self,
        Responder,
        Response,
    },
    serde::json::Json,
};
use rocket_okapi::{
    okapi::schemars::{
        self,
        JsonSchema,
    },
    openapi,
};
use serde::{
    Deserialize,
    Serialize,
};

fn owner(user: &UserGuard) -> Result<i32, Status> {
    user.claims().sub.parse().map_err(|_| Status::Unauthorized)
}

/// Readable podcast operation failure.
#[derive(Serialize, JsonSchema)]
pub struct PodcastError {
    message: String,
}
type Error = rocket::response::status::Custom<Json<PodcastError>>;
fn error(message: String) -> Error {
    rocket::response::status::Custom(Status::BadGateway, Json(PodcastError { message }))
}

fn bad_request(message: String) -> Error {
    rocket::response::status::Custom(Status::BadRequest, Json(PodcastError { message }))
}

/// Return popular podcasts or directory search results without adding any to Koko.
#[openapi(tag = "Podcasts")]
#[get("/api/v1/podcasts/discover?<country>&<query>")]
pub async fn discover(
    _user: UserGuard,
    country: Option<String>,
    query: Option<String>,
) -> Result<Json<Vec<podcasts::DirectoryPodcast>>, Error> {
    podcasts::discover(
        country.as_deref().unwrap_or("us"),
        query.as_deref().unwrap_or(""),
    )
    .await
    .map(Json)
    .map_err(error)
}
/// Return only the current account's subscriptions.
#[openapi(tag = "Podcasts")]
#[get("/api/v1/podcasts")]
pub async fn list(
    user: UserGuard,
    db: DbConn,
) -> Result<Json<Vec<MediaItemSummary>>, Status> {
    let user = owner(&user)?;
    db.run(move |conn| podcasts::subscribed(conn, user))
        .await
        .map(Json)
        .map_err(|_| Status::InternalServerError)
}
/// A public RSS feed to subscribe to.
#[derive(Deserialize, JsonSchema)]
pub struct AddPodcast {
    /// Absolute HTTP or HTTPS feed URL.
    pub feed_url: String,
}
/// The subscribed show that can now be opened.
#[derive(Serialize, JsonSchema)]
pub struct AddedPodcast {
    /// Shared root podcast item identifier.
    pub item_id: i32,
}
/// Subscribe to a directory entry or a manually entered public RSS feed URL.
#[openapi(tag = "Podcasts")]
#[post("/api/v1/podcasts", format = "json", data = "<body>")]
pub async fn add(
    user: UserGuard,
    db: DbConn,
    body: Json<AddPodcast>,
) -> Result<Json<AddedPodcast>, Error> {
    let user = owner(&user).map_err(|_| error("Sign in to add a podcast".into()))?;
    if body.feed_url.len() > 8192 {
        return Err(bad_request("The feed URL is too long".into()));
    }
    let url = podcasts::normalize_url(&body.feed_url).map_err(bad_request)?;
    let item_id = podcasts::subscribe(&db, user, &url).await.map_err(error)?;
    Ok(Json(AddedPodcast { item_id }))
}
/// Remove the current account's subscription, preserving shared episodes and listening history.
#[openapi(tag = "Podcasts")]
#[delete("/api/v1/podcasts/<id>")]
pub async fn remove(
    user: UserGuard,
    db: DbConn,
    id: i32,
) -> Result<Status, Status> {
    let user = owner(&user)?;
    db.run(move |conn| podcasts::unsubscribe(conn, user, id))
        .await
        .map_err(|_| Status::InternalServerError)?;
    Ok(Status::NoContent)
}
/// Check a subscribed feed for new episodes immediately.
#[openapi(tag = "Podcasts")]
#[post("/api/v1/podcasts/<id>/refresh")]
pub async fn refresh(
    user: UserGuard,
    db: DbConn,
    id: i32,
) -> Result<Status, Error> {
    let user = owner(&user).map_err(|_| error("Sign in to refresh a podcast".into()))?;
    db.run(move |conn| podcasts::feed_for_user(conn, user, id))
        .await
        .map_err(|_| {
            rocket::response::status::Custom(
                Status::NotFound,
                Json(PodcastError {
                    message: "Podcast is unavailable".into(),
                }),
            )
        })?;
    podcasts::refresh(&db, user, id).await.map_err(error)?;
    Ok(Status::NoContent)
}

/// A streamed publisher response, preserving byte ranges for seeking without buffering the episode.
pub struct PodcastStream(Response<'static>);

/// Fetch a bounded publisher cover and serve only recognized raster image formats.
pub(super) async fn cover_image(url: &str) -> Result<PodcastStream, Status> {
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        let mut upstream = podcasts::request(url, None)
            .await
            .map_err(|_| Status::BadGateway)?;
        const LIMIT: usize = 8 * 1024 * 1024;
        if upstream
            .content_length()
            .is_some_and(|size| size > LIMIT as u64)
        {
            return Err(Status::BadGateway);
        }
        let mut data = Vec::new();
        while let Some(chunk) = upstream.chunk().await.map_err(|_| Status::BadGateway)? {
            if data.len() + chunk.len() > LIMIT {
                return Err(Status::BadGateway);
            }
            data.extend_from_slice(&chunk);
        }
        let format = image::guess_format(&data).map_err(|_| Status::BadGateway)?;
        let mut response = Response::build();
        response.raw_header("Content-Type", format.to_mime_type());
        response.raw_header("X-Content-Type-Options", "nosniff");
        response.sized_body(data.len(), std::io::Cursor::new(data));
        Ok(PodcastStream(response.finalize()))
    })
    .await
    .map_err(|_| Status::GatewayTimeout)?
}

impl<'r> Responder<'r, 'static> for PodcastStream {
    fn respond_to(
        self,
        _request: &'r Request<'_>,
    ) -> response::Result<'static> {
        Ok(self.0)
    }
}

/// Serve the shared local episode copy, downloading it on first play when needed.
#[get("/api/v1/podcasts/episodes/<id>/stream")]
pub async fn stream(
    user: UserGuard,
    db: DbConn,
    range: super::media::RangeHeader,
    id: i32,
) -> Result<PodcastStreamFile, Status> {
    require_item_access(&db, id, &user).await?;
    let path = podcasts::download_episode(&db, id).await.map_err(|error| {
        log::warn!("Podcast episode {id} download failed: {error}");
        Status::BadGateway
    })?;
    super::media::open_ranged_file(path, &range)
        .await
        .map(PodcastStreamFile)
}

/// A complete local podcast file with normal byte-range seeking.
#[derive(rocket::Responder)]
pub struct PodcastStreamFile(super::media::RangedFile);

/// Return the account's full podcast queue, including episodes in every season.
#[openapi(tag = "Podcasts")]
#[get("/api/v1/podcasts/<id>/episodes")]
pub async fn episodes(
    user: UserGuard,
    db: DbConn,
    id: i32,
) -> Result<Json<Vec<MediaItemSummary>>, Status> {
    let user = owner(&user)?;
    db.run(move |conn| podcasts::episodes(conn, user, id))
        .await
        .map(Json)
        .map_err(|error| {
            if matches!(error, diesel::result::Error::NotFound) {
                Status::NotFound
            } else {
                Status::InternalServerError
            }
        })
}

/// Account-owned podcast playback preferences.
#[derive(Deserialize, JsonSchema)]
pub struct PodcastPreferences {
    /// Oldest-first or newest-first chronological queue order.
    pub playback_order: podcasts::PlaybackOrder,
}

/// Save this account's order without changing another subscriber's queue.
#[openapi(tag = "Podcasts")]
#[put("/api/v1/podcasts/<id>/preferences", format = "json", data = "<body>")]
pub async fn preferences(
    user: UserGuard,
    db: DbConn,
    id: i32,
    body: Json<PodcastPreferences>,
) -> Result<Status, Status> {
    let user = owner(&user)?;
    let order = body.into_inner().playback_order;
    db.run(move |conn| podcasts::set_order(conn, user, id, order))
        .await
        .map_err(|error| {
            if matches!(error, diesel::result::Error::NotFound) {
                Status::NotFound
            } else {
                Status::InternalServerError
            }
        })?;
    Ok(Status::NoContent)
}
