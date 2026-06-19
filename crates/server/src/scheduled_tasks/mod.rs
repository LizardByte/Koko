//! Scheduled background task runner.

// modules
mod database_maintenance;
mod metadata_refresh;
mod trash_cleanup;

// lib imports
use chrono::{Datelike, Local, Timelike};
use diesel::ExpressionMethods;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use diesel::SelectableHelper;
use once_cell::sync::Lazy;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};

// local imports
use crate::config::{ScheduledTaskWeekday, ScheduledTasksSettings, current_settings};
use crate::db::DbConn;
use crate::db::models::AppSetting;

const SCHEDULER_INTERVAL_SECONDS: u64 = 60;

static SCHEDULED_TASK_RUNNER_RUNNING: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

/// Future returned by scheduled task implementations.
pub(super) type ScheduledTaskFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;

/// Common behavior for a scheduled task.
pub(super) trait ScheduledTask: Sync {
    /// Stable task identifier used by settings and manual-run routes.
    fn id(&self) -> &'static str;
    /// Human-readable task name used in logs.
    fn name(&self) -> &'static str;
    /// Whether the task is enabled in the current scheduled task settings.
    fn enabled(
        &self,
        settings: &ScheduledTasksSettings,
    ) -> bool;
    /// Minimum number of days between runs, when the task uses scheduler-level last-run state.
    fn interval_days(
        &self,
        _settings: &ScheduledTasksSettings,
    ) -> Option<u32> {
        None
    }
    /// App setting key used to store scheduler-level last-run state.
    fn last_run_key(&self) -> Option<&'static str> {
        None
    }
    /// Run the task immediately.
    fn run_now<'a>(
        &'a self,
        db: &'a DbConn,
        settings: &'a ScheduledTasksSettings,
    ) -> ScheduledTaskFuture<'a>;
    /// Run the task from the scheduler, respecting optional last-run state.
    fn run_scheduled<'a>(
        &'a self,
        db: &'a DbConn,
        settings: &'a ScheduledTasksSettings,
    ) -> ScheduledTaskFuture<'a> {
        Box::pin(async move {
            if let (Some(last_run_key), Some(interval_days)) =
                (self.last_run_key(), self.interval_days(settings))
            {
                let should_run =
                    scheduled_task_interval_is_due(db, last_run_key, interval_days).await?;
                if !should_run {
                    return Ok(());
                }
            }

            self.run_now(db, settings).await
        })
    }
    /// Run the task from a manual request, ignoring the scheduled window.
    fn run_manual<'a>(
        &'a self,
        db: &'a DbConn,
    ) -> ScheduledTaskFuture<'a> {
        Box::pin(async move {
            let settings = current_settings();
            self.run_now(db, &settings.scheduled_tasks).await
        })
    }
}

/// Start the scheduled background task runner.
pub fn start_scheduled_task_runner(db: DbConn) {
    if SCHEDULED_TASK_RUNNER_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    rocket::tokio::spawn(async move {
        loop {
            let settings = current_settings();
            if scheduled_tasks_can_start_now(&settings.scheduled_tasks) {
                for task in scheduled_tasks() {
                    if !task.enabled(&settings.scheduled_tasks) {
                        continue;
                    }

                    if let Err(error) = task.run_scheduled(&db, &settings.scheduled_tasks).await {
                        log::error!("Scheduled {} failed: {}", task.name(), error);
                    }
                }
            }

            rocket::tokio::time::sleep(rocket::tokio::time::Duration::from_secs(
                SCHEDULER_INTERVAL_SECONDS,
            ))
            .await;
        }
    });
}

/// Start a metadata refresh task immediately, outside the scheduled window.
pub fn start_metadata_refresh_task(db: DbConn) {
    start_task(metadata_refresh::task(), db);
}

/// Start a trash cleanup task immediately, outside the scheduled window.
pub fn start_trash_cleanup_task(db: DbConn) {
    start_task(trash_cleanup::task(), db);
}

/// Start database maintenance immediately, outside the scheduled window.
pub fn start_database_maintenance_task(db: DbConn) {
    start_task(database_maintenance::task(), db);
}

fn scheduled_tasks() -> [&'static dyn ScheduledTask; 3] {
    [
        metadata_refresh::task(),
        trash_cleanup::task(),
        database_maintenance::task(),
    ]
}

fn start_task(
    task: &'static dyn ScheduledTask,
    db: DbConn,
) {
    rocket::tokio::spawn(async move {
        log::info!(
            "Starting manual scheduled task {} ({})",
            task.name(),
            task.id()
        );
        if let Err(error) = task.run_manual(&db).await {
            log::error!("Manual {} failed: {}", task.name(), error);
        }
    });
}

fn scheduled_tasks_can_start_now(settings: &ScheduledTasksSettings) -> bool {
    if !settings.enabled {
        return false;
    }

    let now = Local::now();
    let weekday = scheduled_weekday_from_chrono(now.weekday());
    if !settings.window.weekdays.iter().any(|day| day == &weekday) {
        return false;
    }

    let Some(start) = parse_minutes_since_midnight(&settings.window.start_time) else {
        return false;
    };
    let Some(stop) = parse_minutes_since_midnight(&settings.window.stop_time) else {
        return false;
    };
    let current = now.hour() * 60 + now.minute();

    if start == stop {
        return true;
    }
    if start < stop {
        current >= start && current < stop
    } else {
        current >= start || current < stop
    }
}

fn scheduled_weekday_from_chrono(weekday: chrono::Weekday) -> ScheduledTaskWeekday {
    match weekday {
        chrono::Weekday::Mon => ScheduledTaskWeekday::Monday,
        chrono::Weekday::Tue => ScheduledTaskWeekday::Tuesday,
        chrono::Weekday::Wed => ScheduledTaskWeekday::Wednesday,
        chrono::Weekday::Thu => ScheduledTaskWeekday::Thursday,
        chrono::Weekday::Fri => ScheduledTaskWeekday::Friday,
        chrono::Weekday::Sat => ScheduledTaskWeekday::Saturday,
        chrono::Weekday::Sun => ScheduledTaskWeekday::Sunday,
    }
}

fn parse_minutes_since_midnight(value: &str) -> Option<u32> {
    let (hour, minute) = value.trim().split_once(':')?;
    let hour = hour.parse::<u32>().ok()?;
    let minute = minute.parse::<u32>().ok()?;
    (hour < 24 && minute < 60).then_some(hour * 60 + minute)
}

async fn scheduled_task_interval_is_due(
    db: &DbConn,
    last_run_key: &'static str,
    interval_days: u32,
) -> Result<bool, String> {
    let interval_seconds = i64::from(interval_days).saturating_mul(24 * 60 * 60);
    let now = crate::utils::current_timestamp();

    db.run(move |conn| {
        let last_run = load_scheduled_task_last_run(conn, last_run_key)?;
        Ok::<bool, diesel::result::Error>(
            last_run
                .map(|last_run| now.saturating_sub(last_run) >= interval_seconds)
                .unwrap_or(true),
        )
    })
    .await
    .map_err(|error| error.to_string())
}

pub(super) fn load_scheduled_task_last_run(
    conn: &mut rocket_sync_db_pools::diesel::SqliteConnection,
    setting_key: &str,
) -> Result<Option<i64>, diesel::result::Error> {
    use crate::db::schema::app_settings::dsl as app_settings_dsl;

    app_settings_dsl::app_settings
        .filter(app_settings_dsl::key.eq(setting_key))
        .select(AppSetting::as_select())
        .first::<AppSetting>(conn)
        .optional()
        .map(|row| row.and_then(|row| row.value.parse::<i64>().ok()))
}

pub(super) fn save_scheduled_task_last_run(
    conn: &mut rocket_sync_db_pools::diesel::SqliteConnection,
    setting_key: &str,
    timestamp: i64,
) -> Result<(), diesel::result::Error> {
    use crate::db::schema::app_settings::dsl as app_settings_dsl;

    diesel::insert_into(app_settings_dsl::app_settings)
        .values(AppSetting {
            key: setting_key.to_string(),
            value: timestamp.to_string(),
            updated_at: Some(timestamp),
        })
        .on_conflict(app_settings_dsl::key)
        .do_update()
        .set((
            app_settings_dsl::value.eq(diesel::upsert::excluded(app_settings_dsl::value)),
            app_settings_dsl::updated_at.eq(diesel::upsert::excluded(app_settings_dsl::updated_at)),
        ))
        .execute(conn)
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::parse_minutes_since_midnight;

    #[test]
    fn parse_minutes_rejects_invalid_time_values() {
        assert_eq!(parse_minutes_since_midnight("02:30"), Some(150));
        assert_eq!(parse_minutes_since_midnight("24:00"), None);
        assert_eq!(parse_minutes_since_midnight("02:60"), None);
        assert_eq!(parse_minutes_since_midnight("nope"), None);
    }
}
