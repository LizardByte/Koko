//! Authenticated ebook content and per-user reading state, independent of audio playback.

use super::media::{
    RangeHeader,
    RangedFile,
    open_ranged_file,
    require_item_access,
};
use crate::{
    auth::UserGuard,
    db::DbConn,
    media::books::{
        self,
        ReaderDocument,
        ReadingProgress,
    },
};
use rocket::{
    Request,
    get,
    http::{
        ContentType,
        Header,
        Status,
    },
    put,
    response::{
        self,
        Responder,
    },
    serde::json::Json,
};
use rocket_okapi::openapi;

/// Raw documents are downloads, so opening an HTML book URL cannot execute with Koko's origin.
pub struct ReaderContent(RangedFile);

impl<'r> Responder<'r, 'static> for ReaderContent {
    fn respond_to(
        self,
        request: &'r Request<'_>,
    ) -> response::Result<'static> {
        let mut response = self.0.respond_to(request)?;
        response.set_header(ContentType::Binary);
        response.set_header(Header::new("Content-Disposition", "attachment"));
        response.set_header(Header::new(
            "Content-Security-Policy",
            "sandbox; default-src 'none'",
        ));
        response.set_header(Header::new("X-Content-Type-Options", "nosniff"));
        Ok(response)
    }
}

async fn source(
    db: &DbConn,
    item: i32,
    user: &UserGuard,
) -> Result<books::BookSource, Status> {
    require_item_access(db, item, user).await?;
    let user_id = user
        .claims()
        .sub
        .parse::<i32>()
        .map_err(|_| Status::Unauthorized)?;
    db.run(move |conn| books::source(conn, item, user_id))
        .await
        .map_err(|_| Status::InternalServerError)?
        .ok_or(Status::NotFound)
}

/// Return the book's available rendering format and this user's saved reading position.
#[openapi(tag = "Books")]
#[get("/api/v1/items/<item_id>/reader")]
pub async fn document(
    user: UserGuard,
    db: DbConn,
    item_id: i32,
) -> Result<Json<ReaderDocument>, Status> {
    Ok(Json(source(&db, item_id, &user).await?.document))
}

/// Serve an authorized ebook, converting legacy formats into an isolated cache when needed.
#[get("/api/v1/items/<item_id>/reader/content")]
pub async fn content(
    user: UserGuard,
    db: DbConn,
    range: RangeHeader,
    item_id: i32,
) -> Result<ReaderContent, (Status, Json<serde_json::Value>)> {
    let source = source(&db, item_id, &user).await.map_err(|status| {
        (
            status,
            Json(serde_json::json!({"message": status.reason().unwrap_or("Book unavailable")})),
        )
    })?;
    let path = books::content(source, &crate::config::current_settings())
        .await
        .map_err(|error| {
            (
                Status::UnprocessableEntity,
                Json(serde_json::json!({"message": error})),
            )
        })?;
    open_ranged_file(path, &range)
        .await
        .map(ReaderContent)
        .map_err(|status| {
            (
                status,
                Json(serde_json::json!({"message": "Book content unavailable"})),
            )
        })
}

/// Save this account's position and bookmarks without changing listening history.
#[openapi(tag = "Books")]
#[put(
    "/api/v1/items/<item_id>/reader/progress",
    format = "json",
    data = "<state>"
)]
pub async fn progress(
    user: UserGuard,
    db: DbConn,
    item_id: i32,
    state: Json<ReadingProgress>,
) -> Result<Status, Status> {
    let source = source(&db, item_id, &user).await?;
    if !state.valid() {
        return Err(Status::BadRequest);
    }
    if source.document.revision != state.revision {
        return Err(Status::Conflict);
    }
    let user_id = user
        .claims()
        .sub
        .parse::<i32>()
        .map_err(|_| Status::Unauthorized)?;
    db.run(move |conn| books::save_progress(conn, user_id, item_id, &state.into_inner()))
        .await
        .map_err(|_| Status::InternalServerError)?;
    Ok(Status::NoContent)
}
