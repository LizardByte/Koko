//! Scheduled trash cleanup task.

// local imports
use crate::config::ScheduledTasksSettings;
use crate::db::DbConn;
use crate::media::delete_missing_media_items;
use crate::utils::current_timestamp;

use super::{ScheduledTask, ScheduledTaskFuture, save_scheduled_task_last_run};

const LAST_RUN_KEY: &str = "scheduled_tasks.trash_cleanup.last_run_at";

static TASK: TrashCleanupTask = TrashCleanupTask;

pub(super) fn task() -> &'static dyn ScheduledTask {
    &TASK
}

struct TrashCleanupTask;

impl ScheduledTask for TrashCleanupTask {
    fn id(&self) -> &'static str {
        "trash_cleanup"
    }

    fn name(&self) -> &'static str {
        "trash cleanup"
    }

    fn enabled(
        &self,
        settings: &ScheduledTasksSettings,
    ) -> bool {
        settings.trash_cleanup.enabled
    }

    fn interval_days(
        &self,
        settings: &ScheduledTasksSettings,
    ) -> Option<u32> {
        Some(settings.trash_cleanup.interval_days)
    }

    fn last_run_key(&self) -> Option<&'static str> {
        Some(LAST_RUN_KEY)
    }

    fn run_now<'a>(
        &'a self,
        db: &'a DbConn,
        settings: &'a ScheduledTasksSettings,
    ) -> ScheduledTaskFuture<'a> {
        Box::pin(async move {
            let Some(days) = settings
                .trash_cleanup
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
                save_scheduled_task_last_run(conn, LAST_RUN_KEY, current_timestamp())?;
                Ok::<(), diesel::result::Error>(())
            })
            .await
            .map_err(|error| error.to_string())?;

            log::info!("Scheduled trash cleanup completed");
            Ok(())
        })
    }
}
