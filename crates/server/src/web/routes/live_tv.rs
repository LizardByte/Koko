//! Authenticated Live TV guide, administrator setup, and shared or private recording routes.

use crate::{
    auth::{
        AdminGuard,
        UserGuard,
    },
    db::{
        DbConn,
        schema::{
            live_tv_channels as c,
            live_tv_recordings as r,
            live_tv_series_rules as rules,
            live_tv_sources as s,
        },
    },
    live_tv as tv,
};
use diesel::prelude::*;
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
use serde::{
    Deserialize,
    Serialize,
};

/// Readable Live TV operation failure.
#[derive(Serialize, JsonSchema)]
pub struct TvError {
    kind: &'static str,
    message: String,
}
type Error = rocket::response::status::Custom<Json<TvError>>;

/// Search OTA, cable, and satellite guide lineups by country and postal code.
#[openapi(tag = "Live TV")]
#[get("/api/v1/live-tv/guide/providers?<country>&<postal_code>")]
pub async fn guide_providers(
    _admin: AdminGuard,
    country: &str,
    postal_code: &str,
) -> Result<Json<Vec<tv::gracenote::Provider>>, Error> {
    tv::gracenote::providers(country, postal_code)
        .await
        .map(Json)
        .map_err(error)
}

/// Enumerate local driver-backed Windows BDA or Linux DVB tuners.
#[openapi(tag = "Live TV")]
#[get("/api/v1/live-tv/tuners")]
pub async fn discover_tuners(_admin: AdminGuard) -> Result<Json<Vec<tv::tsduck::Device>>, Error> {
    tv::tsduck::devices().await.map(Json).map_err(error)
}

/// Enumerate supported tuner frequency regions without requiring attached hardware.
#[openapi(tag = "Live TV")]
#[get("/api/v1/live-tv/tuners/regions")]
pub async fn tuner_regions(_admin: AdminGuard) -> Result<Json<Vec<String>>, Error> {
    tv::tsduck::regions().await.map(Json).map_err(error)
}

/// Search source-specific LAN advertisements from the Koko server.
#[openapi(tag = "Live TV")]
#[get("/api/v1/live-tv/discover/<kind>")]
pub async fn discover_sources(
    _admin: AdminGuard,
    kind: &str,
) -> Result<Json<Vec<tv::discovery::DiscoveredSource>>, Error> {
    tv::discovery::discover(kind).await.map(Json).map_err(error)
}

/// Queue a fresh local tuner channel scan, preserving existing channels and recording timers.
#[openapi(tag = "Live TV")]
#[post("/api/v1/live-tv/sources/<id>/scan")]
pub async fn scan_tuner(
    _admin: AdminGuard,
    db: DbConn,
    id: i32,
) -> Result<Status, Error> {
    tv::tsduck::rescan(&db, id).await.map_err(error)?;
    Ok(Status::Accepted)
}
fn error(message: String) -> Error {
    rocket::response::status::Custom(
        Status::BadRequest,
        Json(TvError {
            kind: "live_tv_operation",
            message,
        }),
    )
}
fn database(error_value: diesel::result::Error) -> Error {
    let status = if error_value == diesel::result::Error::NotFound {
        Status::NotFound
    } else {
        Status::InternalServerError
    };
    rocket::response::status::Custom(
        status,
        Json(TvError {
            kind: "live_tv_operation",
            message: "Live TV data could not be found or updated".into(),
        }),
    )
}
fn owner(user: &UserGuard) -> Result<i32, Error> {
    user.claims()
        .sub
        .parse()
        .map_err(|_| error("Sign in to use Live TV".into()))
}

/// Return enabled and available channels without their upstream stream credentials.
#[openapi(tag = "Live TV")]
#[get("/api/v1/live-tv/channels")]
pub async fn get_channels(
    _user: UserGuard,
    db: DbConn,
) -> Result<Json<Vec<tv::providers::PublicChannel>>, Error> {
    db.run(|conn| {
        let mut channels = tv::channels(conn, false)?;
        let sources = tv::guide_sources(conn)?;
        for channel in &mut channels {
            if sources
                .iter()
                .any(|s| s.id == channel.source_id && s.kind == "nextpvr")
            {
                channel.logo = Some(format!("/api/v1/live-tv/channels/{}/artwork", channel.id));
            }
        }
        tv::providers::decorate(conn, channels, false)
    })
    .await
    .map(Json)
    .map_err(database)
}

/// Return disabled and missing channels to administrators for configuration.
#[openapi(tag = "Live TV")]
#[get("/api/v1/live-tv/setup/channels")]
pub async fn get_setup_channels(
    _admin: AdminGuard,
    db: DbConn,
) -> Result<Json<Vec<tv::providers::PublicChannel>>, Error> {
    db.run(|conn| {
        let channels = tv::channels(conn, true)?;
        tv::providers::decorate(conn, channels, true)
    })
    .await
    .map(Json)
    .map_err(database)
}

/// Program guide and station identities used for channel mapping.
#[derive(Serialize, JsonSchema)]
pub struct GuideResponse {
    programs: Vec<tv::Program>,
    stations: Vec<tv::Station>,
    sources: Vec<tv::GuideSource>,
    preferences: tv::preferences::Preferences,
    source_groups: Vec<tv::setup::SourceGroup>,
    providers: Vec<tv::providers::ProviderSummary>,
}

/// Return a bounded UTC guide window for all enabled sources.
#[openapi(tag = "Live TV")]
#[get("/api/v1/live-tv/guide?<start>&<end>")]
pub async fn get_guide(
    user: UserGuard,
    db: DbConn,
    start: Option<i64>,
    end: Option<i64>,
) -> Result<Json<GuideResponse>, Error> {
    let user = owner(&user)?;
    let start = start.unwrap_or_else(tv::now);
    let end = end.unwrap_or_else(|| start.saturating_add(6 * 3600));
    if end <= start
        || end
            .checked_sub(start)
            .is_none_or(|length| length > 7 * 86400)
    {
        return Err(error(
            "Choose a guide window from one second to seven days".into(),
        ));
    }
    db.run(move |conn| {
        let sources = tv::guide_sources(conn)?;
        let configured_sources = tv::list_sources(conn)?;
        let nextpvr_artwork_sources: std::collections::HashSet<_> = configured_sources
            .iter()
            .filter(|source| tv::uses_nextpvr_artwork(source))
            .map(|source| source.id)
            .collect();
        let mut programs = tv::guide(conn, start, end)?;
        for program in &mut programs {
            if nextpvr_artwork_sources.contains(&program.source_id) {
                program.metadata.image.get_or_insert_with(|| {
                    format!("/api/v1/live-tv/programs/{}/artwork", program.id)
                });
                program.metadata.poster.get_or_insert_with(|| {
                    format!(
                        "/api/v1/live-tv/programs/{}/artwork?kind=poster",
                        program.id
                    )
                });
            }
        }
        tv::artwork::decorate(
            &mut programs,
            &configured_sources,
            &crate::config::current_settings().metadata,
        );
        Ok::<_, diesel::result::Error>(GuideResponse {
            programs,
            stations: tv::guide_stations(conn)?,
            sources,
            preferences: tv::preferences::load(conn, user)?,
            source_groups: tv::setup::groups(conn)?,
            providers: tv::providers::all_summaries(conn)?,
        })
    })
    .await
    .map(Json)
    .map_err(database)
}

/// Artwork is either a provider image redirect or an authenticated tuner image.
#[derive(rocket::Responder)]
pub enum ProgramArtwork {
    Remote(rocket::response::Redirect),
    Tuner((rocket::http::ContentType, Vec<u8>)),
}

/// Fetch program artwork without exposing provider credentials or the tuner server's PIN.
#[get("/api/v1/live-tv/programs/<id>/artwork?<kind>")]
pub async fn program_artwork(
    _user: UserGuard,
    db: DbConn,
    id: i32,
    kind: Option<&str>,
) -> Result<ProgramArtwork, Status> {
    let poster = kind == Some("poster");
    if let Some(url) = tv::artwork::image_url(&db, id, poster)
        .await
        .map_err(|_| Status::NotFound)?
    {
        return Ok(ProgramArtwork::Remote(
            rocket::response::Redirect::temporary(url),
        ));
    }
    artwork_response(tv::nextpvr_artwork(&db, id, poster).await).map(ProgramArtwork::Tuner)
}

/// Fetch a channel icon without exposing the tuner server's session or PIN.
#[get("/api/v1/live-tv/channels/<id>/artwork")]
pub async fn channel_artwork(
    _user: UserGuard,
    db: DbConn,
    id: i32,
) -> Result<(rocket::http::ContentType, Vec<u8>), Status> {
    artwork_response(tv::nextpvr_channel_artwork(&db, id).await)
}

fn artwork_response(
    value: Result<(String, Vec<u8>), String>
) -> Result<(rocket::http::ContentType, Vec<u8>), Status> {
    let (mime, bytes) = value.map_err(|_| Status::NotFound)?;
    let content_type = rocket::http::ContentType::parse_flexible(&mime).ok_or(Status::NotFound)?;
    Ok((content_type, bytes))
}

#[derive(Deserialize, JsonSchema)]
pub struct ChannelFavorite {
    favorite: bool,
}

/// Add or remove a channel from the signed-in account's favorites.
#[openapi(tag = "Live TV")]
#[put(
    "/api/v1/live-tv/channels/<id>/favorite",
    format = "json",
    data = "<body>"
)]
pub async fn favorite_channel(
    user: UserGuard,
    db: DbConn,
    id: i32,
    body: Json<ChannelFavorite>,
) -> Result<Status, Error> {
    let user = owner(&user)?;
    db.run(move |conn| tv::preferences::favorite(conn, user, id, body.favorite))
        .await
        .map_err(database)?;
    Ok(Status::NoContent)
}

#[derive(Deserialize, JsonSchema)]
pub struct BufferStart {
    channel_id: i32,
}
#[derive(Deserialize, JsonSchema)]
pub struct BufferKeep {
    #[serde(default)]
    private: bool,
}

/// Start an automatic temporary recording for seekable live TV playback.
#[openapi(tag = "Live TV")]
#[post("/api/v1/live-tv/sessions", format = "json", data = "<body>")]
pub async fn start_buffer(
    user: UserGuard,
    db: DbConn,
    buffer_database: &rocket::State<tv::timeshift::BufferDatabase>,
    body: Json<BufferStart>,
) -> Result<Json<tv::timeshift::BufferInfo>, Error> {
    let user = owner(&user)?;
    let info = tv::timeshift::start(&db, &buffer_database.0, user, body.channel_id)
        .await
        .map_err(error)?;
    let id = body.channel_id;
    db.run(move |conn| tv::preferences::played(conn, user, id))
        .await
        .map_err(database)?;
    Ok(Json(info))
}

/// Keep an active buffer alive while the full or minimized player is open.
#[openapi(tag = "Live TV")]
#[get("/api/v1/live-tv/sessions/<id>")]
pub async fn buffer_status(
    user: UserGuard,
    db: DbConn,
    id: &str,
) -> Result<Json<tv::timeshift::BufferInfo>, Error> {
    tv::timeshift::status(&db, id, owner(&user)?)
        .await
        .map(Json)
        .map_err(error)
}

/// Release a viewing session; a kept recording continues to its scheduled end.
#[openapi(tag = "Live TV")]
#[delete("/api/v1/live-tv/sessions/<id>")]
pub async fn close_buffer(
    user: UserGuard,
    id: &str,
) -> Result<Status, Error> {
    tv::timeshift::close(id, owner(&user)?)
        .await
        .map_err(error)?;
    Ok(Status::NoContent)
}

/// Save buffered content and the rest of the current show without another tuner.
#[openapi(tag = "Live TV")]
#[post("/api/v1/live-tv/sessions/<id>/keep", format = "json", data = "<body>")]
pub async fn keep_buffer(
    user: UserGuard,
    db: DbConn,
    id: &str,
    body: Json<BufferKeep>,
) -> Result<Json<i32>, Error> {
    tv::timeshift::keep(&db, id, owner(&user)?, body.private)
        .await
        .map(Json)
        .map_err(error)
}

/// Play captured TV from a buffer position, independently of the ongoing recording.
#[get("/api/v1/live-tv/sessions/<id>/stream?<start_ms>&<quality>")]
pub async fn stream_buffer(
    user: UserGuard,
    id: &str,
    start_ms: Option<u64>,
    quality: Option<&str>,
) -> Result<tv::LiveStream, Error> {
    tv::timeshift::play(id, owner(&user)?, start_ms, quality.unwrap_or("original"))
        .await
        .map_err(error)
}

/// Return administrator source health without credentials or upstream addresses.
#[openapi(tag = "Live TV")]
#[get("/api/v1/live-tv/sources")]
pub async fn get_sources(
    _admin: AdminGuard,
    db: DbConn,
) -> Result<Json<Vec<tv::setup::SourceSummary>>, Error> {
    db.run(tv::setup::sources).await.map(Json).map_err(database)
}

/// Read administrator connection fields, retaining a masked PIN on subsequent edits.
#[openapi(tag = "Live TV")]
#[get("/api/v1/live-tv/sources/<id>/connection")]
pub async fn get_source_connection(
    _admin: AdminGuard,
    db: DbConn,
    id: i32,
) -> Result<Json<tv::SourceConnection>, Error> {
    tv::connection(&db, id).await.map(Json).map_err(error)
}

/// Read shared guide source groups without exposing connection credentials.
#[openapi(tag = "Live TV")]
#[get("/api/v1/live-tv/source-groups")]
pub async fn get_source_groups(
    _admin: AdminGuard,
    db: DbConn,
) -> Result<Json<Vec<tv::setup::SourceGroup>>, Error> {
    db.run(tv::setup::groups).await.map(Json).map_err(database)
}

/// Group independent source connections into shared guide lineups.
#[openapi(tag = "Live TV")]
#[put("/api/v1/live-tv/source-groups", format = "json", data = "<body>")]
pub async fn set_source_groups(
    _admin: AdminGuard,
    db: DbConn,
    body: Json<Vec<tv::setup::SourceGroup>>,
) -> Result<Status, Error> {
    db.run(move |conn| tv::setup::save_groups(conn, body.into_inner()))
        .await
        .map_err(error)?;
    Ok(Status::NoContent)
}

/// Source creation result; refresh errors preserve the configured source for correction.
#[derive(Serialize, JsonSchema)]
pub struct SourceCreated {
    id: i32,
    refresh_error: Option<String>,
}

/// Configure a tuner or playlist and queue channel/guide refresh in the background.
#[openapi(tag = "Live TV")]
#[post("/api/v1/live-tv/sources", format = "json", data = "<body>")]
pub async fn add_source(
    _admin: AdminGuard,
    db: DbConn,
    body: Json<tv::SourceInput>,
) -> Result<Json<SourceCreated>, Error> {
    let id = tv::save_source(&db, body.into_inner())
        .await
        .map_err(error)?;
    Ok(Json(SourceCreated {
        id,
        refresh_error: None,
    }))
}

/// Remove a connection and its future timers while keeping saved DVR recordings and files.
#[openapi(tag = "Live TV")]
#[delete("/api/v1/live-tv/sources/<id>")]
pub async fn remove_source(
    _admin: AdminGuard,
    db: DbConn,
    id: i32,
) -> Result<Status, Error> {
    tv::removal::remove(&db, id)
        .await
        .map_err(|failure| match failure {
            tv::removal::RemoveError::Database(value) => database(value),
            tv::removal::RemoveError::Busy => rocket::response::status::Custom(
                Status::Conflict,
                Json(TvError {
                    kind: "live_tv_operation",
                    message: "Stop playback and active recordings from this source before \
                              removing it"
                        .into(),
                }),
            ),
        })?;
    Ok(Status::NoContent)
}

/// Replace connection settings without discarding channel IDs, mappings, or DVR schedules.
#[openapi(tag = "Live TV")]
#[put(
    "/api/v1/live-tv/sources/<id>/connection",
    format = "json",
    data = "<body>"
)]
pub async fn replace_source(
    _admin: AdminGuard,
    db: DbConn,
    id: i32,
    body: Json<tv::SourceInput>,
) -> Result<Status, Error> {
    tv::replace_source(&db, id, body.into_inner())
        .await
        .map_err(error)?;
    Ok(Status::NoContent)
}

/// Queue channel and guide refresh without making the browser wait for a tuner scan.
#[openapi(tag = "Live TV")]
#[put(
    "/api/v1/live-tv/sources/<id>/providers/<provider>",
    format = "json",
    data = "<body>"
)]
pub async fn set_provider(
    _admin: AdminGuard,
    db: DbConn,
    id: i32,
    provider: String,
    body: Json<tv::providers::ProviderUpdate>,
) -> Result<Status, Error> {
    db.run(move |conn| tv::providers::update(conn, id, &provider, body.into_inner()))
        .await
        .map_err(error)?;
    Ok(Status::NoContent)
}

/// Upload an image for a source or provider; binary images are decoded and resized before storage.
#[put("/api/v1/live-tv/sources/<id>/image?<provider>", data = "<data>")]
pub async fn upload_source_image(
    _admin: AdminGuard,
    db: DbConn,
    id: i32,
    provider: Option<String>,
    data: rocket::Data<'_>,
) -> Result<Status, Error> {
    use rocket::data::ToByteUnit;
    let bytes = data
        .open(5.mebibytes())
        .into_bytes()
        .await
        .map_err(|_| error("Unable to read the image".into()))?;
    if !bytes.is_complete() {
        return Err(error("Choose an image smaller than 5 MiB".into()));
    }
    let bytes = bytes.into_inner();
    let image = rocket::tokio::task::spawn_blocking(move || tv::providers::prepare_image(&bytes))
        .await
        .map_err(|_| error("Unable to process the image".into()))?
        .map_err(error)?;
    db.run(move |conn| tv::providers::set_image(conn, id, provider.as_deref(), Some(image)))
        .await
        .map_err(error)?;
    Ok(Status::NoContent)
}

/// Restore the bundled icon for a source or provider.
#[delete("/api/v1/live-tv/sources/<id>/image?<provider>")]
pub async fn remove_source_image(
    _admin: AdminGuard,
    db: DbConn,
    id: i32,
    provider: Option<String>,
) -> Result<Status, Error> {
    db.run(move |conn| tv::providers::set_image(conn, id, provider.as_deref(), None))
        .await
        .map_err(error)?;
    Ok(Status::NoContent)
}

/// Read normalized source/provider artwork using the same authenticated image flow as channel
/// icons.
#[get("/api/v1/live-tv/sources/<id>/image?<provider>")]
pub async fn source_image(
    _user: UserGuard,
    db: DbConn,
    id: i32,
    provider: Option<String>,
) -> Result<(rocket::http::ContentType, Vec<u8>), Status> {
    db.run(move |conn| tv::providers::image(conn, id, provider.as_deref()))
        .await
        .map(|data| (rocket::http::ContentType::PNG, data))
        .map_err(|_| Status::NotFound)
}

/// Queue channel and guide refresh without making the browser wait for a tuner scan.
#[openapi(tag = "Live TV")]
#[post("/api/v1/live-tv/sources/<id>/refresh")]
pub async fn refresh_source(
    _admin: AdminGuard,
    db: DbConn,
    id: i32,
) -> Result<Status, Error> {
    let updated = db
        .run(move |conn| {
            diesel::update(s::table.find(id))
                .set(s::next_refresh.eq(0_i64))
                .execute(conn)
        })
        .await
        .map_err(database)?;
    if updated == 0 {
        return Err(database(diesel::result::Error::NotFound));
    }
    Ok(Status::Accepted)
}

/// Source availability and simultaneous stream limit.
#[derive(Deserialize, JsonSchema)]
pub struct SourceUpdate {
    enabled: bool,
    capacity: i32,
}

/// Disable a source without discarding recording history, or change its stream limit.
#[openapi(tag = "Live TV")]
#[put("/api/v1/live-tv/sources/<id>", format = "json", data = "<body>")]
pub async fn set_source(
    _admin: AdminGuard,
    db: DbConn,
    id: i32,
    body: Json<SourceUpdate>,
) -> Result<Status, Error> {
    let mut body = body.into_inner();
    if !(1..=64).contains(&body.capacity) {
        return Err(error("The stream limit must be between 1 and 64".into()));
    }
    let kind = db
        .run(move |conn| {
            s::table
                .find(id)
                .filter(s::kind.ne("removed"))
                .select(s::kind)
                .first::<String>(conn)
        })
        .await
        .map_err(database)?;
    if matches!(kind.as_str(), "dvb" | "native") {
        body.capacity = 1;
    }
    let count = db
        .run(move |conn| {
            diesel::update(s::table.find(id).filter(s::kind.ne("removed")))
                .set((s::enabled.eq(body.enabled), s::capacity.eq(body.capacity)))
                .execute(conn)
        })
        .await
        .map_err(database)?;
    if count == 0 {
        return Err(database(diesel::result::Error::NotFound));
    }
    Ok(Status::NoContent)
}

/// Explicit per-channel guide mapping; null restores automatic mapping.
#[derive(Deserialize, JsonSchema)]
pub struct ChannelUpdate {
    enabled: bool,
    guide_override: Option<String>,
}

/// Save channel visibility and an explicit guide station assignment.
#[openapi(tag = "Live TV")]
#[put("/api/v1/live-tv/channels/<id>", format = "json", data = "<body>")]
pub async fn set_channel(
    _admin: AdminGuard,
    db: DbConn,
    id: i32,
    body: Json<ChannelUpdate>,
) -> Result<Status, Error> {
    let body = body.into_inner();
    if body.guide_override.as_ref().is_some_and(|s| s.len() > 500) {
        return Err(error("The guide channel ID is too long".into()));
    }
    db.run(move |conn| {
        diesel::update(c::table.find(id))
            .set((
                c::enabled.eq(body.enabled),
                c::guide_override.eq(body.guide_override.filter(|s| !s.trim().is_empty())),
            ))
            .execute(conn)
    })
    .await
    .map_err(database)?;
    Ok(Status::NoContent)
}

/// The current account's DVR schedules, saved recordings, and conflict indicators.
#[derive(Serialize, JsonSchema)]
pub struct RecordingsResponse {
    recordings: Vec<tv::Recording>,
    conflicts: Vec<i32>,
    can_record: bool,
}

/// Return shared recordings and this account's private recordings and conflict indicators.
#[openapi(tag = "Live TV")]
#[get("/api/v1/live-tv/recordings")]
pub async fn get_recordings(
    user: UserGuard,
    db: DbConn,
) -> Result<Json<RecordingsResponse>, Error> {
    let user = owner(&user)?;
    db.run(move |conn| {
        Ok::<_, diesel::result::Error>(RecordingsResponse {
            recordings: tv::recordings(conn, user)?,
            conflicts: tv::recording_conflicts(conn, user)?,
            can_record: tv::settings::can_record(conn, user)?,
        })
    })
    .await
    .map(Json)
    .map_err(database)
}

/// Storage and per-account recording permissions visible only to administrators.
#[derive(Serialize, JsonSchema)]
pub struct DvrAdminSettings {
    settings: tv::settings::DvrSettings,
    users: Vec<tv::settings::RecordingPermission>,
}

/// Read DVR storage and account recording permissions.
#[openapi(tag = "Live TV")]
#[get("/api/v1/live-tv/dvr/settings")]
pub async fn get_dvr_settings(
    _admin: AdminGuard,
    db: DbConn,
) -> Result<Json<DvrAdminSettings>, Error> {
    db.run(|conn| {
        Ok::<_, diesel::result::Error>(DvrAdminSettings {
            settings: tv::settings::load(conn)?,
            users: tv::settings::permissions(conn)?,
        })
    })
    .await
    .map(Json)
    .map_err(database)
}

/// Set the folder used for new captures; existing recordings retain their saved locations.
#[openapi(tag = "Live TV")]
#[put("/api/v1/live-tv/dvr/settings", format = "json", data = "<body>")]
pub async fn set_dvr_settings(
    _admin: AdminGuard,
    db: DbConn,
    body: Json<tv::settings::DvrSettings>,
) -> Result<Status, Error> {
    let mut settings = body.into_inner();
    settings.recording_dir = settings.recording_dir.trim().to_owned();
    tv::settings::validate_directory(&settings.recording_dir)
        .await
        .map_err(error)?;
    db.run(move |conn| tv::settings::save(conn, &settings))
        .await
        .map_err(database)?;
    Ok(Status::NoContent)
}

/// Administrator override for one account's DVR scheduling permission.
#[derive(Deserialize, JsonSchema)]
pub struct PermissionUpdate {
    can_record: bool,
}

/// Restrict scheduling and stop future series timers while leaving running captures intact.
#[openapi(tag = "Live TV")]
#[put(
    "/api/v1/live-tv/dvr/permissions/<id>",
    format = "json",
    data = "<body>"
)]
pub async fn set_recording_permission(
    _admin: AdminGuard,
    db: DbConn,
    id: i32,
    body: Json<PermissionUpdate>,
) -> Result<Status, Error> {
    db.run(move |conn| tv::settings::set_permission(conn, id, body.can_record))
        .await
        .map_err(database)?;
    Ok(Status::NoContent)
}

/// Schedule a program, a manually timed recording, or a recurring series.
#[openapi(tag = "Live TV")]
#[post("/api/v1/live-tv/recordings", format = "json", data = "<body>")]
pub async fn schedule_recording(
    user: UserGuard,
    db: DbConn,
    body: Json<tv::ScheduleRequest>,
) -> Result<Status, Error> {
    let user = owner(&user)?;
    db.run(move |conn| tv::schedule(conn, user, body.into_inner()))
        .await
        .map_err(error)?;
    Ok(Status::Created)
}

/// Cancel this account's timer or stop capture while retaining any recorded portion.
#[openapi(tag = "Live TV")]
#[post("/api/v1/live-tv/recordings/<id>/cancel")]
pub async fn cancel_recording(
    user: UserGuard,
    db: DbConn,
    id: i32,
) -> Result<Status, Error> {
    let user = owner(&user)?;
    db.run(move |conn| {
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            diesel::update(
                r::table
                    .find(id)
                    .filter(r::user_id.eq(user))
                    .filter(r::state.eq("recording")),
            )
            .set(r::state.eq("stopping"))
            .execute(conn)?;
            diesel::update(
                r::table
                    .find(id)
                    .filter(r::user_id.eq(user))
                    .filter(r::state.eq("scheduled")),
            )
            .set(r::state.eq("cancelled"))
            .execute(conn)
        })
    })
    .await
    .map_err(database)?;
    Ok(Status::NoContent)
}

/// Remove a saved recording belonging to the current account after it has stopped.
#[openapi(tag = "Live TV")]
#[delete("/api/v1/live-tv/recordings/<id>")]
pub async fn delete_recording(
    user: UserGuard,
    db: DbConn,
    id: i32,
) -> Result<Status, Error> {
    let user = owner(&user)?;
    let previous = db
        .run(move |conn| tv::claim_deletion(conn, user, id))
        .await
        .map_err(database)?;
    let Some(previous) = previous else {
        return Err(error("Stop this recording before deleting it".into()));
    };
    if let Err(message) = tv::remove_recording_file(&db, id).await {
        db.run(move |conn| {
            diesel::update(
                r::table
                    .find(id)
                    .filter(r::user_id.eq(user))
                    .filter(r::state.eq("deleting")),
            )
            .set(r::state.eq(previous))
            .execute(conn)
        })
        .await
        .map_err(database)?;
        return Err(error(message));
    }
    db.run(move |conn| {
        diesel::delete(
            r::table
                .find(id)
                .filter(r::user_id.eq(user))
                .filter(r::state.eq("deleting")),
        )
        .execute(conn)
    })
    .await
    .map_err(database)?;
    Ok(Status::NoContent)
}

/// Return shared series rules and this account's private recurring rules.
#[openapi(tag = "Live TV")]
#[get("/api/v1/live-tv/rules")]
pub async fn get_rules(
    user: UserGuard,
    db: DbConn,
) -> Result<Json<Vec<tv::SeriesRule>>, Error> {
    let user = owner(&user)?;
    db.run(move |conn| tv::rules(conn, user))
        .await
        .map(Json)
        .map_err(database)
}

/// Stop future series reservations while preserving completed and active recordings.
#[openapi(tag = "Live TV")]
#[delete("/api/v1/live-tv/rules/<id>")]
pub async fn delete_rule(
    user: UserGuard,
    db: DbConn,
    id: i32,
) -> Result<Status, Error> {
    let user = owner(&user)?;
    db.run(move |conn| {
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            diesel::update(
                r::table
                    .filter(r::user_id.eq(user))
                    .filter(r::rule_id.eq(id))
                    .filter(r::state.eq("scheduled")),
            )
            .set(r::state.eq("cancelled"))
            .execute(conn)?;
            diesel::delete(rules::table.find(id).filter(rules::user_id.eq(user))).execute(conn)
        })
    })
    .await
    .map_err(database)?;
    Ok(Status::NoContent)
}

/// Convert and stream an authenticated live channel through Koko.
#[get("/api/v1/live-tv/channels/<id>/stream")]
pub async fn stream_channel(
    user: UserGuard,
    db: DbConn,
    id: i32,
) -> Result<tv::LiveStream, Error> {
    let user = owner(&user)?;
    let stream = tv::play_channel(&db, id).await.map_err(error)?;
    db.run(move |conn| tv::preferences::played(conn, user, id))
        .await
        .map_err(database)?;
    Ok(stream)
}

/// Play a shared recording or one owned by the signed-in account.
#[get("/api/v1/live-tv/recordings/<id>/stream")]
pub async fn stream_recording(
    user: UserGuard,
    db: DbConn,
    id: i32,
) -> Result<tv::LiveStream, Error> {
    tv::play_recording(&db, id, owner(&user)?)
        .await
        .map_err(error)
}

/// Import a user-supplied XMLTV document without requiring a remotely hosted guide.
#[post("/api/v1/live-tv/sources/<id>/xmltv", data = "<data>")]
pub async fn upload_guide(
    _admin: AdminGuard,
    db: DbConn,
    id: i32,
    data: rocket::Data<'_>,
) -> Result<Status, Error> {
    use rocket::data::ToByteUnit;
    use tokio::io::AsyncWriteExt;
    let mut file = tv::feeds::temporary().map_err(error)?;
    tokio::io::copy(&mut data.open(u64::MAX.bytes()), &mut file)
        .await
        .map_err(|_| error("Unable to read the XMLTV upload".into()))?;
    file.flush()
        .await
        .map_err(|_| error("Unable to save the XMLTV upload; check temporary disk space".into()))?;
    tv::import_guide(&db, id, file.into_std().await)
        .await
        .map_err(error)?;
    Ok(Status::NoContent)
}
