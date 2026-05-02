//! Scheduled background task runner.

// lib imports
use chrono::{Datelike, Local, Timelike};
use diesel::ExpressionMethods;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use diesel::SelectableHelper;
use diesel::connection::SimpleConnection;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicBool, Ordering};

// local imports
use crate::config::{
    ScheduledTaskWeekday, ScheduledTasksSettings, TrashCleanupTaskSettings, current_settings,
};
use crate::db::DbConn;
use crate::db::models::AppSetting;
use crate::media::delete_missing_media_items;
use crate::utils::current_timestamp;

const SCHEDULER_INTERVAL_SECONDS: u64 = 60;
const TRASH_CLEANUP_LAST_RUN_KEY: &str = "scheduled_tasks.trash_cleanup.last_run_at";
const DATABASE_MAINTENANCE_LAST_RUN_KEY: &str = "scheduled_tasks.database_maintenance.last_run_at";

static SCHEDULED_TASK_RUNNER_RUNNING: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

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
                if settings.scheduled_tasks.metadata_refresh.enabled {
                    run_metadata_refresh_now(&db).await;
                }
                if settings.scheduled_tasks.trash_cleanup.enabled {
                    run_trash_cleanup_if_due(&db, &settings.scheduled_tasks).await;
                }
                if settings.scheduled_tasks.database_maintenance.enabled {
                    run_database_maintenance_if_due(&db, &settings.scheduled_tasks).await;
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
    rocket::tokio::spawn(async move {
        run_metadata_refresh_now(&db).await;
    });
}

/// Start a trash cleanup task immediately, outside the scheduled window.
pub fn start_trash_cleanup_task(db: DbConn) {
    rocket::tokio::spawn(async move {
        let settings = current_settings();
        if let Err(error) =
            run_trash_cleanup_now(&db, &settings.scheduled_tasks.trash_cleanup).await
        {
            log::error!("Manual trash cleanup failed: {}", error);
        }
    });
}

/// Start database maintenance immediately, outside the scheduled window.
pub fn start_database_maintenance_task(db: DbConn) {
    rocket::tokio::spawn(async move {
        if let Err(error) = run_database_maintenance_now(&db).await {
            log::error!("Manual database maintenance failed: {}", error);
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

async fn run_metadata_refresh_now(db: &DbConn) {
    let settings = current_settings();
    crate::web::routes::media::run_scheduled_metadata_refreshes(db, &settings).await;
}

async fn run_trash_cleanup_if_due(
    db: &DbConn,
    settings: &ScheduledTasksSettings,
) {
    let interval_seconds =
        i64::from(settings.trash_cleanup.interval_days).saturating_mul(24 * 60 * 60);
    let now = current_timestamp();
    let should_run = match db
        .run(move |conn| {
            let last_run = load_scheduled_task_last_run(conn, TRASH_CLEANUP_LAST_RUN_KEY)?;
            Ok::<bool, diesel::result::Error>(
                last_run
                    .map(|last_run| now.saturating_sub(last_run) >= interval_seconds)
                    .unwrap_or(true),
            )
        })
        .await
    {
        Ok(should_run) => should_run,
        Err(error) => {
            log::warn!("Failed to load scheduled trash cleanup state: {}", error);
            return;
        }
    };
    if !should_run {
        return;
    }

    match run_trash_cleanup_now(db, &settings.trash_cleanup).await {
        Ok(()) => {}
        Err(error) => log::error!("Scheduled trash cleanup failed: {}", error),
    }
}

async fn run_trash_cleanup_now(
    db: &DbConn,
    settings: &TrashCleanupTaskSettings,
) -> Result<(), String> {
    let Some(days) = settings
        .missing_item_auto_delete_days
        .filter(|days| *days > 0)
    else {
        return Ok(());
    };

    log::info!("Starting scheduled trash cleanup");
    db.run(move |conn| {
        let cutoff = current_timestamp().saturating_sub(i64::from(days) * 24 * 60 * 60);
        let summary = delete_missing_media_items(conn, None, Some(cutoff))?;
        if summary.deleted_items > 0 || summary.deleted_files > 0 {
            log::info!(
                "Scheduled trash cleanup deleted {} missing item rows and {} missing file rows",
                summary.deleted_items,
                summary.deleted_files
            );
        }
        save_scheduled_task_last_run(conn, TRASH_CLEANUP_LAST_RUN_KEY, current_timestamp())?;
        Ok::<(), diesel::result::Error>(())
    })
    .await
    .map_err(|error| error.to_string())?;

    log::info!("Scheduled trash cleanup completed");
    Ok(())
}

async fn run_database_maintenance_if_due(
    db: &DbConn,
    settings: &ScheduledTasksSettings,
) {
    let interval_seconds =
        i64::from(settings.database_maintenance.interval_days).saturating_mul(24 * 60 * 60);
    let now = current_timestamp();
    let should_run = match db
        .run(move |conn| {
            let last_run = load_scheduled_task_last_run(conn, DATABASE_MAINTENANCE_LAST_RUN_KEY)?;
            Ok::<bool, diesel::result::Error>(
                last_run
                    .map(|last_run| now.saturating_sub(last_run) >= interval_seconds)
                    .unwrap_or(true),
            )
        })
        .await
    {
        Ok(should_run) => should_run,
        Err(error) => {
            log::warn!(
                "Failed to load scheduled database maintenance state: {}",
                error
            );
            return;
        }
    };
    if !should_run {
        return;
    }

    match run_database_maintenance_now(db).await {
        Ok(()) => {}
        Err(error) => log::error!("Scheduled database maintenance failed: {}", error),
    }
}

async fn run_database_maintenance_now(db: &DbConn) -> Result<(), String> {
    log::info!("Starting scheduled database maintenance");
    db.run(move |conn| {
        conn.batch_execute("PRAGMA wal_checkpoint(TRUNCATE); VACUUM; PRAGMA optimize;")?;
        save_scheduled_task_last_run(conn, DATABASE_MAINTENANCE_LAST_RUN_KEY, current_timestamp())?;
        Ok::<(), diesel::result::Error>(())
    })
    .await
    .map_err(|error| error.to_string())?;

    log::info!("Scheduled database maintenance completed");
    Ok(())
}

fn load_scheduled_task_last_run(
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

fn save_scheduled_task_last_run(
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
