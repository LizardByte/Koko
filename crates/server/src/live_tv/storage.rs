//! Typed persistence with stable channel IDs and transactional guide replacement.

use super::formats::{
    Guide,
    Station,
};
use super::{
    Channel,
    Program,
};
use crate::db::schema::live_tv_stations as stations;
use crate::db::schema::{
    live_tv_channels as c,
    live_tv_programs as p,
    live_tv_recordings as r,
    live_tv_series_rules as rules_table,
    live_tv_sources as s,
};
use diesel::prelude::*;
use rocket_okapi::okapi::schemars::{
    self,
    JsonSchema,
};
use serde::Serialize;

pub(crate) fn guide_stations(conn: &mut SqliteConnection) -> QueryResult<Vec<Station>> {
    type Row = (i32, String, String, String, String, Option<String>);
    let rows = stations::table.load::<Row>(conn)?;
    Ok(rows
        .into_iter()
        .map(
            |(source_id, guide_key, name, number, aliases, logo)| Station {
                source_id,
                guide_key,
                name,
                number,
                aliases: serde_json::from_str(&aliases).unwrap_or_default(),
                logo,
            },
        )
        .collect())
}

pub(super) fn save_guide(
    conn: &mut SqliteConnection,
    source: i32,
    guide: &Guide,
) -> QueryResult<()> {
    conn.transaction(|conn| {
        save_programs(conn, source, &guide.programs)?;
        diesel::delete(stations::table.filter(stations::source_id.eq(source))).execute(conn)?;
        for station in &guide.stations {
            diesel::insert_into(stations::table)
                .values((
                    stations::source_id.eq(source),
                    stations::guide_key.eq(&station.guide_key),
                    stations::name.eq(&station.name),
                    stations::number.eq(&station.number),
                    stations::aliases_json.eq(serde_json::to_string(&station.aliases).unwrap()),
                    stations::logo.eq(&station.logo),
                ))
                .on_conflict_do_nothing()
                .execute(conn)?;
        }
        let channels = c::table
            .filter(c::source_id.eq(source))
            .select(Channel::as_select())
            .load::<Channel>(conn)?;
        for channel in channels {
            let exact = guide
                .stations
                .iter()
                .find(|s| s.guide_key == channel.guide_key);
            let matches = guide
                .stations
                .iter()
                .filter(|s| {
                    !channel.number.is_empty()
                        && (s.number == channel.number || s.aliases.contains(&channel.number))
                        || s.name.eq_ignore_ascii_case(&channel.name)
                        || s.aliases
                            .iter()
                            .any(|n| n.eq_ignore_ascii_case(&channel.name))
                })
                .collect::<Vec<_>>();
            let selected =
                exact.or_else(|| if matches.len() == 1 { Some(matches[0]) } else { None });
            if let Some(station) = selected {
                diesel::update(c::table.find(channel.id))
                    .set((
                        c::guide_key.eq(&station.guide_key),
                        c::logo.eq(channel.logo.or_else(|| station.logo.clone())),
                    ))
                    .execute(conn)?;
            }
        }
        Ok(())
    })
}

pub(super) fn merge_guide(
    conn: &mut SqliteConnection,
    source: i32,
    incoming: Guide,
) -> QueryResult<()> {
    conn.transaction(|conn| {
        let mut programs = guide(conn, super::now() - 86400, super::now() + 21 * 86400)?
            .into_iter()
            .filter(|p| p.source_id == source)
            .map(|p| ((p.guide_key.clone(), p.starts_at), p))
            .collect::<std::collections::HashMap<_, _>>();
        for program in incoming.programs {
            programs.insert((program.guide_key.clone(), program.starts_at), program);
        }
        let mut stations = guide_stations(conn)?
            .into_iter()
            .filter(|s| s.source_id == source)
            .map(|s| (s.guide_key.clone(), s))
            .collect::<std::collections::HashMap<_, _>>();
        for station in incoming.stations {
            stations.insert(station.guide_key.clone(), station);
        }
        save_guide(
            conn,
            source,
            &Guide {
                programs: programs.into_values().collect(),
                stations: stations.into_values().collect(),
            },
        )
    })
}

#[derive(Clone, Queryable, Selectable, Serialize, JsonSchema)]
#[diesel(table_name = s)]
pub(crate) struct Source {
    pub id: i32,
    pub name: String,
    pub kind: String,
    #[serde(skip)]
    pub config_json: String,
    pub enabled: bool,
    pub capacity: i32,
    pub next_refresh: i64,
    pub last_error: Option<String>,
}

#[derive(Queryable, Serialize, JsonSchema)]
pub(crate) struct GuideSource {
    pub id: i32,
    pub name: String,
    pub kind: String,
}

pub(crate) fn guide_sources(conn: &mut SqliteConnection) -> QueryResult<Vec<GuideSource>> {
    s::table
        .filter(s::enabled.eq(true))
        .select((s::id, s::name, s::kind))
        .order(s::id)
        .load(conn)
}

#[derive(Clone, Queryable, Selectable, Serialize, JsonSchema)]
#[diesel(table_name = r)]
pub(crate) struct Recording {
    pub id: i32,
    pub user_id: i32,
    pub channel_id: i32,
    pub rule_id: Option<i32>,
    pub title: String,
    pub starts_at: i64,
    pub ends_at: i64,
    pub padding_before: i32,
    pub padding_after: i32,
    pub state: String,
    #[diesel(select_expression = r::file_name.is_not_null())]
    pub has_video: bool,
    pub last_error: Option<String>,
    #[serde(rename = "metadata", serialize_with = "serialize_recording_metadata")]
    #[schemars(with = "super::formats::ProgramMetadata")]
    pub metadata_json: String,
    pub shared: bool,
}

fn serialize_recording_metadata<S: serde::Serializer>(
    value: &str,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let metadata =
        serde_json::from_str::<super::formats::ProgramMetadata>(value).unwrap_or_default();
    metadata.serialize(serializer)
}

#[derive(Clone, Queryable, Selectable, Serialize, JsonSchema)]
#[diesel(table_name = rules_table)]
pub(crate) struct SeriesRule {
    pub id: i32,
    pub user_id: i32,
    pub channel_id: i32,
    pub title: String,
    pub new_only: bool,
    pub padding_before: i32,
    pub padding_after: i32,
    pub enabled: bool,
    pub shared: bool,
}

pub(crate) fn sources(conn: &mut SqliteConnection) -> QueryResult<Vec<Source>> {
    s::table
        .order(s::name)
        .select(Source::as_select())
        .load(conn)
}

pub(crate) fn source(
    conn: &mut SqliteConnection,
    id: i32,
) -> QueryResult<Source> {
    s::table.find(id).select(Source::as_select()).first(conn)
}

pub(crate) fn channel(
    conn: &mut SqliteConnection,
    id: i32,
) -> QueryResult<Channel> {
    c::table.find(id).select(Channel::as_select()).first(conn)
}

pub(crate) fn channels(
    conn: &mut SqliteConnection,
    include_disabled: bool,
) -> QueryResult<Vec<Channel>> {
    let mut query = c::table
        .filter(c::source_id.eq_any(s::table.filter(s::enabled.eq(true)).select(s::id)))
        .into_boxed();
    if !include_disabled {
        query = query
            .filter(c::enabled.eq(true))
            .filter(c::available.eq(true));
    }
    let mut rows = query.select(Channel::as_select()).load::<Channel>(conn)?;
    rows.sort_by(|a, b| {
        number_key(&a.number)
            .cmp(&number_key(&b.number))
            .then(a.name.cmp(&b.name))
    });
    Ok(rows)
}

fn number_key(value: &str) -> (u32, u32) {
    let mut parts = value.split(['.', '-']);
    (
        parts
            .next()
            .and_then(|p| p.parse().ok())
            .unwrap_or(u32::MAX),
        parts.next().and_then(|p| p.parse().ok()).unwrap_or(0),
    )
}

pub(crate) fn save_channels(
    conn: &mut SqliteConnection,
    source: i32,
    rows: &[Channel],
) -> QueryResult<()> {
    conn.transaction(|conn| {
        diesel::update(c::table.filter(c::source_id.eq(source)))
            .set(c::available.eq(false))
            .execute(conn)?;
        for row in rows {
            let values = (
                c::name.eq(&row.name),
                c::number.eq(&row.number),
                c::group_name.eq(&row.group_name),
                c::logo.eq(&row.logo),
                c::stream_url.eq(&row.stream_url),
                c::guide_key.eq(&row.guide_key),
                c::available.eq(true),
            );
            diesel::insert_into(c::table)
                .values((
                    c::source_id.eq(source),
                    c::source_key.eq(&row.source_key),
                    values,
                ))
                .on_conflict((c::source_id, c::source_key))
                .do_update()
                .set(values)
                .execute(conn)?;
        }
        Ok(())
    })
}

pub(crate) fn save_programs(
    conn: &mut SqliteConnection,
    source: i32,
    rows: &[Program],
) -> QueryResult<()> {
    conn.transaction(|conn| {
        diesel::delete(p::table.filter(p::source_id.eq(source))).execute(conn)?;
        for row in rows {
            diesel::insert_into(p::table)
                .values((
                    p::source_id.eq(source),
                    p::guide_key.eq(&row.guide_key),
                    p::starts_at.eq(row.starts_at),
                    p::ends_at.eq(row.ends_at),
                    p::title.eq(&row.title),
                    p::metadata_json.eq(serde_json::to_string(&row.metadata).unwrap()),
                ))
                .on_conflict_do_nothing()
                .execute(conn)?;
        }
        Ok(())
    })
}

pub(crate) fn guide(
    conn: &mut SqliteConnection,
    start: i64,
    end: i64,
) -> QueryResult<Vec<Program>> {
    type Row = (i32, i32, String, i64, i64, String, String);
    let rows = p::table
        .filter(p::starts_at.lt(end))
        .filter(p::ends_at.gt(start))
        .filter(p::source_id.eq_any(s::table.filter(s::enabled.eq(true)).select(s::id)))
        .order((p::guide_key, p::starts_at))
        .limit(50_000)
        .load::<Row>(conn)?;
    Ok(rows
        .into_iter()
        .map(
            |(id, source_id, guide_key, starts_at, ends_at, title, metadata)| Program {
                id,
                source_id,
                guide_key,
                starts_at,
                ends_at,
                title,
                metadata: serde_json::from_str(&metadata).unwrap_or_default(),
            },
        )
        .collect())
}

pub(super) fn station_programs(
    conn: &mut SqliteConnection,
    source: i32,
    key: &str,
    title: &str,
) -> QueryResult<Vec<Program>> {
    type Row = (i32, i32, String, i64, i64, String, String);
    let rows = p::table
        .filter(p::source_id.eq(source))
        .filter(p::guide_key.eq(key))
        .filter(p::title.eq(title))
        .filter(p::ends_at.gt(super::now()))
        .order(p::starts_at)
        .limit(5000)
        .load::<Row>(conn)?;
    Ok(rows
        .into_iter()
        .map(
            |(id, source_id, guide_key, starts_at, ends_at, title, metadata)| Program {
                id,
                source_id,
                guide_key,
                starts_at,
                ends_at,
                title,
                metadata: serde_json::from_str(&metadata).unwrap_or_default(),
            },
        )
        .collect())
}

pub(crate) fn recordings(
    conn: &mut SqliteConnection,
    user: i32,
) -> QueryResult<Vec<Recording>> {
    r::table
        .filter(r::user_id.eq(user).or(r::shared.eq(true)))
        .order(r::starts_at.desc())
        .select(Recording::as_select())
        .load(conn)
}

pub(crate) fn claim_deletion(
    conn: &mut SqliteConnection,
    user: i32,
    id: i32,
) -> QueryResult<Option<String>> {
    let previous = r::table
        .find(id)
        .filter(r::user_id.eq(user))
        .select(r::state)
        .first::<String>(conn)
        .optional()?;
    let Some(previous) = previous else {
        return Ok(None);
    };
    if matches!(previous.as_str(), "recording" | "stopping") {
        return Ok(None);
    }
    let changed = diesel::update(
        r::table
            .find(id)
            .filter(r::user_id.eq(user))
            .filter(r::state.eq(&previous)),
    )
    .set(r::state.eq("deleting"))
    .execute(conn)?;
    Ok((changed > 0).then_some(previous))
}

pub(crate) fn rules(
    conn: &mut SqliteConnection,
    user: i32,
) -> QueryResult<Vec<SeriesRule>> {
    rules_table::table
        .filter(
            rules_table::user_id
                .eq(user)
                .or(rules_table::shared.eq(true)),
        )
        .select(SeriesRule::as_select())
        .load(conn)
}
