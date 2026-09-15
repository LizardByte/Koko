<script lang="ts">
  // SystemActivities — list of non-terminal background activities with progress
  // bars. Port of renderSystemActivitiesPanel (dashboardView.ts:217-258).
  import { activities } from '$lib/stores';
  import type { SystemActivity } from '$lib/api';

  // Filter to non-terminal activities (running/pending/queued).
  const active = $derived(
    (activities.systemActivities?.activities ?? []).filter(
      (a) => a.state !== 'completed' && a.state !== 'failed',
    ),
  );

  function activityProgress(activity: SystemActivity): { percent: number; completed: number; total: number; failed: number } {
    const total = activity.total_items ?? 0;
    const completed = activity.completed_items ?? 0;
    const failed = activity.failed_items ?? 0;
    const percent = total > 0 ? Math.round(((completed + failed) / total) * 100) : 0;
    return { percent, completed, total, failed };
  }
</script>

<section class="panel page-panel settings-system-activity-panel">
  <div class="section-heading">
    <h3>Backend activities</h3>
  </div>
  {#if active.length === 0}
    <div class="empty-state tight">No background activities are running right now.</div>
  {:else}
    <div class="settings-system-activity-list">
      {#each active as activity (activity.id)}
        {@const progress = activityProgress(activity)}
        <article class="settings-system-activity">
          <div class="settings-system-activity-header">
            <div>
              <strong>{activity.label}</strong>
              <p class="muted">{activity.scope} · {activity.source}</p>
            </div>
            <div class="provider-tags">
              <span class="tag {activity.state === 'running' ? 'warning' : ''}">{activity.state}</span>
              {#if activity.provider_id}<span class="tag">{activity.provider_id}</span>{/if}
            </div>
          </div>
          <div class="activity-progress-row">
            <div class="activity-progress-bar" aria-hidden="true">
              <span class="activity-progress-fill" style="--activity-progress: {progress.percent}%;"></span>
            </div>
            <span class="muted">{progress.completed}/{progress.total}{progress.failed ? ` · ${progress.failed} failed` : ''}</span>
          </div>
        </article>
      {/each}
    </div>
  {/if}
</section>
