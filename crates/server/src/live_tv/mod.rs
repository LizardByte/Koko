//! Live television sources, guide data, and shared or private DVR schedules.

pub(crate) mod artwork;
mod broadcast;
mod buffer_files;
pub(crate) mod discovery;
mod dvr;
pub(crate) mod feeds;
mod formats;
pub(crate) mod gracenote;
mod hls;
pub(crate) mod iptv;
mod native;
pub(crate) mod preferences;
pub(crate) mod providers;
mod proxy;
pub(crate) mod removal;
mod retention;
pub(crate) mod settings;
pub(crate) mod setup;
mod sources;
mod storage;
mod stream;
#[cfg(test)]
mod tests;
pub(crate) mod timeshift;
pub(crate) mod tsduck;

pub(crate) use dvr::start_workers;
pub(crate) use formats::{
    Channel,
    Program,
    Station,
};
pub(crate) use sources::{
    SourceConfig,
    SourceInput,
};
pub(crate) use storage::{
    GuideSource,
    Recording,
    SeriesRule,
    Source,
};
pub(crate) use stream::LiveStream;

pub(crate) use dvr::conflicts as recording_conflicts;
pub(crate) use dvr::{
    ScheduleRequest,
    schedule,
};
pub(crate) use sources::{
    SourceConnection,
    connection,
};
pub(crate) use sources::{
    nextpvr_artwork,
    nextpvr_channel_artwork,
    refresh,
    replace_source,
    save_source,
    uses_nextpvr_artwork,
};
pub(crate) use storage::claim_deletion;
pub(crate) use storage::{
    channels,
    guide,
    guide_sources,
    guide_stations,
    recordings,
    rules,
    sources as list_sources,
};
pub(crate) use stream::{
    play_channel,
    play_recording,
};

pub(crate) async fn remove_recording_file(
    db: &crate::db::DbConn,
    id: i32,
) -> Result<(), String> {
    let path = db.run(move |conn| settings::capture_path(conn, id)).await?;
    for attempt in 0..20 {
        match tokio::fs::remove_file(&path).await {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e)
                if attempt < 19
                    && (e.kind() == std::io::ErrorKind::PermissionDenied
                        || (cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33)))) =>
            {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            Err(_) => {
                return Err(
                    "Close recording playback before deleting it, and check file permissions"
                        .into(),
                );
            }
        }
    }
    Err("The recording file could not be deleted".into())
}

pub(crate) async fn import_guide(
    db: &crate::db::DbConn,
    id: i32,
    file: std::fs::File,
) -> Result<(), String> {
    let lock = sources::source_lock(id);
    let _guard = lock.lock().await;
    let file = tokio::fs::File::from_std(file);
    let guide_bytes = file
        .metadata()
        .await
        .map_err(|_| "Unable to read the XMLTV file size")?
        .len();
    let guide = feeds::read_guide(file.into_std().await).await?;
    let source = db
        .run(move |conn| storage::source(conn, id))
        .await
        .map_err(db_error)?;
    let mut config = sources::config(&source)?;
    config.guide_kind = "upload".into();
    let encoded =
        serde_json::to_string(&config).map_err(|_| "Unable to save the guide settings")?;
    db.run(move |conn| {
        storage::source(conn, id)?;
        storage::save_guide(conn, id, &guide)?;
        let mut sizes = setup::feed_sizes(conn, id)?;
        sizes.guide_bytes = Some(guide_bytes);
        setup::save_feed_sizes(conn, id, &sizes)?;
        use crate::db::schema::live_tv_sources as s;
        use diesel::prelude::*;
        diesel::update(s::table.find(id))
            .set((s::last_error.eq(None::<String>), s::config_json.eq(encoded)))
            .execute(conn)?;
        Ok(())
    })
    .await
    .map_err(db_error)
}

pub(crate) fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

pub(crate) fn db_error(_: diesel::result::Error) -> String {
    "Unable to access Live TV data".into()
}
