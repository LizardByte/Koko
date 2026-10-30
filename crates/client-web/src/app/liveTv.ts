/** Renders the Live TV guide, program details, and shared or private recordings. */
import type { TvChannel, TvProgram, TvRecording } from '../liveTvApi';
import { escapeHtml as escape } from './format';
import { state } from './state';
import { renderIcon } from './ui';
import type { LiveTvState } from './liveTvActions';

/** Match source-scoped station IDs, respecting a user's explicit channel mapping. */
export function channelPrograms(channel: TvChannel, programs: TvProgram[]): TvProgram[] {
  const key = channel.guide_override ?? channel.guide_key;
  return programs.filter(p => p.source_id === channel.source_id && p.guide_key === key).sort((a, b) => a.starts_at - b.starts_at);
}

/** Format UTC epoch seconds in the viewer's local timezone. */
export function tvTime(seconds: number, date = false): string {
  return new Date(seconds * 1000).toLocaleString(undefined, date
    ? { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' }
    : { hour: 'numeric', minute: '2-digit' });
}

/** Reject active or local-file artwork URLs before inserting them into the guide. */
export function tvImage(value?: string | null): string {
  if (!value) return '';
  try { const url = new URL(value); return ['http:', 'https:'].includes(url.protocol) && !url.username && !url.password ? url.href : ''; }
  catch { return ''; }
}

function picture(value?: string | null, className = ''): string {
  const url = tvImage(value);
  return url ? `<img class="${className}" src="${escape(url)}" alt="" loading="lazy" referrerpolicy="no-referrer" />` : '';
}

/** Render data only for the account that fetched it. */
export function renderLiveTvPage(): string {
  const data = state.liveTv;
  if (!data || data.owner !== state.bootstrap?.current_user?.id) return '<section class="live-tv-page"><h1>Live TV</h1><p role="status">Loading channels and guide…</p></section>';
  const admin = state.bootstrap?.current_user?.admin === true;
  const tab = data.tab ?? 'guide';
  const tabs = [{ key: 'guide', label: 'Guide' }, { key: 'recordings', label: 'Recordings' }];
  let content = renderGuide(data);
  if (tab === 'recordings') content = renderRecordings(data);
  return `<section class="live-tv-page" data-nav-section="live-tv" data-nav-label="Live TV">
    <header class="live-tv-header"><div><p class="eyebrow">Television</p><h1>Live TV</h1><p>Live channels, your guide, and recordings.</p></div>
      <div class="live-tv-actions">${admin ? '<button type="button" class="secondary-button" data-settings-section-path="/settings/live-tv">Live TV settings</button>' : ''}<button type="button" class="secondary-button" data-tv-refresh>${renderIcon('refresh-cw')} Refresh</button></div></header>
    <nav class="live-tv-tabs" aria-label="Live TV sections">${tabs.map(t => `<button type="button" data-tv-tab="${t.key}" aria-pressed="${tab === t.key}">${t.label}</button>`).join('')}</nav>
    ${data.error ? `<p class="live-tv-error" role="alert">${escape(data.error)}</p>` : ''}
    ${data.message ? `<p role="status">${escape(data.message)}</p>` : ''}
    ${data.loading ? '<p role="status">Updating Live TV…</p>' : ''}${content}</section>`;
}

function renderProgram(channel: TvChannel, program: TvProgram): string {
  const metadata = program.metadata;
  const now = Date.now() / 1000;
  const live = program.starts_at <= now && program.ends_at > now;
  const tags = [...(metadata.categories ?? []), metadata.episode, metadata.rating].filter(Boolean).join(' · ');
  return `<details class="live-tv-program${live ? ' is-live' : ''}"><summary>
    <small>${escape(tvTime(program.starts_at))}–${escape(tvTime(program.ends_at))}${live ? ' · Live' : ''}</small>
    <strong>${escape(program.title)}</strong>${metadata.subtitle ? `<span>${escape(metadata.subtitle)}</span>` : ''}</summary>
    <div class="live-tv-program-details">${picture(metadata.image, 'live-tv-program-image')}<p>${escape(metadata.description ?? 'No program description available.')}</p>
      ${tags ? `<p class="muted">${escape(tags)}</p>` : ''}${metadata.credits?.length ? `<p>${escape(metadata.credits.join(', '))}</p>` : ''}
      ${program.ends_at > now ? renderRecordingChoices(channel, program) : ''}</div></details>`;
}

function renderRecordingChoices(channel: TvChannel, program: TvProgram): string {
  if (state.liveTv?.canRecord === false) return '<p>Recording is disabled for your account.</p>';
  return `<form data-tv-program-record="${program.id}" data-tv-channel="${channel.id}" class="live-tv-record-choices">
    <label>Before (minutes)<input name="before" type="number" min="0" max="60" value="1" /></label>
    <label>After (minutes)<input name="after" type="number" min="0" max="60" value="2" /></label>
    <label><input type="checkbox" name="new-only" /> New episodes only (series)</label>
    <label><input type="checkbox" name="private" /> Private (only my account)</label>
    <button type="submit" name="mode" value="single">Record program</button>
    <button type="submit" name="mode" value="series" class="secondary-button">Record series</button></form>`;
}

function renderGuide(data: LiveTvState): string {
  const groups = [...new Set(data.channels.map(c => c.group_name).filter(Boolean))].sort((a, b) => a.localeCompare(b));
  const query = (data.query ?? '').toLocaleLowerCase();
  const channels = data.channels.filter(c => c.enabled && c.available && (!data.group || c.group_name === data.group)
    && (!query || `${c.number} ${c.name}`.toLocaleLowerCase().includes(query)));
  const rows = channels.map(channel => {
    const programs = channelPrograms(channel, data.programs);
    return `<article class="live-tv-row"><div class="live-tv-channel">${picture(channel.logo, 'live-tv-channel-logo')}
      <span class="live-tv-channel-number">${escape(channel.number)}</span><strong>${escape(channel.name)}</strong>
      <button type="button" data-tv-play="${channel.id}">${renderIcon('play')} Watch live</button></div>
      <div class="live-tv-programs">${programs.length ? programs.map(p => renderProgram(channel, p)).join('') : '<p class="live-tv-no-guide">No guide data. You can still watch this channel or schedule a recording.</p>'}</div></article>`;
  }).join('');
  return `<div class="live-tv-guide-controls"><button type="button" class="secondary-button" data-tv-window="-1" aria-label="Earlier programs">←</button>
    <button type="button" class="secondary-button" data-tv-window="0">Now</button>
    <strong>${escape(tvTime(data.windowStart, true))}</strong><button type="button" class="secondary-button" data-tv-window="1" aria-label="Later programs">→</button>
    <form id="tv-filter-form"><label>Channel<input type="search" name="query" value="${escape(data.query ?? '')}" placeholder="Search channels" /></label>
      <label>Group<select name="group"><option value="">All groups</option>${groups.map(g => `<option ${data.group === g ? 'selected' : ''}>${escape(g)}</option>`).join('')}</select></label><button type="submit" class="secondary-button">Filter</button></form></div>
    ${rows || '<p>No channels available. Add a TV source in Settings → Live TV, or ask the server administrator to configure one.</p>'}`;
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
