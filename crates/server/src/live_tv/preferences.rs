//! Account-scoped favorite channels and bounded recently tuned channel history.
use crate::db::schema::{
    app_settings as a,
    live_tv_channels as c,
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

#[derive(Default, Deserialize, Serialize, JsonSchema)]
#[serde(default)]
pub(crate) struct Preferences {
    pub favorites: Vec<i32>,
    pub recent: Vec<i32>,
}

fn key(user: i32) -> String {
    format!("live_tv_user:{user}")
}

pub(crate) fn load(
    conn: &mut SqliteConnection,
    user: i32,
) -> QueryResult<Preferences> {
    let mut value: Preferences = a::table
        .find(key(user))
        .select(a::value)
        .first::<String>(conn)
        .optional()?
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let channels = super::storage::channels(conn, false)?
        .into_iter()
        .map(|c| c.id)
        .collect::<std::collections::HashSet<_>>();
    value.favorites.retain(|id| channels.contains(id));
    value.recent.retain(|id| channels.contains(id));
    Ok(value)
}

fn save(
    conn: &mut SqliteConnection,
    user: i32,
    value: &Preferences,
) -> QueryResult<()> {
    let value = serde_json::to_string(value).expect("serializable channel preferences");
    diesel::insert_into(a::table)
        .values((
            a::key.eq(key(user)),
            a::value.eq(&value),
            a::updated_at.eq(super::now()),
        ))
        .on_conflict(a::key)
        .do_update()
        .set((a::value.eq(&value), a::updated_at.eq(super::now())))
        .execute(conn)?;
    Ok(())
}

pub(crate) fn favorite(
    conn: &mut SqliteConnection,
    user: i32,
    channel: i32,
    enabled: bool,
) -> QueryResult<()> {
    conn.transaction(|conn| {
        c::table
            .find(channel)
            .filter(c::enabled.eq(true).and(c::available.eq(true)))
            .select(c::id)
            .first::<i32>(conn)?;
        let mut value = load(conn, user)?;
        value.favorites.retain(|id| *id != channel);
        if enabled {
            value.favorites.push(channel);
        }
        save(conn, user, &value)
    })
}

pub(crate) fn played(
    conn: &mut SqliteConnection,
    user: i32,
    channel: i32,
) -> QueryResult<()> {
    conn.transaction(|conn| {
        let mut value = load(conn, user)?;
        value.recent.retain(|id| *id != channel);
        value.recent.insert(0, channel);
        value.recent.truncate(30);
        save(conn, user, &value)
    })
}
