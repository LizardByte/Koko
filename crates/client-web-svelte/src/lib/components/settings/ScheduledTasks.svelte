<script lang="ts">
  // ScheduledTasks — task runner window + 3 task cards (metadata-refresh,
  // trash-cleanup, database-maintenance). Port of renderScheduledTasksPage
  // (settingsView.ts:310-422). Each task card has a Run-now button (calls
  // settings.runTask) + per-task config fields.
  import Button from '../Button.svelte';
  import { settings, ui } from '$lib/stores';
  import type { SettingsSnapshot, ScheduledTaskId, ScheduledTaskWeekday } from '$lib/api';

  const WEEKDAYS: ScheduledTaskWeekday[] = [
    'monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday',
  ];

  function weekdayLabel(weekday: ScheduledTaskWeekday): string {
    return weekday.slice(0, 3).toUpperCase();
  }

  // Local editable copy of the scheduled_tasks section.
  let editing = $state<SettingsSnapshot['scheduled_tasks'] | undefined>(undefined);
  let refreshInterval = $state<number | null>(30);
  let saving = $state(false);
  let runningTask = $state<ScheduledTaskId | undefined>(undefined);

  $effect(() => {
    const s = settings.settings;
    if (s) {
      const sched = s.scheduled_tasks;
      editing = {
        enabled: sched.enabled,
        window: { ...sched.window, weekdays: [...sched.window.weekdays] },
        metadata_refresh: { ...sched.metadata_refresh },
        trash_cleanup: { ...sched.trash_cleanup },
        database_maintenance: { ...sched.database_maintenance },
      };
      refreshInterval = s.metadata.refresh_interval_days ?? null;
    }
  });

  function toggleWeekday(weekday: ScheduledTaskWeekday) {
    if (!editing) return;
    const set = new Set(editing.window.weekdays);
    if (set.has(weekday)) set.delete(weekday);
    else set.add(weekday);
    editing.window.weekdays = WEEKDAYS.filter((d) => set.has(d));
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    const current = settings.settings;
    if (!current || !editing) return;
    saving = true;
    try {
      const next: SettingsSnapshot = {
        ...current,
        scheduled_tasks: editing,
        metadata: { ...current.metadata, refresh_interval_days: refreshInterval },
      };
      await settings.save(next);
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : 'Failed to save scheduled tasks.');
    } finally {
      saving = false;
    }
  }

  async function runTask(taskId: ScheduledTaskId) {
    runningTask = taskId;
    try {
      await settings.runTask(taskId);
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : 'Failed to run scheduled task.');
    } finally {
      runningTask = undefined;
    }
  }
</script>

{#if editing}
  <section class="panel page-panel settings-page-panel">
    <form class="settings-form" onsubmit={save}>
      <section>
        <div class="section-heading">
          <h3>Scheduled tasks</h3>
        </div>
        <div class="settings-library-card">
          <div class="settings-library-header">
            <div>
              <p class="eyebrow">Runner</p>
              <h3>Task window</h3>
            </div>
            <span class="tag {editing.enabled ? 'success' : ''}">{editing.enabled ? 'Enabled' : 'Disabled'}</span>
          </div>
          <div class="form-row checkbox-row">
            <label><input type="checkbox" bind:checked={editing.enabled} /> Enable scheduled task runner</label>
          </div>
          <div class="form-row">
            <label>Start time<input type="time" bind:value={editing.window.start_time} /></label>
            <label>Stop time<input type="time" bind:value={editing.window.stop_time} /></label>
          </div>
          <fieldset>
            <legend>Run days</legend>
            <div class="weekday-toggle-row">
              {#each WEEKDAYS as weekday (weekday)}
                <label class="checkbox-inline">
                  <input type="checkbox" checked={editing.window.weekdays.includes(weekday)} onchange={() => toggleWeekday(weekday)} />
                  {weekdayLabel(weekday)}
                </label>
              {/each}
            </div>
          </fieldset>
        </div>

        <div class="settings-library-list">
          <!-- Metadata refresh -->
          <section class="settings-library-card">
            <div class="settings-library-header">
              <div>
                <p class="eyebrow">Task</p>
                <h3>Metadata refresh</h3>
              </div>
              <div class="settings-library-actions">
                <span class="tag {editing.metadata_refresh.enabled ? 'success' : ''}">{editing.metadata_refresh.enabled ? 'Scheduled' : 'Manual'}</span>
                <Button variant="secondary" label="Run now" icon="play" busy={runningTask === 'metadata_refresh'} onclick={() => runTask('metadata_refresh')} />
              </div>
            </div>
            <div class="form-row checkbox-row">
              <label><input type="checkbox" bind:checked={editing.metadata_refresh.enabled} /> Run stale metadata refreshes automatically</label>
            </div>
            <div class="form-row">
              <label>Refresh interval
                <select bind:value={refreshInterval}>
                  <option value={30}>Every 30 days</option>
                  <option value={60}>Every 60 days</option>
                  <option value={90}>Every 90 days</option>
                  <option value={null}>Never</option>
                </select>
              </label>
            </div>
          </section>

          <!-- Trash cleanup -->
          <section class="settings-library-card">
            <div class="settings-library-header">
              <div>
                <p class="eyebrow">Task</p>
                <h3>Trash cleanup</h3>
              </div>
              <div class="settings-library-actions">
                <span class="tag {editing.trash_cleanup.enabled ? 'warning' : ''}">{editing.trash_cleanup.enabled ? 'Scheduled' : 'Manual'}</span>
                <Button variant="secondary" label="Run now" icon="play" busy={runningTask === 'trash_cleanup'} onclick={() => runTask('trash_cleanup')} />
              </div>
            </div>
            <div class="form-row checkbox-row">
              <label><input type="checkbox" bind:checked={editing.trash_cleanup.enabled} /> Delete missing items automatically</label>
            </div>
            <div class="form-row">
              <label>Days missing<input type="number" min="1" max="3650" bind:value={editing.trash_cleanup.missing_item_auto_delete_days} /></label>
              <label>Run interval<input type="number" min="1" max="365" bind:value={editing.trash_cleanup.interval_days} /></label>
            </div>
          </section>

          <!-- Database maintenance -->
          <section class="settings-library-card">
            <div class="settings-library-header">
              <div>
                <p class="eyebrow">Task</p>
                <h3>Database maintenance</h3>
              </div>
              <div class="settings-library-actions">
                <span class="tag {editing.database_maintenance.enabled ? 'success' : ''}">{editing.database_maintenance.enabled ? 'Scheduled' : 'Manual'}</span>
                <Button variant="secondary" label="Run now" icon="play" busy={runningTask === 'database_maintenance'} onclick={() => runTask('database_maintenance')} />
              </div>
            </div>
            <div class="form-row checkbox-row">
              <label><input type="checkbox" bind:checked={editing.database_maintenance.enabled} /> Checkpoint, vacuum, and optimize automatically</label>
            </div>
            <div class="form-row">
              <label>Run interval<input type="number" min="1" max="365" bind:value={editing.database_maintenance.interval_days} /></label>
            </div>
          </section>
        </div>
      </section>
      <div class="page-actions">
        <Button type="submit" label="Save scheduled tasks" icon="save" busy={saving} />
      </div>
    </form>
  </section>
{/if}
