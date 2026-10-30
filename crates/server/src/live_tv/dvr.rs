//! Durable single and series timers, tuner conflicts, and restart-safe recording capture.

use super::{
    Program,
    Recording,
    SeriesRule,
    storage,
    stream,
};
use crate::db::{
    DbConn,
    schema::{
        live_tv_channels as channels,
        live_tv_recordings as r,
        live_tv_series_rules as rules,
        live_tv_sources as sources,
    },
};
use diesel::prelude::*;
use rocket_okapi::okapi::schemars::{
    self,
    JsonSchema,
};
use serde::Deserialize;
use std::{
    sync::Arc,
    time::Duration,
};
use tokio::io::{
    AsyncReadExt,
    AsyncWriteExt,
};

#[derive(Deserialize, JsonSchema)]
pub(crate) struct ScheduleRequest {
    pub channel_id: i32,
    pub starts_at: i64,
    pub ends_at: i64,
    pub title: String,
    #[serde(default)]
    pub series: bool,
    #[serde(default)]
    pub new_only: bool,
    #[serde(default)]
    pub padding_before: i32,
    #[serde(default)]
    pub padding_after: i32,
    /// Share this recording and future series episodes with all accounts unless private is
    /// selected.
    #[serde(default)]
    pub private: bool,
}

fn valid(request: &ScheduleRequest) -> bool {
    request.ends_at > super::now()
        && chrono::DateTime::from_timestamp(request.starts_at, 0).is_some()
        && chrono::DateTime::from_timestamp(request.ends_at, 0).is_some()
        && request.ends_at > request.starts_at
        && request
            .ends_at
            .checked_sub(request.starts_at)
            .is_some_and(|length| length <= 86400)
        && !request.title.trim().is_empty()
        && request.title.len() <= 500
        && (0..=3600).contains(&request.padding_before)
        && (0..=3600).contains(&request.padding_after)
}

pub(crate) fn schedule(
    conn: &mut SqliteConnection,
    user: i32,
    request: ScheduleRequest,
) -> Result<(), String> {
    if !super::settings::can_record(conn, user).map_err(super::db_error)? {
        return Err("Your administrator has disabled recording for this account".into());
    }
    if !valid(&request) {
        return Err(
            "Enter a title, a valid future time range of at most 24 hours, and padding from 0 to \
             3,600 seconds"
                .into(),
        );
    }
    let channel = storage::channel(conn, request.channel_id).map_err(super::db_error)?;
    if !channel.enabled || !channel.available {
        return Err("This channel is unavailable".into());
    }
    let source = storage::source(conn, channel.source_id).map_err(super::db_error)?;
    if !source.enabled {
        return Err("This source is disabled".into());
    }
    let program = storage::guide(conn, request.starts_at, request.ends_at)
        .map_err(super::db_error)?
        .into_iter()
        .find(|p| {
            p.source_id == channel.source_id
                && p.guide_key
                    == channel
                        .guide_override
                        .as_deref()
                        .unwrap_or(&channel.guide_key)
                && p.starts_at == request.starts_at
        });
    if request.series && program.is_none() {
        return Err("Choose a program with guide data to create a series rule".into());
    }
    conn.transaction::<_, diesel::result::Error, _>(|conn| {
        let mut rule_id = None;
        if request.series {
            let title = &program.as_ref().unwrap().title;
            diesel::insert_into(rules::table)
                .values((
                    rules::user_id.eq(user),
                    rules::channel_id.eq(channel.id),
                    rules::title.eq(title),
                    rules::new_only.eq(request.new_only),
                    rules::padding_before.eq(request.padding_before),
                    rules::padding_after.eq(request.padding_after),
                    rules::shared.eq(!request.private),
                ))
                .on_conflict((rules::user_id, rules::channel_id, rules::title))
                .do_update()
                .set((
                    rules::enabled.eq(true),
                    rules::new_only.eq(request.new_only),
                    rules::padding_before.eq(request.padding_before),
                    rules::padding_after.eq(request.padding_after),
                    rules::shared.eq(!request.private),
                ))
                .execute(conn)?;
            rule_id = Some(
                rules::table
                    .filter(rules::user_id.eq(user))
                    .filter(rules::channel_id.eq(channel.id))
                    .filter(rules::title.eq(title))
                    .select(rules::id)
                    .first::<i32>(conn)?,
            );
            diesel::update(
                r::table
                    .filter(r::rule_id.eq(rule_id))
                    .filter(r::state.eq("scheduled")),
            )
            .set(r::shared.eq(!request.private))
            .execute(conn)?;
        }
        if !request.series
            || !request.new_only
            || program
                .as_ref()
                .is_some_and(|p| p.metadata.is_new == Some(true))
        {
            insert_timer(conn, user, &request, program.as_ref(), rule_id, true)?;
        }
        Ok(())
    })
    .map_err(super::db_error)
}

fn insert_timer(
    conn: &mut SqliteConnection,
    user: i32,
    request: &ScheduleRequest,
    program: Option<&Program>,
    rule: Option<i32>,
    explicit: bool,
) -> QueryResult<()> {
    let metadata = program
        .map(|p| serde_json::to_string(&p.metadata).unwrap())
        .unwrap_or_else(|| "{}".into());
    diesel::insert_into(r::table)
        .values((
            r::user_id.eq(user),
            r::channel_id.eq(request.channel_id),
            r::rule_id.eq(rule),
            r::title.eq(&request.title),
            r::starts_at.eq(request.starts_at),
            r::ends_at.eq(request.ends_at),
            r::padding_before.eq(request.padding_before),
            r::padding_after.eq(request.padding_after),
            r::state.eq("scheduled"),
            r::metadata_json.eq(&metadata),
            r::shared.eq(!request.private),
        ))
        .on_conflict_do_nothing()
        .execute(conn)?;
    if explicit {
        // A user's explicit retry can restore an uncaptured timer; series expansion must respect
        // cancellation.
        diesel::update(
            r::table
                .filter(r::user_id.eq(user))
                .filter(r::channel_id.eq(request.channel_id))
                .filter(r::starts_at.eq(request.starts_at))
                .filter(r::state.eq_any(["cancelled", "failed"]))
                .filter(r::file_name.is_null()),
        )
        .set((
            r::state.eq("scheduled"),
            r::shared.eq(!request.private),
            r::rule_id.eq(rule),
            r::title.eq(&request.title),
            r::ends_at.eq(request.ends_at),
            r::padding_before.eq(request.padding_before),
            r::padding_after.eq(request.padding_after),
            r::metadata_json.eq(&metadata),
            r::last_error.eq(None::<String>),
        ))
        .execute(conn)?;
        diesel::update(
            r::table
                .filter(r::user_id.eq(user))
                .filter(r::channel_id.eq(request.channel_id))
                .filter(r::starts_at.eq(request.starts_at))
                .filter(r::state.eq("scheduled")),
        )
        .set(r::shared.eq(!request.private))
        .execute(conn)?;
    }
    Ok(())
}

pub(super) fn expand_rules(conn: &mut SqliteConnection) -> QueryResult<()> {
    let rules = rules::table
        .filter(rules::enabled.eq(true))
        .select(SeriesRule::as_select())
        .load::<SeriesRule>(conn)?;
    for rule in rules {
        if !super::settings::can_record(conn, rule.user_id)? {
            continue;
        }
        let channel = storage::channel(conn, rule.channel_id)?;
        let saved = r::table
            .filter(r::rule_id.eq(rule.id))
            .filter(r::state.ne("cancelled"))
            .select(r::metadata_json)
            .load::<String>(conn)?;
        let mut episodes = saved
            .iter()
            .filter_map(|s| {
                serde_json::from_str::<super::formats::ProgramMetadata>(s)
                    .ok()?
                    .episode_id
            })
            .collect::<std::collections::HashSet<_>>();
        let programs = storage::station_programs(
            conn,
            channel.source_id,
            channel
                .guide_override
                .as_deref()
                .unwrap_or(&channel.guide_key),
            &rule.title,
        )?;
        for p in programs
            .iter()
            .filter(|p| !rule.new_only || p.metadata.is_new == Some(true))
        {
            if p.metadata
                .episode_id
                .as_ref()
                .is_some_and(|id| episodes.contains(id))
            {
                continue;
            }
            let request = ScheduleRequest {
                channel_id: channel.id,
                starts_at: p.starts_at,
                ends_at: p.ends_at,
                title: p.title.clone(),
                series: false,
                new_only: false,
                padding_before: rule.padding_before,
                padding_after: rule.padding_after,
                private: !rule.shared,
            };
            insert_timer(conn, rule.user_id, &request, Some(p), Some(rule.id), false)?;
            if let Some(id) = &p.metadata.episode_id {
                episodes.insert(id.clone());
            }
        }
    }
    Ok(())
}

pub(super) fn recover(conn: &mut SqliteConnection) -> QueryResult<()> {
    // MPEG-TS can be appended after restart; preserve already captured bytes.
    diesel::update(r::table.filter(r::state.eq("recording")))
        .set((
            r::state.eq("scheduled"),
            r::last_error.eq("The server restarted during this recording; video may contain a gap"),
        ))
        .execute(conn)?;
    diesel::update(r::table.filter(r::state.eq("stopping")))
        .set(r::state.eq("cancelled"))
        .execute(conn)?;
    Ok(())
}

pub(crate) fn start_workers(
    db: DbConn,
    mut shutdown: rocket::Shutdown,
) {
    let db = Arc::new(db);
    rocket::tokio::spawn(async move {
        if let Err(error) = db.run(recover).await {
            log::error!("Unable to recover Live TV recordings: {error}");
        }
        let deleting = db
            .run(|conn| {
                r::table
                    .filter(r::state.eq("deleting"))
                    .select(r::id)
                    .load::<i32>(conn)
            })
            .await
            .unwrap_or_default();
        for id in deleting {
            match super::remove_recording_file(&db, id).await {
                Ok(()) => {
                    let _ = db
                        .run(move |conn| {
                            diesel::delete(r::table.find(id).filter(r::state.eq("deleting")))
                                .execute(conn)
                        })
                        .await;
                }
                Err(error) => {
                    let _ = db
                        .run(move |conn| {
                            diesel::update(r::table.find(id))
                                .set((r::state.eq("failed"), r::last_error.eq(error)))
                                .execute(conn)
                        })
                        .await;
                }
            }
        }
        let mut workers = tokio::task::JoinSet::new();
        let refresh_db = db.clone();
        workers.spawn(async move {
            let mut refreshes = tokio::task::JoinSet::new();
            let mut active = std::collections::HashMap::new();
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                tokio::select! {
                    _ = interval.tick() => {},
                    completed = refreshes.join_next_with_id(), if !refreshes.is_empty() => {
                        if let Some(result)=completed {
                            match result {
                                Ok((task,result))=>{
                                    if let Some(id)=active.remove(&task) {
                                        if let Err(error)=result {
                                            log::warn!("TV source {id}: {error}");
                                        }
                                    }
                                },
                                Err(error)=>{active.remove(&error.id());},
                            }
                        }
                    },
                }
                let due = refresh_db
                    .run(|conn| {
                        sources::table
                            .filter(sources::enabled.eq(true))
                            .filter(sources::next_refresh.le(super::now()))
                            .select(sources::id)
                            .load::<i32>(conn)
                    })
                    .await
                    .unwrap_or_default();
                for id in due {
                    if refreshes.len() >= 4 {
                        break;
                    }
                    if active.values().any(|value| *value == id) {
                        continue;
                    }
                    let db = refresh_db.clone();
                    let task = refreshes.spawn(async move { super::refresh(&db, id).await });
                    active.insert(task.id(), id);
                }
            }
        });
        loop {
            tokio::select! {
                _=&mut shutdown=>break,
                _=tokio::time::sleep(Duration::from_secs(5))=>{},
            }
            while workers.try_join_next().is_some() {}
            if let Err(error) = db.run(expand_rules).await {
                log::warn!("Unable to expand Live TV series rules: {error}");
            }
            let due = db
                .run(|conn| {
                    // Padding is evaluated in Rust to keep the same interval semantics everywhere.
                    r::table
                        .filter(r::state.eq("scheduled"))
                        .order((r::starts_at, r::id))
                        .select(Recording::as_select())
                        .load::<Recording>(conn)
                })
                .await
                .unwrap_or_default();
            for recording in due
                .into_iter()
                .filter(|r| r.starts_at - i64::from(r.padding_before) <= super::now())
            {
                let id = recording.id;
                let claimed = db
                    .run(move |conn| {
                        diesel::update(r::table.find(id).filter(r::state.eq("scheduled")))
                            .set(r::state.eq("recording"))
                            .execute(conn)
                    })
                    .await
                    .unwrap_or(0);
                if claimed == 0 {
                    continue;
                }
                let record_db = db.clone();
                workers.spawn(async move {
                    let result = capture(&record_db, &recording).await;
                    let (state, error) = match result {
                        Ok(true) => ("completed", None),
                        Ok(false) => ("cancelled", None),
                        Err(error) => ("failed", Some(error)),
                    };
                    let _ = record_db
                        .run(move |conn| {
                            let row = r::table
                                .find(id)
                                .filter(r::state.eq_any(["recording", "stopping"]));
                            match error {
                                Some(error) => diesel::update(row)
                                    .set((r::state.eq(state), r::last_error.eq(error)))
                                    .execute(conn),
                                None => diesel::update(row).set(r::state.eq(state)).execute(conn),
                            }
                        })
                        .await;
                });
            }
        }
        workers.abort_all();
        while workers.join_next().await.is_some() {}
    });
}

async fn capture(
    db: &DbConn,
    recording: &Recording,
) -> Result<bool, String> {
    let deadline = recording.ends_at + i64::from(recording.padding_after);
    if deadline <= super::now() {
        return Err("The server was offline when this recording was scheduled".into());
    }
    let (source, channel, _lease) = stream::input(db, recording.channel_id).await?;
    let id = recording.id;
    let path = db
        .run(move |conn| super::settings::capture_path(conn, id))
        .await?;
    tokio::fs::create_dir_all(path.parent().unwrap())
        .await
        .map_err(|_| "Unable to create the recording folder")?;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
        .map_err(|_| "Unable to write the recording file")?;
    let mut pipeline = stream::pipeline(&source, &channel, true).await?;
    let mut output = pipeline
        .child
        .stdout
        .take()
        .ok_or("FFmpeg did not open the recording stream")?;
    let saved_path = path.to_string_lossy().into_owned();
    db.run(move |conn| {
        diesel::update(r::table.find(id))
            .set(r::file_name.eq(saved_path))
            .execute(conn)
    })
    .await
    .map_err(super::db_error)?;
    let mut buffer = vec![0; 188 * 1024];
    let mut written = 0;
    let mut interval = tokio::time::interval(Duration::from_secs(2));
    loop {
        tokio::select! {
            result=output.read(&mut buffer)=>{
                let bytes=result.map_err(|_|"The TV stream could not be read")?;
                if bytes==0 {
                    file.flush().await.map_err(|_|"Unable to finish saving this recording")?;
                    if written == 0 || super::now() < deadline - 5 {
                        return Err("The TV stream ended early; any captured video was kept".into());
                    }
                    return Ok(true);
                }
                file.write_all(&buffer[..bytes]).await.map_err(|_| {
                    "The recording could not be written; check available disk space"
                })?;
                written+=bytes;
            },
            _=interval.tick()=>{
                let state = db.run(move |conn| {
                    r::table.find(id).select(r::state).first::<String>(conn)
                }).await.map_err(super::db_error)?;
                if state!="recording" || super::now()>=deadline {
                    let _=pipeline.child.kill().await;
                    file.flush().await.map_err(|_|"Unable to finish saving this recording")?;
                    return Ok(state=="recording");
                }
            },
        }
    }
}

/// Compare requested reservations, including padding, against every source's tuner capacity.
pub(crate) fn conflicts(
    conn: &mut SqliteConnection,
    user: i32,
) -> QueryResult<Vec<i32>> {
    let sources = storage::sources(conn)?;
    let channels = channels::table
        .select(super::Channel::as_select())
        .load::<super::Channel>(conn)?;
    let timers = r::table
        .filter(r::state.eq_any(["scheduled", "recording"]))
        .select(Recording::as_select())
        .load::<Recording>(conn)?;
    let mut result = Vec::new();
    for timer in timers.iter().filter(|t| t.user_id == user || t.shared) {
        let source = channels
            .iter()
            .find(|c| c.id == timer.channel_id)
            .and_then(|c| sources.iter().find(|s| s.id == c.source_id));
        let Some(source) = source else {
            continue;
        };
        let start = timer.starts_at - i64::from(timer.padding_before);
        let end = timer.ends_at + i64::from(timer.padding_after);
        let mut edges = Vec::new();
        for other in timers.iter().filter(|t| {
            channels
                .iter()
                .any(|c| c.id == t.channel_id && c.source_id == source.id)
        }) {
            let a = (other.starts_at - i64::from(other.padding_before)).max(start);
            let b = (other.ends_at + i64::from(other.padding_after)).min(end);
            if a < b {
                edges.push((a, 1));
                edges.push((b, -1));
            }
        }
        edges.sort();
        let mut count = 0;
        if edges.iter().any(|(_, change)| {
            count += change;
            count > source.capacity
        }) {
            result.push(timer.id);
        }
    }
    Ok(result)
}
