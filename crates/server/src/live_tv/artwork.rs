//! Optional movie/series artwork with conservative matching and a bounded persistent URL cache.
#[cfg(test)]
mod tests;
use super::{
    Program,
    Source,
    sources,
};
use crate::config::{
    MetadataProviderId,
    MetadataSettings,
};
use crate::db::{
    DbConn,
    schema::{
        app_settings as a,
        live_tv_programs as p,
    },
};
use crate::metadata::{
    MetadataRegistry,
    MetadataSearchResult,
    providers::MetadataProvider,
};
use diesel::prelude::*;
use serde::{
    Deserialize,
    Serialize,
};
use sha2::{
    Digest,
    Sha256,
};
use std::collections::HashMap;
use std::sync::{
    Arc,
    Mutex,
    Weak,
};

const PREFIX: &str = "live_tv_artwork:";
const PREFIX_PATTERN: &str = "live\\_tv\\_artwork:%";
const CACHE_LIMIT: i64 = 2000;
static REQUESTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);
static LOCKS: once_cell::sync::Lazy<Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Artwork {
    pub poster: Option<String>,
    pub image: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct CacheEntry {
    expires_at: i64,
    artwork: Option<Artwork>,
}

fn lock(key: &str) -> Arc<tokio::sync::Mutex<()>> {
    let mut locks = LOCKS.lock().expect("TV artwork locks poisoned");
    locks.retain(|_, value| value.strong_count() > 0);
    if let Some(lock) = locks.get(key).and_then(Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(tokio::sync::Mutex::new(()));
    locks.insert(key.into(), Arc::downgrade(&lock));
    lock
}

pub(crate) fn configured(
    settings: &MetadataSettings,
    mode: &str,
) -> MetadataSettings {
    let mut settings = settings.clone();
    settings.providers.retain_mut(|provider| {
        provider.enabled
            && matches!(
                provider.id,
                MetadataProviderId::Tmdb | MetadataProviderId::Tvdb
            )
            && matches!(mode, "" | "automatic" | "tmdb" | "tvdb")
            && (matches!(mode, "" | "automatic") || provider.id.as_storage_value() == mode)
            && crate::config::resolve_metadata_provider_api_key(provider).is_ok()
            && provider
                .api_key
                .as_ref()
                .is_some_and(|key| !key.trim().is_empty())
    });
    settings
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn movie(program: &Program) -> bool {
    program
        .metadata
        .episode_id
        .as_ref()
        .is_some_and(|id| id.starts_with("MV"))
        || program.metadata.categories.iter().any(|value| {
            let value = value.to_lowercase();
            value.contains("movie") || value.contains("film")
        })
}

fn eligible(program: &Program) -> bool {
    !program.metadata.categories.iter().any(|value| {
        let value = value.to_lowercase();
        [
            "sport", "news", "weather", "shopping",
        ]
        .iter()
        .any(|word| value.contains(word))
    }) && ![
        "paidprogramming",
        "noscheduledprogramming",
        "offair",
    ]
    .contains(&normalize(&program.title).as_str())
}

fn episode(program: &Program) -> Option<(i32, i32, &str)> {
    let value = program.metadata.episode.as_deref()?;
    let (season, number) = value.strip_prefix('S')?.split_once('E')?;
    Some((
        season.trim().parse().ok()?,
        number.trim().parse().ok()?,
        program
            .metadata
            .subtitle
            .as_deref()
            .filter(|subtitle| !subtitle.trim().is_empty())?,
    ))
}

fn candidates(
    program: &Program,
    results: Vec<MetadataSearchResult>,
    kind: &str,
) -> Vec<MetadataSearchResult> {
    let title = normalize(&program.title);
    let mut results = results
        .into_iter()
        .filter(|result| {
            result.media_type == kind
                && normalize(&result.title) == title
                && (!movie(program)
                    || program.metadata.year.is_none()
                    || result.release_year == program.metadata.year)
        })
        .collect::<Vec<_>>();
    results.sort_by(|a, b| a.external_id.cmp(&b.external_id));
    results.dedup_by(|a, b| a.external_id == b.external_id);
    results
}

async fn lookup(
    provider: &(dyn MetadataProvider + Send + Sync),
    settings: &MetadataSettings,
    program: &Program,
) -> Result<Option<Artwork>, String> {
    let kind = if movie(program) {
        "movie"
    } else if provider.descriptor().id == MetadataProviderId::Tmdb {
        "tv"
    } else {
        "series"
    };
    let results = candidates(
        program,
        provider
            .search(settings, &program.title, Some(kind))
            .await?,
        kind,
    );
    if results.len() > 4 {
        return Ok(None);
    }
    let mut matched = Vec::new();
    for candidate in results {
        if let Some((season, number, subtitle)) = episode(program).filter(|_| kind != "movie") {
            match provider
                .fetch_episode_snapshot(
                    settings,
                    &candidate.external_id,
                    season,
                    number,
                    None,
                    false,
                )
                .await
            {
                Ok(snapshot)
                    if snapshot
                        .title
                        .as_deref()
                        .is_some_and(|title| normalize(title) == normalize(subtitle)) => {}
                _ => continue,
            }
        }
        matched.push(candidate);
    }
    if matched.len() != 1 {
        return Ok(None);
    }
    let candidate = &matched[0];
    let snapshot = provider
        .fetch_snapshot(settings, &candidate.external_id, kind, false)
        .await?;
    let artwork = Artwork {
        poster: snapshot.artwork_url.and_then(valid_image),
        image: snapshot.backdrop_url.and_then(valid_image),
    };
    Ok((artwork.poster.is_some() || artwork.image.is_some()).then_some(artwork))
}

fn valid_image(value: String) -> Option<String> {
    super::formats::http_url(&value)
        .ok()
        .filter(|url| url.username().is_empty() && url.password().is_none())
        .map(|url| url.to_string())
}

fn key(
    program: &Program,
    settings: &MetadataSettings,
) -> String {
    let identity = if let Some(series) = program.metadata.series_id.as_ref() {
        series.clone()
    } else {
        format!(
            "{:?}:{:?}",
            program.metadata.episode, program.metadata.subtitle
        )
    };
    let value = serde_json::to_vec(&(
        normalize(&program.title),
        movie(program),
        program.metadata.year,
        identity,
        &settings.providers,
    ))
    .expect("serializable artwork key");
    let digest = Sha256::digest(value)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{PREFIX}{digest}")
}

fn cached(
    conn: &mut SqliteConnection,
    key: &str,
    now: i64,
) -> QueryResult<Option<Option<Artwork>>> {
    Ok(a::table
        .find(key)
        .select(a::value)
        .first::<String>(conn)
        .optional()?
        .and_then(|value| serde_json::from_str::<CacheEntry>(&value).ok())
        .filter(|entry| entry.expires_at > now)
        .map(|entry| entry.artwork))
}

fn save(
    conn: &mut SqliteConnection,
    key: &str,
    artwork: Option<Artwork>,
    ttl: i64,
    now: i64,
) -> QueryResult<()> {
    let value = serde_json::to_string(&CacheEntry {
        expires_at: now + ttl,
        artwork,
    })
    .expect("serializable artwork cache");
    diesel::insert_into(a::table)
        .values((a::key.eq(key), a::value.eq(&value), a::updated_at.eq(now)))
        .on_conflict(a::key)
        .do_update()
        .set((a::value.eq(&value), a::updated_at.eq(now)))
        .execute(conn)?;
    prune(conn, now)
}

pub(super) fn prune(
    conn: &mut SqliteConnection,
    now: i64,
) -> QueryResult<()> {
    let rows = a::table
        .filter(a::key.like(PREFIX_PATTERN).escape('\\'))
        .select((a::key, a::value))
        .load::<(String, String)>(conn)?;
    let expired = rows
        .into_iter()
        .filter_map(|(key, value)| {
            let current = serde_json::from_str::<CacheEntry>(&value)
                .is_ok_and(|entry| entry.expires_at > now);
            (!current).then_some(key)
        })
        .collect::<Vec<_>>();
    diesel::delete(a::table.filter(a::key.eq_any(expired))).execute(conn)?;
    let excess = a::table
        .filter(a::key.like(PREFIX_PATTERN).escape('\\'))
        .order((a::updated_at.desc(), a::key))
        .offset(CACHE_LIMIT)
        .select(a::key)
        .load::<String>(conn)?;
    diesel::delete(a::table.filter(a::key.eq_any(excess))).execute(conn)?;
    Ok(())
}

async fn enriched(
    db: &DbConn,
    program: &Program,
    settings: MetadataSettings,
) -> Option<Artwork> {
    if !eligible(program) || settings.providers.is_empty() {
        return None;
    }
    let cache_key = key(program, &settings);
    let lock = lock(&cache_key);
    let _guard = lock.lock().await;
    let read_key = cache_key.clone();
    if let Ok(Some(value)) = db
        .run(move |conn| cached(conn, &read_key, super::now()))
        .await
    {
        return value;
    }
    let _permit = REQUESTS.acquire().await.ok()?;
    let registry = MetadataRegistry::new();
    let mut result = None;
    let mut failed = false;
    for configured in &settings.providers {
        let Some(provider) = registry.provider(&configured.id) else {
            continue;
        };
        match lookup(provider, &settings, program).await {
            Ok(Some(artwork)) => {
                result = Some(artwork);
                break;
            }
            Ok(None) => {}
            Err(_) => failed = true,
        }
    }
    let ttl = if result.is_some() {
        7 * 86400
    } else if failed {
        300
    } else {
        86400
    };
    let saved = result.clone();
    let _ = db
        .run(move |conn| save(conn, &cache_key, saved, ttl, super::now()))
        .await;
    result
}

pub(crate) async fn image_url(
    db: &DbConn,
    id: i32,
    poster: bool,
) -> Result<Option<String>, String> {
    let (program, source) = db
        .run(move |conn| {
            let (source, key, start, end, title, metadata) = p::table
                .find(id)
                .select((
                    p::source_id,
                    p::guide_key,
                    p::starts_at,
                    p::ends_at,
                    p::title,
                    p::metadata_json,
                ))
                .first::<(i32, String, i64, i64, String, String)>(conn)?;
            let source = super::storage::source(conn, source)?;
            let program = Program {
                id,
                source_id: source.id,
                guide_key: key,
                starts_at: start,
                ends_at: end,
                title,
                metadata: serde_json::from_str(&metadata).unwrap_or_default(),
            };
            Ok::<_, diesel::result::Error>((program, source))
        })
        .await
        .map_err(super::db_error)?;
    if !source.enabled {
        return Err("This TV source is disabled".into());
    }
    let config = sources::config(&source)?;
    let settings = configured(
        &crate::config::current_settings().metadata,
        &config.artwork_provider,
    );
    let matched = enriched(db, &program, settings).await;
    let preferred = matched
        .and_then(|art| if poster { art.poster.or(art.image) } else { art.image.or(art.poster) });
    let fallback = if poster {
        program.metadata.poster.or(program.metadata.image)
    } else {
        program.metadata.image.or(program.metadata.poster)
    };
    Ok(preferred.or(fallback).and_then(valid_image))
}

/// Decorate artwork URLs without changing guide titles, descriptions, times, or episode identity.
pub(crate) fn decorate(
    programs: &mut [Program],
    sources: &[Source],
    settings: &MetadataSettings,
) {
    let enabled = sources
        .iter()
        .filter_map(|source| {
            let config = sources::config(source).ok()?;
            let settings = configured(settings, &config.artwork_provider);
            (!settings.providers.is_empty()).then_some((
                source.id,
                settings
                    .providers
                    .iter()
                    .map(|provider| provider.id.as_storage_value().to_owned())
                    .collect::<Vec<_>>(),
            ))
        })
        .collect::<HashMap<_, _>>();
    for program in programs {
        if let Some(providers) = enabled
            .get(&program.source_id)
            .filter(|_| eligible(program))
        {
            program.metadata.image =
                Some(format!("/api/v1/live-tv/programs/{}/artwork", program.id));
            program.metadata.poster = Some(format!(
                "/api/v1/live-tv/programs/{}/artwork?kind=poster",
                program.id
            ));
            program.metadata.artwork_sources = providers.clone();
        }
    }
}
