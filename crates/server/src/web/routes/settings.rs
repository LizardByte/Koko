//! Settings and library-management routes.

// lib imports
use chrono::{DateTime, Local, LocalResult, NaiveDate, NaiveDateTime, TimeZone};
use once_cell::sync::Lazy;
use regex::Regex;
use rocket::delete;
use rocket::get;
use rocket::http::Status;
use rocket::post;
use rocket::put;
use rocket::serde::json::Json;
use rocket_okapi::openapi;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// local imports
use crate::config::{
    MediaLibrarySettings, Settings, current_settings, replace_current_settings,
    save_database_settings, save_settings, settings_file_path,
};
use crate::db::DbConn;
use crate::globals;
use crate::logging::{normalize_display_path, normalize_log_source_path};
use crate::media::{
    add_library_setting, count_persisted_libraries, list_library_settings, remove_library_setting,
    replace_library_settings,
};

static STRUCTURED_LOG_LINE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?P<timestamp>\S+) \[(?P<level>[^]]+)\] \[(?P<module>[^]]+)\] \[(?P<source>[^]]+)\] (?P<message>.*)$")
        .expect("Failed to compile structured log regex")
});

/// Settings response payload.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SettingsResponse {
    /// Current settings snapshot.
    pub settings: Settings,
    /// Path to the YAML settings file.
    pub settings_path: String,
}

/// Metadata cache clear response.
#[derive(Debug, Serialize, JsonSchema)]
pub struct MetadataCacheClearResponse {
    /// Number of cache files removed.
    pub removed_files: usize,
}

/// Scheduled task manual run response.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ScheduledTaskRunResponse {
    /// Scheduled task identifier.
    pub task_id: String,
    /// Whether the task was accepted for background execution.
    pub started: bool,
    /// Human-readable status message.
    pub message: String,
}

/// Add-library request payload.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddLibraryRequest {
    /// New library configuration.
    pub library: MediaLibrarySettings,
}

/// One structured log entry parsed from the application log file.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct LogEntry {
    /// Original timestamp string from the log file.
    pub timestamp: String,
    /// Log level such as `INFO` or `WARN`.
    pub level: String,
    /// Module path emitted by the logger.
    pub module: String,
    /// Source file path for the log entry.
    pub source_file_path: String,
    /// Source line number, when available.
    pub line_number: Option<u32>,
    /// Human-readable log message.
    pub message: String,
}

/// Structured log response for the settings page.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct LogEntriesResponse {
    /// Path to the active application log file.
    pub log_path: String,
    /// Parsed log entries matching the request filters.
    pub entries: Vec<LogEntry>,
}

fn merged_settings_response(
    settings: Settings,
    libraries: Vec<MediaLibrarySettings>,
) -> SettingsResponse {
    let mut merged = settings;
    merged.media.libraries = libraries;
    SettingsResponse {
        settings: merged,
        settings_path: normalize_display_path(&settings_file_path().to_string_lossy()),
    }
}

fn persist_bootstrap_settings(settings: &Settings) -> Result<(), Status> {
    save_settings(settings).map_err(|error| {
        log::error!("Failed to save settings: {}", error);
        Status::InternalServerError
    })
}

fn parse_log_source(source: &str) -> (String, Option<u32>) {
    let trimmed = normalize_log_source_path(source);
    let Some((path, line)) = trimmed.rsplit_once(':') else {
        return (trimmed.to_string(), None);
    };

    (path.to_string(), line.trim().parse::<u32>().ok())
}

fn parse_log_entry_timestamp(value: &str) -> Option<DateTime<chrono::FixedOffset>> {
    DateTime::parse_from_rfc3339(value.trim()).ok()
}

fn parse_log_filter_timestamp(value: Option<&str>) -> Option<DateTime<chrono::FixedOffset>> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }

    if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
        return Some(parsed);
    }

    for format in [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d",
    ] {
        if format == "%Y-%m-%d" {
            if let Ok(parsed) = NaiveDate::parse_from_str(value, format) {
                let Some(naive) = parsed.and_hms_opt(0, 0, 0) else {
                    continue;
                };
                return match Local.from_local_datetime(&naive) {
                    LocalResult::Single(date_time) => Some(date_time.fixed_offset()),
                    LocalResult::Ambiguous(date_time, _) => Some(date_time.fixed_offset()),
                    LocalResult::None => None,
                };
            }
            continue;
        }

        if let Ok(parsed) = NaiveDateTime::parse_from_str(value, format) {
            return match Local.from_local_datetime(&parsed) {
                LocalResult::Single(date_time) => Some(date_time.fixed_offset()),
                LocalResult::Ambiguous(date_time, _) => Some(date_time.fixed_offset()),
                LocalResult::None => None,
            };
        }
    }

    None
}

fn read_structured_log_entries(
    level: Option<&str>,
    module: Option<&str>,
    search: Option<&str>,
    since: Option<&str>,
    until: Option<&str>,
    limit: usize,
) -> Vec<LogEntry> {
    let contents = std::fs::read_to_string(&globals::APP_PATHS.log_path).unwrap_or_default();
    let level_filter = level
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let module_filter = module
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let search_filter = search
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let since_filter = parse_log_filter_timestamp(since);
    let until_filter = parse_log_filter_timestamp(until);

    let mut entries = contents
        .lines()
        .filter_map(|line| {
            let captures = STRUCTURED_LOG_LINE_REGEX.captures(line)?;
            let timestamp = captures.name("timestamp")?.as_str().to_string();
            let level = captures.name("level")?.as_str().to_string();
            let module_name = captures.name("module")?.as_str().to_string();
            let source = captures.name("source")?.as_str().to_string();
            let message = captures.name("message")?.as_str().to_string();
            let (source_file_path, line_number) = parse_log_source(&source);

            Some(LogEntry {
                timestamp,
                level,
                module: module_name,
                source_file_path,
                line_number,
                message,
            })
        })
        .filter(|entry| {
            let level_matches = level_filter
                .as_ref()
                .map(|filter| entry.level.to_ascii_lowercase() == *filter)
                .unwrap_or(true);
            let module_matches = module_filter
                .as_ref()
                .map(|filter| entry.module.to_ascii_lowercase().contains(filter))
                .unwrap_or(true);
            let search_matches = search_filter
                .as_ref()
                .map(|filter| {
                    entry.message.to_ascii_lowercase().contains(filter)
                        || entry.module.to_ascii_lowercase().contains(filter)
                        || entry.source_file_path.to_ascii_lowercase().contains(filter)
                })
                .unwrap_or(true);
            let timestamp_matches = parse_log_entry_timestamp(&entry.timestamp)
                .map(|timestamp| {
                    let after_since = since_filter
                        .as_ref()
                        .map(|filter| timestamp >= *filter)
                        .unwrap_or(true);
                    let before_until = until_filter
                        .as_ref()
                        .map(|filter| timestamp <= *filter)
                        .unwrap_or(true);
                    after_since && before_until
                })
                .unwrap_or(since_filter.is_none() && until_filter.is_none());
            level_matches && module_matches && search_matches && timestamp_matches
        })
        .collect::<Vec<_>>();

    entries.reverse();
    entries.truncate(limit);
    entries
}

/// Return the current server settings snapshot.
#[openapi(tag = "Settings")]
#[get("/api/v1/settings")]
pub async fn get_settings(db: DbConn) -> Result<Json<SettingsResponse>, Status> {
    let settings = current_settings();
    let legacy_libraries = settings.media.libraries.clone();
    let libraries = db
        .run(move |conn| list_library_settings(conn, &legacy_libraries))
        .await
        .map_err(|error| {
            log::error!("Failed to load persisted library settings: {}", error);
            Status::InternalServerError
        })?;

    persist_bootstrap_settings(&settings)?;

    Ok(Json(merged_settings_response(settings, libraries)))
}

/// Clear cached provider metadata responses.
#[openapi(tag = "Settings")]
#[post("/api/v1/settings/metadata-cache/clear")]
pub fn clear_metadata_cache() -> Result<Json<MetadataCacheClearResponse>, Status> {
    let data_dir = current_settings().general.data_dir;
    let removed_files =
        crate::metadata::clear_metadata_response_cache(&data_dir).map_err(|error| {
            log::error!("Failed to clear metadata response cache: {}", error);
            Status::InternalServerError
        })?;
    Ok(Json(MetadataCacheClearResponse { removed_files }))
}

/// Start one scheduled task immediately.
#[openapi(tag = "Settings")]
#[post("/api/v1/scheduled-tasks/<task_id>/run")]
pub fn run_scheduled_task(
    db: DbConn,
    task_id: &str,
) -> Result<Json<ScheduledTaskRunResponse>, Status> {
    let message = match task_id {
        "metadata_refresh" => {
            crate::scheduled_tasks::start_metadata_refresh_task(db);
            "Metadata refresh started"
        }
        "trash_cleanup" => {
            crate::scheduled_tasks::start_trash_cleanup_task(db);
            "Trash cleanup started"
        }
        "database_maintenance" => {
            crate::scheduled_tasks::start_database_maintenance_task(db);
            "Database maintenance started"
        }
        _ => return Err(Status::NotFound),
    };

    Ok(Json(ScheduledTaskRunResponse {
        task_id: task_id.to_string(),
        started: true,
        message: message.to_string(),
    }))
}

/// Return structured application logs for the settings page.
#[openapi(tag = "Settings")]
#[get("/api/v1/settings/logs?<level>&<module>&<search>&<since>&<until>&<limit>")]
pub fn get_logs(
    level: Option<&str>,
    module: Option<&str>,
    search: Option<&str>,
    since: Option<&str>,
    until: Option<&str>,
    limit: Option<usize>,
) -> Json<LogEntriesResponse> {
    Json(LogEntriesResponse {
        log_path: normalize_display_path(&globals::APP_PATHS.log_path),
        entries: read_structured_log_entries(
            level,
            module,
            search,
            since,
            until,
            limit.unwrap_or(200).clamp(1, 500),
        ),
    })
}

/// Replace the full settings snapshot and persist it to disk.
#[openapi(tag = "Settings")]
#[put("/api/v1/settings", format = "json", data = "<settings>")]
pub async fn update_settings(
    db: DbConn,
    settings: Json<Settings>,
) -> Result<Json<SettingsResponse>, Status> {
    let settings = settings.into_inner();
    let libraries = settings.media.libraries.clone();
    let settings_for_database = settings.clone();
    let persisted_libraries = db
        .run(move |conn| {
            let existing_count =
                count_persisted_libraries(conn).map_err(|error| error.to_string())? as usize;
            let persisted_libraries = if libraries.len() < existing_count {
                log::warn!(
                    "Preserving {} persisted media libraries omitted from settings update; use the library delete route to remove libraries",
                    existing_count - libraries.len()
                );
                let mut merged = list_library_settings(conn, &[])
                    .map_err(|error| error.to_string())?;
                for (index, library) in libraries.iter().cloned().enumerate() {
                    if let Some(existing) = merged.get_mut(index) {
                        *existing = library;
                    }
                }
                replace_library_settings(conn, &merged).map_err(|error| error.to_string())?
            } else {
                replace_library_settings(conn, &libraries).map_err(|error| error.to_string())?
            };
            save_database_settings(conn, &settings_for_database)?;
            Ok::<_, String>(persisted_libraries)
        })
        .await
        .map_err(|error| {
            log::error!("Failed to replace persisted library settings: {}", error);
            Status::InternalServerError
        })?;

    persist_bootstrap_settings(&settings)?;
    let mut runtime_settings = settings.clone();
    runtime_settings.media.libraries.clear();
    replace_current_settings(runtime_settings);

    Ok(Json(merged_settings_response(
        settings,
        persisted_libraries,
    )))
}

/// Append a new library to the persisted media-library settings.
#[openapi(tag = "Settings")]
#[post("/api/v1/settings/libraries", format = "json", data = "<request>")]
pub async fn add_library(
    db: DbConn,
    request: Json<AddLibraryRequest>,
) -> Result<Json<SettingsResponse>, Status> {
    let mut library = request.into_inner().library;
    library.normalize();

    let libraries = db
        .run(move |conn| add_library_setting(conn, &library))
        .await
        .map_err(|error| {
            log::error!("Failed to add persisted library setting: {}", error);
            Status::InternalServerError
        })?;

    let settings = current_settings();
    persist_bootstrap_settings(&settings)?;

    Ok(Json(merged_settings_response(settings, libraries)))
}

/// Remove one configured library from the database and return the merged settings snapshot.
#[openapi(tag = "Settings")]
#[delete("/api/v1/settings/libraries/<library_index>")]
pub async fn remove_library(
    db: DbConn,
    library_index: usize,
) -> Result<Json<SettingsResponse>, Status> {
    let removed = db
        .run(move |conn| remove_library_setting(conn, library_index))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to remove persisted library at index {}: {}",
                library_index,
                error
            );
            Status::InternalServerError
        })?;
    if !removed {
        return Err(Status::NotFound);
    }

    let settings = current_settings();
    let libraries = db
        .run(|conn| list_library_settings(conn, &[]))
        .await
        .map_err(|error| {
            log::error!(
                "Failed to reload persisted libraries after removal: {}",
                error
            );
            Status::InternalServerError
        })?;

    persist_bootstrap_settings(&settings)?;

    Ok(Json(merged_settings_response(settings, libraries)))
}
