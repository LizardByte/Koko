//! DVR storage configuration and recording permissions, checked against current database state.

use crate::db::schema::{
    app_settings as a,
    live_tv_recordings as r,
    live_tv_user_permissions as p,
    users as u,
};
use diesel::prelude::*;
use rocket_okapi::okapi::schemars::{
    self,
    JsonSchema,
};
use serde::{
    Deserialize,
    Serialize,
};
use std::path::{
    Path,
    PathBuf,
};

const KEY: &str = "live_tv";

/// Prepare a capture path on the blocking database worker, before committing a recording row.
pub(crate) fn prepare_capture_path(
    conn: &mut SqliteConnection,
    id: i32,
) -> Result<PathBuf, String> {
    let path = capture_path(conn, id)?;
    std::fs::create_dir_all(path.parent().ok_or("Invalid recording folder")?)
        .map_err(|_| "Unable to create the recording folder")?;
    Ok(path)
}

#[derive(Default, Deserialize, Serialize, JsonSchema)]
#[serde(default)]
pub(crate) struct DvrSettings {
    pub recording_dir: String,
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct RecordingPermission {
    pub user_id: i32,
    pub username: String,
    pub can_record: bool,
}

pub(crate) fn load(conn: &mut SqliteConnection) -> QueryResult<DvrSettings> {
    Ok(a::table
        .find(KEY)
        .select(a::value)
        .first::<String>(conn)
        .optional()?
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default())
}

pub(crate) fn save(
    conn: &mut SqliteConnection,
    settings: &DvrSettings,
) -> QueryResult<()> {
    diesel::insert_into(a::table)
        .values((
            a::key.eq(KEY),
            a::value.eq(serde_json::to_string(settings).unwrap()),
            a::updated_at.eq(super::now()),
        ))
        .on_conflict(a::key)
        .do_update()
        .set((
            a::value.eq(serde_json::to_string(settings).unwrap()),
            a::updated_at.eq(super::now()),
        ))
        .execute(conn)?;
    Ok(())
}

pub(crate) fn can_record(
    conn: &mut SqliteConnection,
    user: i32,
) -> QueryResult<bool> {
    // A missing override permits recording; removed accounts never acquire permissions.
    u::table.find(user).select(u::id).first::<i32>(conn)?;
    Ok(p::table
        .find(user)
        .select(p::can_record)
        .first::<bool>(conn)
        .optional()?
        .unwrap_or(true))
}

pub(crate) fn permissions(conn: &mut SqliteConnection) -> QueryResult<Vec<RecordingPermission>> {
    let accounts = u::table
        .select((u::id, u::username))
        .order(u::username)
        .load::<(i32, String)>(conn)?;
    accounts
        .into_iter()
        .map(|(user_id, username)| {
            Ok(RecordingPermission {
                user_id,
                username,
                can_record: can_record(conn, user_id)?,
            })
        })
        .collect()
}

pub(crate) fn set_permission(
    conn: &mut SqliteConnection,
    user: i32,
    allowed: bool,
) -> QueryResult<()> {
    conn.transaction(|conn| {
        u::table.find(user).select(u::id).first::<i32>(conn)?;
        diesel::insert_into(p::table)
            .values((p::user_id.eq(user), p::can_record.eq(allowed)))
            .on_conflict(p::user_id)
            .do_update()
            .set(p::can_record.eq(allowed))
            .execute(conn)?;
        if !allowed {
            use crate::db::schema::live_tv_series_rules as rules;
            diesel::update(
                r::table
                    .filter(r::user_id.eq(user))
                    .filter(r::state.eq("scheduled")),
            )
            .set(r::state.eq("cancelled"))
            .execute(conn)?;
            diesel::update(rules::table.filter(rules::user_id.eq(user)))
                .set(rules::enabled.eq(false))
                .execute(conn)?;
        }
        Ok(())
    })
}

pub(super) fn saved_path(
    id: i32,
    saved: &str,
) -> PathBuf {
    if Path::new(saved).is_absolute() {
        PathBuf::from(saved)
    } else {
        super::stream::recording_path(id)
    }
}

pub(super) fn capture_path(
    conn: &mut SqliteConnection,
    id: i32,
) -> Result<PathBuf, String> {
    if let Some(saved) = r::table
        .find(id)
        .select(r::file_name)
        .first::<Option<String>>(conn)
        .map_err(super::db_error)?
    {
        return Ok(saved_path(id, &saved));
    }
    let settings = load(conn).map_err(super::db_error)?;
    Ok(if settings.recording_dir.is_empty() {
        super::stream::recording_path(id)
    } else {
        Path::new(&settings.recording_dir).join(format!("recording-{id}.ts"))
    })
}

pub(crate) async fn validate_directory(directory: &str) -> Result<(), String> {
    if directory.is_empty() {
        return Ok(());
    }
    let path = Path::new(directory);
    if !path.is_absolute() {
        return Err("Choose an absolute DVR folder path on the server".into());
    }
    tokio::fs::create_dir_all(path)
        .await
        .map_err(|_| "Unable to create the DVR folder; check server permissions")?;
    let probe = path.join(format!(".koko-write-check-{:032x}", rand::random::<u128>()));
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .await
        .map_err(|_| "The server cannot write to this DVR folder")?;
    drop(file);
    tokio::fs::remove_file(probe)
        .await
        .map_err(|_| "The server cannot remove files in this DVR folder".into())
}
