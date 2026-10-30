//! Shared podcast feeds with account-owned subscriptions and ordinary media playback history.
mod discovery;
mod downloads;
mod feed;
mod http;
mod preferences;
mod storage;

pub use discovery::{
    DirectoryPodcast,
    discover,
};
pub(crate) use downloads::ensure as download_episode;
pub use downloads::{
    DownloadInfo,
    DownloadState,
};
pub(crate) use http::{
    normalize_url,
    request,
};
pub use preferences::{
    PlaybackOrder,
    PodcastUserSettings,
};
pub(crate) use preferences::{
    apply as apply_preferences,
    apply_detail as apply_user_context,
    continuation,
    episodes,
    filter_home,
    save_user_settings,
    set as set_order,
    user_settings,
};
pub use storage::{
    PodcastEpisodePage,
    apply_detail,
    apply_summaries,
    can_access,
    filter_visible,
    playback,
    subscribed,
};
pub(crate) use storage::{
    artwork_url,
    episode_page,
    feed_for_user,
    has_subscription,
};

use crate::db::DbConn;

use rocket_okapi::okapi::schemars::{
    self,
    JsonSchema,
};
use serde::{
    Deserialize,
    Serialize,
};

const MAX_FEED_BYTES: usize = 64 * 1024 * 1024;

/// Origin of a podcast episode grouping.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PodcastSeasonKind {
    /// An explicit season number supplied by the publisher.
    Number,
    /// A publication-year grouping for episodes without a publisher season.
    Year,
}

/// Publisher-provided metadata retained from a podcast feed.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PodcastMetadata {
    /// Shared root podcast item identifier.
    pub podcast_id: i32,
    /// Current account's chronological playback order.
    #[serde(default)]
    pub playback_order: PlaybackOrder,
    /// Whether this season came from publisher numbering or publication year.
    #[serde(default)]
    pub season_kind: Option<PodcastSeasonKind>,
    /// Publication year for year-based seasons; absent for unknown dates and numbered seasons.
    #[serde(default)]
    pub season_year: Option<i32>,
    /// Shared local episode availability, without exposing filesystem paths.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download: Option<DownloadInfo>,
    /// Public subscription feed URL, when this is the root show.
    pub feed_url: Option<String>,
    /// Last feed refresh failure, when available.
    pub last_error: Option<String>,
    /// Last feed check as Unix seconds.
    pub checked_at: Option<i64>,
    /// Publisher or author name.
    pub author: Option<String>,
    /// Publisher description or episode show notes.
    pub description: Option<String>,
    /// Publisher cover image URL.
    pub artwork_url: Option<String>,
    /// Publisher website for the show or episode.
    pub website_url: Option<String>,
    /// Publisher language tag.
    pub language: Option<String>,
    /// Publisher category labels.
    pub categories: Vec<String>,
    /// Whether the publisher marks this content explicit.
    pub explicit: Option<bool>,
    /// Publisher copyright notice.
    pub copyright: Option<String>,
    /// Publisher ordering mode, typically episodic or serial.
    pub show_type: Option<String>,
    /// Full, bonus, or trailer episode type.
    pub episode_type: Option<String>,
    /// Publication time as Unix seconds.
    pub published_at: Option<i64>,
    /// Publisher episode identity.
    pub guid: Option<String>,
    /// Publisher transcript links.
    pub transcripts: Vec<PodcastLink>,
    /// Publisher chapter document URL.
    pub chapters_url: Option<String>,
    /// Named participants supplied by the publisher.
    pub people: Vec<String>,
}

/// A publisher-supplied transcript link with its language and media type.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PodcastLink {
    /// Public document URL.
    pub url: String,
    /// Publisher media type.
    pub mime_type: Option<String>,
    /// Publisher language tag.
    pub language: Option<String>,
}

/// Fetch a feed outside database transactions, then atomically reuse its content and subscribe.
pub async fn subscribe(
    db: &DbConn,
    user: i32,
    url: &str,
) -> Result<i32, String> {
    let requested = http::normalize_url(url)?;
    let _mutation = SUBSCRIPTION_MUTATIONS.lock().await;
    let existing_url = requested.clone();
    // A recently checked feed can be subscribed to even while its publisher is unavailable.
    if let Some(id) = db
        .run(move |conn| storage::subscribe_existing(conn, user, &existing_url))
        .await
        .map_err(|_| "Unable to save podcast subscription")?
    {
        return Ok(id);
    }
    let (canonical, xml) = http::fetch_text(&requested, MAX_FEED_BYTES).await?;
    let parsed = feed::parse(&xml, &canonical)?;
    db.run(move |conn| storage::save(conn, Some(user), &requested, &canonical, &parsed))
        .await
        .map_err(|_| "Unable to save podcast feed".into())
}

/// Refresh a subscribed feed while preserving episode identities and all listening history.
pub async fn refresh(
    db: &DbConn,
    user: i32,
    id: i32,
) -> Result<(), String> {
    let url = db
        .run(move |conn| storage::feed_for_user(conn, user, id))
        .await
        .map_err(|_| "Podcast is unavailable")?;
    refresh_url(db, id, url).await
}

async fn refresh_url(
    db: &DbConn,
    id: i32,
    url: String,
) -> Result<(), String> {
    let result = async {
        let (canonical, xml) = http::fetch_text(&url, MAX_FEED_BYTES).await?;
        let parsed = feed::parse(&xml, &canonical)?;
        db.run(move |conn| storage::save(conn, None, &url, &canonical, &parsed))
            .await
            .map_err(|_| "Unable to update podcast feed".to_string())?;
        Ok(())
    }
    .await;
    let error = result.as_ref().err().cloned();
    db.run(move |conn| storage::record_check(conn, id, error))
        .await
        .map_err(|_| "Unable to save podcast refresh status")?;
    result
}

/// Poll only feeds with subscribers, at most hourly; a failed publisher cannot block other feeds.
pub async fn refresh_due(db: &DbConn) {
    if !crate::config::current_settings().podcasts.enabled {
        return;
    }
    let feeds = match db.run(storage::due).await {
        Ok(feeds) => feeds,
        Err(error) => {
            log::warn!("Unable to load podcast refresh queue: {error}");
            return;
        }
    };
    for (id, url) in feeds {
        if !crate::config::current_settings().podcasts.enabled {
            break;
        }
        if let Err(error) = refresh_url(db, id, url).await {
            log::warn!("Podcast {id} refresh failed: {error}");
        }
    }
}

#[cfg(test)]
mod tests;

/// Run feed checks independently of maintenance windows and stop when this server shuts down.
pub(crate) fn start_refresh_runner(
    db: DbConn,
    mut shutdown: rocket::Shutdown,
) {
    rocket::tokio::spawn(async move {
        let checks = async {
            if let Err(error) = db.run(storage::reconcile_year_seasons).await {
                log::warn!("Unable to group existing podcast episodes by year: {error}");
            }
            loop {
                tokio::join!(refresh_due(&db), downloads::download_pending(&db));
                rocket::tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            }
        };
        rocket::tokio::select! { _ = checks => {}, _ = &mut shutdown => {} }
    });
}

static SUBSCRIPTION_MUTATIONS: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static SHOW_LOCKS: once_cell::sync::Lazy<
    std::sync::Mutex<std::collections::HashMap<i32, std::sync::Weak<tokio::sync::RwLock<()>>>>,
> = once_cell::sync::Lazy::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

pub(super) fn show_lock(id: i32) -> std::sync::Arc<tokio::sync::RwLock<()>> {
    let mut locks = SHOW_LOCKS.lock().expect("Podcast show lock poisoned");
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(&id).and_then(std::sync::Weak::upgrade) {
        return lock;
    }
    let lock = std::sync::Arc::new(tokio::sync::RwLock::new(()));
    locks.insert(id, std::sync::Arc::downgrade(&lock));
    lock
}

/// Preserve other subscribers' saved copies and remove files after the final subscription ends.
pub(crate) async fn remove_subscription(
    db: &DbConn,
    user: i32,
    id: i32,
) -> Result<(), String> {
    use crate::db::schema::podcast_subscriptions as subscriptions;
    use diesel::prelude::*;
    let _mutation = SUBSCRIPTION_MUTATIONS.lock().await;
    let lock = show_lock(id);
    let _show = lock.write().await;
    let remaining = db
        .run(move |conn| {
            storage::feed_for_user(conn, user, id)?;
            subscriptions::table
                .filter(subscriptions::podcast_id.eq(id))
                .count()
                .get_result::<i64>(conn)
        })
        .await
        .map_err(|_| "Unable to find podcast subscription")?;
    if remaining == 1 {
        downloads::cleanup_unsubscribed(db, id).await?;
    }
    db.run(move |conn| storage::unsubscribe(conn, user, id))
        .await
        .map_err(|_| "Unable to remove podcast subscription")?;
    Ok(())
}
