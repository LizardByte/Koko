//! Scheduled database maintenance task.

// lib imports
use diesel::connection::SimpleConnection;

// local imports
use crate::config::ScheduledTasksSettings;
use crate::db::DbConn;
use crate::utils::current_timestamp;

use super::{ScheduledTask, ScheduledTaskFuture, save_scheduled_task_last_run};

const LAST_RUN_KEY: &str = "scheduled_tasks.database_maintenance.last_run_at";

static TASK: DatabaseMaintenanceTask = DatabaseMaintenanceTask;

pub(super) fn task() -> &'static dyn ScheduledTask {
    &TASK
}

struct DatabaseMaintenanceTask;

impl ScheduledTask for DatabaseMaintenanceTask {
    fn id(&self) -> &'static str {
        "database_maintenance"
    }

    fn name(&self) -> &'static str {
        "database maintenance"
    }

    fn enabled(
        &self,
        settings: &ScheduledTasksSettings,
    ) -> bool {
        settings.database_maintenance.enabled
    }

    fn interval_days(
        &self,
        settings: &ScheduledTasksSettings,
    ) -> Option<u32> {
        Some(settings.database_maintenance.interval_days)
    }

    fn last_run_key(&self) -> Option<&'static str> {
        Some(LAST_RUN_KEY)
    }

    fn run_now<'a>(
        &'a self,
        db: &'a DbConn,
        _settings: &'a ScheduledTasksSettings,
    ) -> ScheduledTaskFuture<'a> {
        Box::pin(async move {
            log::info!("Starting scheduled database maintenance");
            db.run(move |conn| {
                conn.batch_execute("PRAGMA wal_checkpoint(TRUNCATE); VACUUM; PRAGMA optimize;")?;
                save_scheduled_task_last_run(conn, LAST_RUN_KEY, current_timestamp())?;
                Ok::<(), diesel::result::Error>(())
            })
            .await
            .map_err(|error| error.to_string())?;

            log::info!("Scheduled database maintenance completed");
            Ok(())
        })
    }
}
