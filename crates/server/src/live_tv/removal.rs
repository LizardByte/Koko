//! Remove connections while retaining the channel references needed by saved DVR history.
use super::{
    sources,
    storage,
    stream,
};
use crate::db::{
    DbConn,
    schema::{
        app_settings as a,
        live_tv_channels as c,
        live_tv_programs as p,
        live_tv_recordings as r,
        live_tv_series_rules as rules,
        live_tv_sources as s,
        live_tv_stations as stations,
    },
};
use diesel::prelude::*;

/// Archived connections retain DVR foreign keys but cannot be configured or used again.
pub(super) const REMOVED: &str = "removed";

pub(crate) enum RemoveError {
    Busy,
    Database(diesel::result::Error),
}
impl From<diesel::result::Error> for RemoveError {
    fn from(error: diesel::result::Error) -> Self {
        Self::Database(error)
    }
}

pub(crate) async fn remove(
    db: &DbConn,
    id: i32,
) -> Result<(), RemoveError> {
    let setup = sources::source_lock(0);
    let _setup_guard = setup.lock().await;
    let lock = sources::source_lock(id);
    let _guard = lock.lock().await;
    let _admission = stream::ADMISSIONS.lock().await;
    let previous = db.run(move |conn| remove_connection(conn, id)).await?;
    if !previous.starts_with('{') {
        let _ = crate::secrets::delete_secret(&previous);
    }
    Ok(())
}

fn remove_connection(
    conn: &mut SqliteConnection,
    id: i32,
) -> Result<String, RemoveError> {
    conn.immediate_transaction(|conn| {
        let previous = storage::source(conn, id)?;
        let channel_ids = c::table.filter(c::source_id.eq(id)).select(c::id);
        let recording = r::table
            .filter(r::channel_id.eq_any(channel_ids))
            .filter(r::state.eq_any(["recording", "stopping"]))
            .count()
            .get_result::<i64>(conn)?;
        if stream::source_in_use(id) || recording > 0 {
            return Err(RemoveError::Busy);
        }
        diesel::delete(
            r::table
                .filter(r::channel_id.eq_any(channel_ids))
                .filter(r::state.eq("scheduled"))
                .filter(r::file_name.is_null()),
        )
        .execute(conn)?;
        diesel::update(
            r::table
                .filter(r::channel_id.eq_any(channel_ids))
                .filter(r::state.eq("scheduled")),
        )
        .set((r::state.eq("cancelled"), r::last_error.eq(None::<String>)))
        .execute(conn)?;
        diesel::delete(rules::table.filter(rules::channel_id.eq_any(channel_ids))).execute(conn)?;
        diesel::delete(p::table.filter(p::source_id.eq(id))).execute(conn)?;
        diesel::delete(stations::table.filter(stations::source_id.eq(id))).execute(conn)?;
        diesel::delete(
            c::table
                .filter(c::source_id.eq(id))
                .filter(c::id.ne_all(r::table.select(r::channel_id))),
        )
        .execute(conn)?;
        let retained = c::table
            .filter(c::source_id.eq(id))
            .count()
            .get_result::<i64>(conn)?;
        if retained == 0 {
            diesel::delete(s::table.find(id)).execute(conn)?;
        } else {
            // Saved recordings use their stored file paths, never these upstream credentials.
            diesel::update(c::table.filter(c::source_id.eq(id)))
                .set((
                    c::enabled.eq(false),
                    c::available.eq(false),
                    c::stream_url.eq(""),
                    c::logo.eq(None::<String>),
                    c::guide_key.eq(""),
                    c::guide_override.eq(None::<String>),
                ))
                .execute(conn)?;
            diesel::update(s::table.find(id))
                .set((
                    s::kind.eq(REMOVED),
                    s::enabled.eq(false),
                    s::config_json.eq("{}"),
                    s::next_refresh.eq(i64::MAX),
                    s::last_error.eq(None::<String>),
                ))
                .execute(conn)?;
        }
        let mut groups = super::setup::groups(conn)?;
        for group in &mut groups {
            group.source_ids.retain(|source| *source != id);
        }
        groups.retain(|group| group.source_ids.len() >= 2);
        super::setup::save_groups(conn, groups)
            .map_err(|_| diesel::result::Error::RollbackTransaction)?;
        let keys = [
            format!("live_tv_metadata:{id}"),
            format!("live_tv_feed_sizes:{id}"),
        ];
        diesel::delete(
            a::table.filter(
                a::key.eq_any(keys).or(a::key
                    .like(format!("live\\_tv\\_image:{id}:%"))
                    .escape('\\')),
            ),
        )
        .execute(conn)?;
        Ok(previous.config_json)
    })
}

#[cfg(test)]
mod tests;
