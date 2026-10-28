//! Typed persistence for shared feeds, stable episode identities, and private subscriptions.
use super::{
    PodcastMetadata,
    feed::Feed,
};
use crate::{
    db::{
        models::NewMediaItem,
        schema::{
            media_items as items,
            media_libraries as libraries,
            podcast_feed_aliases as aliases,
            podcast_feeds as feeds,
            podcast_items as metadata,
            podcast_subscriptions as subscriptions,
        },
    },
    media::{
        MediaItemDetail,
        MediaItemSummary,
        PlaybackDecision,
    },
    utils::current_timestamp,
};
use diesel::prelude::*;
use std::collections::{
    HashMap,
    HashSet,
};

type Result<T> = diesel::QueryResult<T>;

fn feed_id(
    conn: &mut SqliteConnection,
    url: &str,
) -> Result<Option<i32>> {
    aliases::table
        .find(url)
        .select(aliases::podcast_id)
        .first(conn)
        .optional()
}
fn add_subscription(
    conn: &mut SqliteConnection,
    user: i32,
    podcast: i32,
) -> Result<()> {
    diesel::insert_into(subscriptions::table)
        .values((
            subscriptions::user_id.eq(user),
            subscriptions::podcast_id.eq(podcast),
            subscriptions::added_at.eq(current_timestamp()),
        ))
        .on_conflict_do_nothing()
        .execute(conn)?;
    Ok(())
}
pub(super) fn subscribe_existing(
    conn: &mut SqliteConnection,
    user: i32,
    url: &str,
) -> Result<Option<i32>> {
    conn.immediate_transaction(|conn| {
        let id = feed_id(conn, url)?;
        if let Some(id) = id {
            add_subscription(conn, user, id)?;
        }
        Ok(id)
    })
}

fn library(conn: &mut SqliteConnection) -> Result<i32> {
    if let Some(id) = libraries::table
        .filter(libraries::kind.eq("podcasts"))
        .select(libraries::id)
        .first(conn)
        .optional()?
    {
        return Ok(id);
    }
    diesel::insert_into(libraries::table)
        .values((
            libraries::name.eq("Podcasts"),
            libraries::path.eq("koko://podcasts"),
            libraries::kind.eq("podcasts"),
            libraries::metadata_providers_json.eq("[]"),
        ))
        .execute(conn)?;
    libraries::table
        .order(libraries::id.desc())
        .select(libraries::id)
        .first(conn)
}

fn item(
    conn: &mut SqliteConnection,
    library_id: i32,
    identity: String,
    title: &str,
    kind: &str,
    parent: Option<i32>,
    episode: Option<&super::feed::Episode>,
) -> Result<i32> {
    let timestamp = current_timestamp();
    let row = NewMediaItem {
        library_id,
        parent_id: parent,
        identity_key: identity.clone(),
        item_type: kind.into(),
        display_title: title.into(),
        relative_path: None,
        media_kind: Some(
            if episode.is_some_and(|e| e.mime.starts_with("video/")) { "video" } else { "audio" }
                .into(),
        ),
        season_number: episode.and_then(|e| e.season),
        episode_number: episode.and_then(|e| e.number),
        child_count: 0,
        playable: episode.is_some(),
        file_size: None,
        duration_ms: episode.and_then(|e| e.duration_ms),
        modified_at: episode.and_then(|e| e.metadata.published_at),
        created_at: Some(timestamp),
        updated_at: Some(timestamp),
        missing_since: None,
        deleted_at: None,
    };
    diesel::insert_into(items::table)
        .values(&row)
        .on_conflict(items::identity_key)
        .do_update()
        .set((
            items::display_title.eq(title),
            items::parent_id.eq(parent),
            items::season_number.eq(row.season_number),
            items::episode_number.eq(row.episode_number),
            items::duration_ms.eq(row.duration_ms),
            items::modified_at.eq(row.modified_at),
            items::media_kind.eq(row.media_kind.as_deref()),
            items::updated_at.eq(timestamp),
            items::deleted_at.eq(None::<i64>),
        ))
        .execute(conn)?;
    items::table
        .filter(items::identity_key.eq(identity))
        .select(items::id)
        .first(conn)
}
fn save_metadata(
    conn: &mut SqliteConnection,
    id: i32,
    root: i32,
    value: &PodcastMetadata,
    enclosure: Option<&str>,
    mime: Option<&str>,
) -> Result<()> {
    let mut value = value.clone();
    value.podcast_id = root;
    let local_download = metadata::table
        .find(id)
        .select(metadata::metadata_json)
        .first::<String>(conn)
        .optional()?
        .and_then(|json| serde_json::from_str::<super::downloads::StoredMetadata>(&json).ok())
        .and_then(|value| value.local_download);
    let stored = super::downloads::StoredMetadata {
        publisher: value,
        local_download,
    };
    let json =
        serde_json::to_string(&stored).expect("Podcast metadata contains serializable values");
    diesel::insert_into(metadata::table)
        .values((
            metadata::media_item_id.eq(id),
            metadata::podcast_id.eq(root),
            metadata::metadata_json.eq(&json),
            metadata::enclosure_url.eq(enclosure),
            metadata::mime_type.eq(mime),
        ))
        .on_conflict(metadata::media_item_id)
        .do_update()
        .set((
            metadata::metadata_json.eq(&json),
            metadata::enclosure_url.eq(enclosure),
            metadata::mime_type.eq(mime),
        ))
        .execute(conn)?;
    Ok(())
}

pub(super) fn save(
    conn: &mut SqliteConnection,
    user: Option<i32>,
    requested: &str,
    canonical: &str,
    feed: &Feed,
) -> Result<i32> {
    conn.immediate_transaction(|conn| {
        let library_id = library(conn)?;
        let existing_feed = feed_id(conn, requested)?.or(feed_id(conn, canonical)?);
        let root = match existing_feed {
            Some(id) => {
                diesel::update(items::table.find(id))
                    .set((
                        items::display_title.eq(&feed.title),
                        items::updated_at.eq(current_timestamp()),
                    ))
                    .execute(conn)?;
                id
            }
            None => item(
                conn,
                library_id,
                format!("podcast:{canonical}"),
                &feed.title,
                "podcast",
                None,
                None,
            )?,
        };
        diesel::insert_into(feeds::table)
            .values((
                feeds::media_item_id.eq(root),
                feeds::feed_url.eq(canonical),
                feeds::checked_at.eq(current_timestamp()),
            ))
            .on_conflict(feeds::media_item_id)
            .do_update()
            .set((
                feeds::feed_url.eq(canonical),
                feeds::checked_at.eq(current_timestamp()),
                feeds::last_error.eq(None::<String>),
            ))
            .execute(conn)?;
        for url in [requested, canonical] {
            diesel::insert_into(aliases::table)
                .values((aliases::feed_url.eq(url), aliases::podcast_id.eq(root)))
                .on_conflict_do_nothing()
                .execute(conn)?;
        }
        let mut show_metadata = feed.metadata.clone();
        show_metadata.feed_url = Some(canonical.to_string());
        save_metadata(conn, root, root, &show_metadata, None, None)?;
        if let Some(user) = user {
            add_subscription(conn, user, root)?;
        }
        for episode in &feed.episodes {
            let parent = episode_parent(
                conn,
                library_id,
                root,
                episode.season,
                episode.metadata.published_at,
                &feed.metadata,
            )?;
            let id = item(
                conn,
                library_id,
                format!("podcast:{root}:episode:{}", episode.guid),
                &episode.title,
                "podcast_episode",
                Some(parent),
                Some(episode),
            )?;
            let is_new = metadata::table
                .find(id)
                .select(metadata::media_item_id)
                .first::<i32>(conn)
                .optional()?
                .is_none();
            save_metadata(
                conn,
                id,
                root,
                &episode.metadata,
                Some(&episode.url),
                Some(&episode.mime),
            )?;
            if existing_feed.is_some()
                && is_new
                && crate::config::current_settings()
                    .podcasts
                    .download_new_episodes
            {
                super::downloads::queue(conn, id)?;
            }
        }
        // Rolling feeds often omit older episodes; keep them and their listening history.
        let parent_ids = items::table
            .inner_join(metadata::table)
            .filter(metadata::podcast_id.eq(root))
            .filter(items::item_type.eq_any(["podcast", "podcast_season"]))
            .select(items::id)
            .load::<i32>(conn)?;
        for parent in parent_ids
            .into_iter()
            .filter(|id| *id != root)
            .chain(std::iter::once(root))
        {
            let count = items::table
                .filter(items::parent_id.eq(parent))
                .filter(items::deleted_at.is_null())
                .count()
                .get_result::<i64>(conn)?;
            if parent != root && count == 0 {
                diesel::delete(items::table.find(parent)).execute(conn)?;
            } else {
                diesel::update(items::table.find(parent))
                    .set(items::child_count.eq(count as i32))
                    .execute(conn)?;
            }
        }
        Ok(root)
    })
}

/// Check subscription ownership for every podcast item, including seasons and episode streams.
pub fn can_access(
    conn: &mut SqliteConnection,
    item: i32,
    user: Option<i32>,
) -> Result<bool> {
    let Some(user) = user else {
        return Ok(false);
    };
    Ok(metadata::table
        .inner_join(subscriptions::table.on(subscriptions::podcast_id.eq(metadata::podcast_id)))
        .filter(metadata::media_item_id.eq(item))
        .filter(subscriptions::user_id.eq(user))
        .select(metadata::media_item_id)
        .first::<i32>(conn)
        .optional()?
        .is_some())
}
pub(crate) fn has_subscription(
    conn: &mut SqliteConnection,
    user: i32,
) -> Result<bool> {
    Ok(subscriptions::table
        .filter(subscriptions::user_id.eq(user))
        .select(subscriptions::podcast_id)
        .first::<i32>(conn)
        .optional()?
        .is_some())
}
/// Remove inaccessible podcast rows before Home, search, or library results are returned.
pub fn filter_visible(
    conn: &mut SqliteConnection,
    user: Option<i32>,
    items: &mut Vec<MediaItemSummary>,
) -> Result<()> {
    let visible = metadata::table
        .inner_join(subscriptions::table.on(subscriptions::podcast_id.eq(metadata::podcast_id)))
        .filter(subscriptions::user_id.eq(user.unwrap_or(-1)))
        .select(metadata::media_item_id)
        .load::<i32>(conn)?
        .into_iter()
        .collect::<HashSet<_>>();
    items.retain(|item| !item.item_type.starts_with("podcast") || visible.contains(&item.id));
    Ok(())
}
/// Return subscribed shows using Koko's regular artwork, hierarchy, and listening state.
pub fn subscribed(
    conn: &mut SqliteConnection,
    user: i32,
) -> Result<Vec<MediaItemSummary>> {
    let mut items = crate::media::list_media_items_for_user_with_preferred_languages(
        conn,
        Some(user),
        None,
        &[],
    )?;
    items.retain(|item| item.item_type == "podcast");
    Ok(items)
}
pub(crate) fn feed_for_user(
    conn: &mut SqliteConnection,
    user: i32,
    id: i32,
) -> Result<String> {
    if !can_access(conn, id, Some(user))? {
        return Err(diesel::result::Error::NotFound);
    }
    feeds::table.find(id).select(feeds::feed_url).first(conn)
}
pub(crate) fn unsubscribe(
    conn: &mut SqliteConnection,
    user: i32,
    id: i32,
) -> Result<()> {
    diesel::delete(
        subscriptions::table
            .filter(subscriptions::user_id.eq(user))
            .filter(subscriptions::podcast_id.eq(id)),
    )
    .execute(conn)?;
    Ok(())
}
pub(super) fn record_check(
    conn: &mut SqliteConnection,
    id: i32,
    error: Option<String>,
) -> Result<()> {
    diesel::update(feeds::table.find(id))
        .set((
            feeds::checked_at.eq(current_timestamp()),
            feeds::last_error.eq(error),
        ))
        .execute(conn)?;
    Ok(())
}
pub(super) fn due(conn: &mut SqliteConnection) -> Result<Vec<(i32, String)>> {
    feeds::table
        .inner_join(subscriptions::table)
        .filter(feeds::checked_at.lt(current_timestamp() - 3600))
        .select((feeds::media_item_id, feeds::feed_url))
        .distinct()
        .limit(100)
        .load(conn)
}

fn load_metadata(
    conn: &mut SqliteConnection,
    ids: &[i32],
) -> Result<HashMap<i32, PodcastMetadata>> {
    let mut result = HashMap::new();
    for chunk in ids.chunks(500) {
        for (id, json) in metadata::table
            .filter(metadata::media_item_id.eq_any(chunk))
            .select((metadata::media_item_id, metadata::metadata_json))
            .load::<(i32, String)>(conn)?
        {
            if let Ok(mut value) = serde_json::from_str::<super::downloads::StoredMetadata>(&json) {
                value.publisher.download = value.local_download.map(|record| record.info());
                result.insert(id, value.publisher);
            }
        }
    }
    Ok(result)
}
fn overview(value: &PodcastMetadata) -> Option<String> {
    static TAGS: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| regex::Regex::new("<[^>]*>").unwrap());
    value.description.as_deref().map(|text| {
        TAGS.replace_all(text, " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&nbsp;", " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    })
}
/// Apply publisher metadata to catalog cards.
pub fn apply_summaries(
    conn: &mut SqliteConnection,
    items: &mut [MediaItemSummary],
) -> Result<()> {
    let ids = items
        .iter()
        .filter(|i| i.item_type.starts_with("podcast"))
        .map(|i| i.id)
        .collect::<Vec<_>>();
    if ids.is_empty() {
        return Ok(());
    }
    let values = load_metadata(conn, &ids)?;
    for item in items {
        if let Some(value) = values.get(&item.id) {
            item.overview = overview(value);
            item.genres = value.categories.clone();
            item.display_subtitle = value.author.clone();
            item.has_metadata = true;
            item.artwork_updated_at = item.modified_at;
            item.release_year = value
                .published_at
                .and_then(|v| chrono::DateTime::from_timestamp(v, 0))
                .map(|v| chrono::Datelike::year(&v));
            item.podcast = Some(Box::new(value.clone()));
        }
    }
    Ok(())
}
/// Apply publisher metadata and refresh status to an item page.
pub fn apply_detail(
    conn: &mut SqliteConnection,
    item: &mut MediaItemDetail,
) -> Result<()> {
    if !item.item_type.starts_with("podcast") {
        return Ok(());
    }
    if let Some(value) = load_metadata(conn, &[item.id])?.remove(&item.id) {
        item.overview = overview(&value);
        item.genres = value.categories.clone();
        item.poster_url = value.artwork_url.clone();
        item.has_metadata = true;
        item.content_rating = value
            .explicit
            .map(|v| if v { "Explicit" } else { "Clean" }.to_string());
        let mut value = value;
        if let Some((checked, error)) = feeds::table
            .find(value.podcast_id)
            .select((feeds::checked_at, feeds::last_error))
            .first::<(i64, Option<String>)>(conn)
            .optional()?
        {
            value.checked_at = Some(checked);
            value.last_error = error;
        }
        item.podcast = Some(Box::new(value));
    }
    Ok(())
}
/// A public enclosure streams through Koko so HTTPS clients can also play HTTP publishers.
pub fn playback(
    conn: &mut SqliteConnection,
    id: i32,
) -> Result<Option<PlaybackDecision>> {
    let enclosure = enclosure(conn, id)?;
    Ok(enclosure.map(|(_, mime)| PlaybackDecision {
        item_id: id,
        can_direct_play: true,
        transcode_required: false,
        reason: "Play a saved podcast episode; download a local copy before first playback".into(),
        stream_url: Some(format!("/api/v1/podcasts/episodes/{id}/stream")),
        mime_type: mime,
        transcode_container: None,
        transcode_video_codec: None,
        transcode_audio_codec: None,
        video_transcode_required: false,
        audio_transcode_required: false,
        source_video_codec: None,
        source_audio_codec: None,
        source_container: None,
    }))
}
pub(super) fn enclosure(
    conn: &mut SqliteConnection,
    id: i32,
) -> Result<Option<(String, Option<String>)>> {
    let row = metadata::table
        .find(id)
        .select((metadata::enclosure_url, metadata::mime_type))
        .first::<(Option<String>, Option<String>)>(conn)
        .optional()?;
    Ok(row.and_then(|(url, mime)| url.map(|url| (url, mime))))
}

/// Resolve publisher cover metadata without hydrating unrelated local media sources.
pub(crate) fn artwork_url(
    conn: &mut SqliteConnection,
    id: i32,
) -> Result<Option<String>> {
    Ok(load_metadata(conn, &[id])?
        .remove(&id)
        .and_then(|value| value.artwork_url))
}

/// Return all episode identifiers for one shared podcast across its real seasons.
pub(super) fn episode_ids(
    conn: &mut SqliteConnection,
    podcast: i32,
) -> Result<Vec<i32>> {
    metadata::table
        .filter(metadata::podcast_id.eq(podcast))
        .filter(metadata::enclosure_url.is_not_null())
        .select(metadata::media_item_id)
        .load(conn)
}

fn episode_parent(
    conn: &mut SqliteConnection,
    library_id: i32,
    root: i32,
    season: Option<i32>,
    published: Option<i64>,
    show: &PodcastMetadata,
) -> Result<i32> {
    let year = published
        .and_then(|value| chrono::DateTime::from_timestamp(value, 0))
        .map(|value| chrono::Datelike::year(&value));
    let (identity, title, kind) = if let Some(number) = season {
        (
            format!("podcast:{root}:season:{number}"),
            format!("Season {number}"),
            super::PodcastSeasonKind::Number,
        )
    } else {
        (
            format!(
                "podcast:{root}:year:{}",
                year.map(|year| year.to_string())
                    .unwrap_or_else(|| "unknown".into())
            ),
            year.map(|year| year.to_string())
                .unwrap_or_else(|| "Unknown year".into()),
            super::PodcastSeasonKind::Year,
        )
    };
    let parent = item(
        conn,
        library_id,
        identity,
        &title,
        "podcast_season",
        Some(root),
        None,
    )?;
    diesel::update(items::table.find(parent))
        .set(items::season_number.eq(season))
        .execute(conn)?;
    let mut value = show.clone();
    value.published_at = None;
    value.season_kind = Some(kind);
    value.season_year = if season.is_none() { year } else { None };
    save_metadata(conn, parent, root, &value, None, None)?;
    Ok(parent)
}

/// Group previously imported unseasoned episodes locally without changing episode IDs or history.
pub(super) fn reconcile_year_seasons(conn: &mut SqliteConnection) -> Result<()> {
    let rows = items::table
        .inner_join(metadata::table)
        .filter(items::item_type.eq("podcast_episode"))
        .select((
            items::id,
            items::library_id,
            items::parent_id,
            items::season_number,
            metadata::podcast_id,
            metadata::metadata_json,
        ))
        .load::<(i32, i32, Option<i32>, Option<i32>, i32, String)>(conn)?;
    conn.immediate_transaction(|conn| {
        let mut roots = HashSet::new();
        for (id, library, parent, season, root, json) in rows {
            if season.is_some() || parent != Some(root) {
                continue;
            }
            let Ok(value) = serde_json::from_str::<PodcastMetadata>(&json) else {
                continue;
            };
            let show = super::downloads::stored(conn, root)?.publisher;
            let parent = episode_parent(conn, library, root, None, value.published_at, &show)?;
            diesel::update(items::table.find(id))
                .set(items::parent_id.eq(parent))
                .execute(conn)?;
            roots.insert(root);
        }
        for root in roots {
            let parents = items::table
                .inner_join(metadata::table)
                .filter(metadata::podcast_id.eq(root))
                .filter(items::item_type.eq_any(["podcast", "podcast_season"]))
                .select(items::id)
                .load::<i32>(conn)?;
            for parent in parents {
                let count = items::table
                    .filter(items::parent_id.eq(parent))
                    .count()
                    .get_result::<i64>(conn)?;
                diesel::update(items::table.find(parent))
                    .set(items::child_count.eq(count as i32))
                    .execute(conn)?;
            }
        }
        Ok(())
    })
}
