/** Renders the Live TV guide, program details, and shared or private recordings. */
import type { TvRecording } from '../liveTvApi';
import { escapeHtml as escape } from './format';
import { state } from './state';
import { renderIcon } from './ui';
import type { LiveTvState } from './liveTvActions';
import { renderTvGuide, renderTvHero } from './liveTvGuide';
import { tvTime } from './liveTvGuideModel';
/** Shared guide helpers for source-scoped schedules and safe artwork. */
export { channelPrograms, tvImage, tvTime } from './liveTvGuideModel';

/** Render data only for the account that fetched it. */
export function renderLiveTvPage(): string {
  const data = state.liveTv;
  if (!data || data.owner !== state.bootstrap?.current_user?.id) return '<section class="live-tv-page"><h1>Live TV</h1><p role="status">Loading channels and guide…</p></section>';
  const admin = state.bootstrap?.current_user?.admin === true;
  const tab = data.tab ?? 'guide';
  const tabs = [{ key: 'guide', label: 'Guide' }, { key: 'recordings', label: 'Recordings' }];
  let content = renderTvGuide(data);
  if (tab === 'recordings') content = renderRecordings(data);
  return `<div class="home-pinned-header"><header class="home-navbar" data-nav-section="browse-controls" data-nav-label="Live TV">
    <nav class="browse-tabs" aria-label="Live TV sections">${tabs.map(t => `<button type="button" class="browse-tab-button${tab === t.key ? ' active' : ''}" data-tv-tab="${t.key}" aria-pressed="${tab === t.key}">${t.label}</button>`).join('')}</nav>
    <div class="home-navbar-tools">${admin ? '<button type="button" class="secondary-button" data-settings-section-path="/settings/live-tv">Live TV settings</button>' : ''}<button type="button" class="secondary-button" data-tv-refresh>${renderIcon('refresh-cw')} Refresh</button></div></header>${renderTvHero(data)}</div>
    <section class="shelf-stack panel page-panel" data-nav-section="live-tv" data-nav-label="Live TV">
    ${data.error ? `<p class="live-tv-error" role="alert">${escape(data.error)}</p>` : ''}
    ${data.message ? `<p role="status">${escape(data.message)}</p>` : ''}
    ${data.loading ? '<p role="status">Updating Live TV…</p>' : ''}${content}</section>`;
}

function renderRecording(recording: TvRecording, data: LiveTvState): string {
  const active = ['scheduled', 'recording'].includes(recording.state);
  const conflict = data.conflicts.includes(recording.id);
  const cancelLabel = recording.state === 'recording' ? 'Stop recording' : 'Cancel recording';
  const play = recording.has_video ? `<button type="button" data-tv-recording-play="${recording.id}">Play</button>` : '';
  let controls = active
    ? `<button type="button" class="secondary-button" data-tv-cancel="${recording.id}">${cancelLabel}</button>`
    : `${play}<button type="button" class="secondary-button" data-tv-delete="${recording.id}">Delete recording</button>`;
  if (recording.state === 'stopping') controls = '<span role="status">Stopping recording…</span>';
  if (recording.state === 'deleting') controls = '<span role="status">Deleting recording…</span>';
  if (recording.user_id !== data.owner) controls = play;
  const error = recording.last_error ? `<p class="live-tv-error">${escape(recording.last_error)}</p>` : '';
  const description = recording.metadata?.description ? `<p>${escape(recording.metadata.description)}</p>` : '';
  return `<article class="live-tv-recording"><div><h3>${escape(recording.title)}</h3><p>${escape(tvTime(recording.starts_at, true))}–${escape(tvTime(recording.ends_at))}</p>
    <p>${escape(recording.state)} · ${recording.shared ? 'Shared' : 'Private'}${conflict ? ' · Tuner conflict' : ''}</p>${description}${error}</div><div class="live-tv-actions">${controls}</div></article>`;
}

function renderRecordings(data: LiveTvState): string {
  const rules = data.rules.map(rule => {
    const stop = rule.user_id === data.owner ? `<button type="button" class="secondary-button" data-tv-delete-rule="${rule.id}">Stop series</button>` : '';
    return `<article class="live-tv-recording"><div><strong>${escape(rule.title)}</strong><p>${rule.new_only ? 'New episodes only' : 'All episodes'} · ${rule.shared ? 'Shared' : 'Private'} · Channel ${escape(data.channels.find(c => c.id === rule.channel_id)?.name ?? String(rule.channel_id))}</p></div>${stop}</article>`;
  }).join('');
  return `<h2>Recordings</h2>${data.conflicts.length ? '<p class="live-tv-error" role="status">Some recordings overlap beyond the available tuner capacity. Cancel a conflicting recording or increase the source limit if more tuners are available.</p>' : ''}
    ${data.recordings.map(r => renderRecording(r, data)).join('') || '<p>No recordings scheduled.</p>'}
    <h2>Series</h2>${rules || '<p>Open a program in the guide and choose Record series.</p>'}
    ${renderManualRecording(data)}`;
}

function renderManualRecording(data: LiveTvState): string {
  if (data.canRecord === false) return '<p>Recording is disabled for your account.</p>';
  return `<details class="live-tv-panel"><summary>Schedule by time</summary><form id="tv-manual-record-form" class="live-tv-form">
      <label>Channel<select name="channel" required>${data.channels.filter(c => c.enabled && c.available).map(c => `<option value="${c.id}">${escape(c.number)} ${escape(c.name)}</option>`).join('')}</select></label>
      <label>Recording title<input name="title" required maxlength="500" /></label><label>Starts<input name="start" type="datetime-local" required /></label><label>Ends<input name="end" type="datetime-local" required /></label>
      <label>Before (minutes)<input name="before" type="number" min="0" max="60" value="0" /></label><label>After (minutes)<input name="after" type="number" min="0" max="60" value="0" /></label><label><input type="checkbox" name="private" /> Private (only my account)</label><button type="submit">Schedule recording</button></form></details>`;
}
