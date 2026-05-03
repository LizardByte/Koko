//! Scheduled metadata refresh task.

// local imports
use crate::config::{
    ScheduledTasksSettings,
    current_settings,
};
use crate::db::DbConn;

use super::{
    ScheduledTask,
    ScheduledTaskFuture,
};

static TASK: MetadataRefreshTask = MetadataRefreshTask;

pub(super) fn task() -> &'static dyn ScheduledTask {
    &TASK
}

struct MetadataRefreshTask;

impl ScheduledTask for MetadataRefreshTask {
    fn id(&self) -> &'static str {
        "metadata_refresh"
    }

    fn name(&self) -> &'static str {
        "metadata refresh"
    }

    fn enabled(
        &self,
        settings: &ScheduledTasksSettings,
    ) -> bool {
        settings.metadata_refresh.enabled
    }

    fn run_now<'a>(
        &'a self,
        db: &'a DbConn,
        _settings: &'a ScheduledTasksSettings,
    ) -> ScheduledTaskFuture<'a> {
        Box::pin(async move {
            let settings = current_settings();
            crate::web::routes::media::run_scheduled_metadata_refreshes(db, &settings).await;
            Ok(())
        })
    }
}
