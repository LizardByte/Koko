//! Durable source grouping and last-import feed sizes, without merging tuner resource pools.
use crate::db::schema::{
    app_settings as a,
    live_tv_channels as c,
    live_tv_sources as s,
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
use std::collections::{
    HashMap,
    HashSet,
};

const GROUPS_KEY: &str = "live_tv_source_groups";

#[derive(Clone, Deserialize, Serialize, JsonSchema)]
pub(crate) struct SourceGroup {
    pub id: String,
    pub name: String,
    pub source_ids: Vec<i32>,
}

#[derive(Default, Deserialize, Serialize, JsonSchema)]
#[serde(default)]
pub(crate) struct FeedSizes {
    pub playlist_bytes: Option<u64>,
    pub guide_bytes: Option<u64>,
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct SourceSummary {
    #[serde(flatten)]
    pub source: super::Source,
    pub channel_count: i64,
    pub image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iptv_mode: Option<String>,
    pub providers: Vec<super::providers::ProviderSummary>,
    #[serde(flatten)]
    pub sizes: FeedSizes,
}

pub(super) fn load<T: serde::de::DeserializeOwned + Default>(
    conn: &mut SqliteConnection,
    key: &str,
) -> QueryResult<T> {
    Ok(a::table
        .find(key)
        .select(a::value)
        .first::<String>(conn)
        .optional()?
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default())
}

pub(super) fn save(
    conn: &mut SqliteConnection,
    key: &str,
    value: impl Serialize,
) -> QueryResult<()> {
    let value =
        serde_json::to_string(&value).map_err(|_| diesel::result::Error::RollbackTransaction)?;
    diesel::insert_into(a::table)
        .values((
            a::key.eq(key),
            a::value.eq(&value),
            a::updated_at.eq(super::now()),
        ))
        .on_conflict(a::key)
        .do_update()
        .set((a::value.eq(&value), a::updated_at.eq(super::now())))
        .execute(conn)?;
    Ok(())
}

pub(crate) fn groups(conn: &mut SqliteConnection) -> QueryResult<Vec<SourceGroup>> {
    load(conn, GROUPS_KEY)
}

pub(crate) fn save_groups(
    conn: &mut SqliteConnection,
    mut groups: Vec<SourceGroup>,
) -> Result<(), String> {
    let sources = s::table
        .filter(s::kind.ne(super::removal::REMOVED))
        .select(s::id)
        .load::<i32>(conn)
        .map_err(super::db_error)?
        .into_iter()
        .collect::<HashSet<_>>();
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for group in &mut groups {
        group.name = group.name.trim().to_owned();
        if group.id.is_empty()
            || group.id.len() > 80
            || !group
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            || !ids.insert(group.id.clone())
        {
            return Err("Source group IDs must be unique".into());
        }
        if group.name.is_empty()
            || group.name.len() > 200
            || !names.insert(group.name.to_lowercase())
        {
            return Err("Enter a unique name for each source group".into());
        }
        group.source_ids.sort_unstable();
        group.source_ids.dedup();
        if group.source_ids.len() < 2 || group.source_ids.iter().any(|id| !sources.contains(id)) {
            return Err("Choose at least two existing sources for each group".into());
        }
    }
    save(conn, GROUPS_KEY, groups).map_err(super::db_error)
}

pub(super) fn feed_sizes(
    conn: &mut SqliteConnection,
    id: i32,
) -> QueryResult<FeedSizes> {
    load(conn, &format!("live_tv_feed_sizes:{id}"))
}

pub(super) fn save_feed_sizes(
    conn: &mut SqliteConnection,
    id: i32,
    sizes: &FeedSizes,
) -> QueryResult<()> {
    save(conn, &format!("live_tv_feed_sizes:{id}"), sizes)
}

pub(crate) fn sources(conn: &mut SqliteConnection) -> QueryResult<Vec<SourceSummary>> {
    let counts = c::table
        .filter(c::available.eq(true))
        .group_by(c::source_id)
        .select((c::source_id, diesel::dsl::count_star()))
        .load::<(i32, i64)>(conn)?
        .into_iter()
        .collect::<HashMap<_, _>>();
    super::storage::sources(conn)?
        .into_iter()
        .map(|source| {
            let sizes = feed_sizes(conn, source.id)?;
            let metadata = super::providers::load(conn, source.id)?;
            Ok(SourceSummary {
                channel_count: counts.get(&source.id).copied().unwrap_or(0),
                image_url: metadata.image_url(source.id, None),
                providers: super::providers::summaries(conn, &source, &metadata)?,
                iptv_mode: super::sources::iptv_mode(&source),
                source,
                sizes,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
