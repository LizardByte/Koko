//! Per-account podcast order, independent of the shared publisher metadata.
use super::storage;
use crate::{
    db::{
        models::AppSetting,
        schema::app_settings as settings,
    },
    media::{
        MediaItemDetail,
        MediaItemSummary,
        MediaPlaybackTarget,
    },
};
use diesel::QueryResult;
use diesel::prelude::*;
use rocket_okapi::okapi::schemars::{
    self,
    JsonSchema,
};
use serde::{
    Deserialize,
    Serialize,
};
use std::collections::HashMap;

/// Chronological order for one account's podcast subscription.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackOrder {
    /// Play from the oldest publication toward newer episodes.
    #[default]
    OldestFirst,
    /// Play from the newest publication toward older episodes.
    NewestFirst,
}

fn key(
    user: i32,
    podcast: i32,
) -> String {
    format!("podcast_order:{user}:{podcast}")
}

/// Read the account's saved order, defaulting to oldest first for existing subscriptions.
pub fn get(
    conn: &mut SqliteConnection,
    user: i32,
    podcast: i32,
) -> QueryResult<PlaybackOrder> {
    let value = settings::table
        .find(key(user, podcast))
        .select(settings::value)
        .first::<String>(conn)
        .optional()?;
    Ok(value
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default())
}

/// Save an order only for a show to which this account is subscribed.
pub fn set(
    conn: &mut SqliteConnection,
    user: i32,
    podcast: i32,
    order: PlaybackOrder,
) -> QueryResult<()> {
    storage::feed_for_user(conn, user, podcast)?;
    let row = AppSetting {
        key: key(user, podcast),
        value: serde_json::to_string(&order).unwrap(),
        updated_at: Some(crate::utils::current_timestamp()),
    };
    diesel::insert_into(settings::table)
        .values(&row)
        .on_conflict(settings::key)
        .do_update()
        .set(&row)
        .execute(conn)?;
    Ok(())
}

/// Keep unknown publication dates last and use publisher numbering to resolve date ties.
pub fn sort(
    items: &mut [MediaItemSummary],
    order: PlaybackOrder,
) {
    items.sort_by(|left, right| {
        let date =
            |item: &MediaItemSummary| item.podcast.as_ref().and_then(|value| value.published_at);
        let left_date = date(left);
        let right_date = date(right);
        let known = right_date.is_some().cmp(&left_date.is_some());
        if !known.is_eq() {
            return known;
        }
        let chronological = left_date
            .cmp(&right_date)
            .then_with(|| left.season_number.cmp(&right.season_number))
            .then_with(|| left.episode_number.cmp(&right.episode_number))
            .then_with(|| left.id.cmp(&right.id));
        if order == PlaybackOrder::NewestFirst { chronological.reverse() } else { chronological }
    });
}

/// Return every episode across real seasons, with the account's independent listening progress.
pub fn episodes(
    conn: &mut SqliteConnection,
    user: i32,
    podcast: i32,
) -> QueryResult<Vec<MediaItemSummary>> {
    storage::feed_for_user(conn, user, podcast)?;
    let ids = storage::episode_ids(conn, podcast)?;
    let mut items = crate::media::list_media_items_by_ids(conn, &ids, &[])?;
    crate::media::apply_user_playback_progress_to_summaries(conn, Some(user), &mut items)?;
    let order = get(conn, user, podcast)?;
    for item in &mut items {
        if let Some(metadata) = &mut item.podcast {
            metadata.playback_order = order;
        }
    }
    sort(&mut items, order);
    Ok(items)
}

/// Apply preferences after account access filtering so shared metadata never stores a user's order.
pub fn apply(
    conn: &mut SqliteConnection,
    user: Option<i32>,
    items: &mut [MediaItemSummary],
) -> QueryResult<()> {
    let Some(user) = user else {
        return Ok(());
    };
    let mut orders = HashMap::new();
    for item in items {
        if let Some(metadata) = &mut item.podcast {
            if let std::collections::hash_map::Entry::Vacant(entry) =
                orders.entry(metadata.podcast_id)
            {
                entry.insert(get(conn, user, metadata.podcast_id)?);
            }
            metadata.playback_order = orders[&metadata.podcast_id];
        }
    }
    Ok(())
}

/// Select a resumable episode and retain the same order through season boundaries.
pub fn apply_detail(
    conn: &mut SqliteConnection,
    user: Option<i32>,
    detail: &mut MediaItemDetail,
) -> QueryResult<()> {
    let Some(metadata) = detail.podcast.as_mut() else {
        return Ok(());
    };
    let Some(user) = user else {
        return Ok(());
    };
    metadata.playback_order = get(conn, user, metadata.podcast_id)?;
    let root = metadata.podcast_id;
    apply(conn, Some(user), &mut detail.hierarchy)?;
    apply(conn, Some(user), &mut detail.children)?;
    if detail.playable {
        return Ok(());
    }
    let mut candidates = episodes(conn, user, root)?;
    if detail.item_type == "podcast_season" {
        candidates.retain(|item| item.parent_id == Some(detail.id));
    }
    let recent = candidates
        .iter()
        .filter(|item| {
            !item.playback_completed && item.playback_position_ms.unwrap_or_default() > 0
        })
        .max_by_key(|item| item.last_played_at);
    let selected = recent
        .or_else(|| candidates.iter().find(|item| !item.playback_completed))
        .or_else(|| candidates.first());
    detail.playback_target = selected.map(target);
    detail.restart_playback_target = candidates.first().map(|item| MediaPlaybackTarget {
        start_ms: 0,
        resume: false,
        label: "Start podcast over".into(),
        ..target(item)
    });
    Ok(())
}

fn target(item: &MediaItemSummary) -> MediaPlaybackTarget {
    let position =
        if item.playback_completed { 0 } else { item.playback_position_ms.unwrap_or_default() };
    MediaPlaybackTarget {
        item_id: item.id,
        start_ms: position,
        resume: position > 0,
        label: if position > 0 { "Resume listening" } else { "Play podcast" }.into(),
        display_title: item.display_title.clone(),
        season_number: item.season_number,
        episode_number: item.episode_number,
    }
}
