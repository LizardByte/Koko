/** Renders administrator Live TV source, guide, channel, and DVR settings. */
import type { TvSource } from '../liveTvApi';
import { escapeHtml as escape, formatFileSize } from './format';
import { state } from './state';
import type { LiveTvState } from './liveTvActions';
import { renderTvSourceChannels,renderTvSourceGroups } from './liveTvSourceLists';
import { renderTvSourceHeading,renderTvSourceAppearance,renderTvProviders } from './liveTvProviders';
import { renderIcon } from './ui';

/** Keep Live TV administration inside the Settings workspace. */
export function renderLiveTvSettings(): string {
  const data = state.liveTv;
  if (!data || data.owner !== state.bootstrap?.current_user?.id) return '<p role="status">Loading Live TV settings…</p>';
  const error = data.error ? `<p class="live-tv-error" role="alert">${escape(data.error)}</p>` : '';
  const message = data.message ? `<p role="status">${escape(data.message)}</p>` : '';
  return `<section class="live-tv-settings">${error}${message}${data.loading ? '<p role="status">Updating Live TV settings…</p>' : ''}${renderSetup(data)}</section>`;
}
function renderSource(source: TvSource, data: LiveTvState): string {
  const scan = source.kind === 'native' ? `<button type="button" class="secondary-button" data-tv-source-scan="${source.id}">Rescan channels</button>` : '';
  const count = source.channel_count ?? data.channels.filter(c=>c.source_id === source.id && c.available).length;
  const sizes = [source.playlist_bytes == null ? '' : `M3U ${formatFileSize(source.playlist_bytes)}`,source.guide_bytes == null ? '' : `XMLTV ${formatFileSize(source.guide_bytes)}`].filter(Boolean).join(' · ');
  const open = data.connectionOpen === source.id;
  let editor = '';
  if (open) editor = data.connections?.[source.id] ? renderAddSource(source) : '<p role="status">Loading saved connection settings…</p>';
  const sizeMarkup = sizes ? '<span class="muted">Last imported: '+escape(sizes)+'</span>' : '';
  const expanded=data.sourceCardsOpen?.includes(source.id) ?? false;
  return `<details class="live-tv-source" data-controlled-open data-tv-source-card="${source.id}" ${expanded?'open':''}><summary class="tv-source-summary"><span class="tv-source-overview">${renderTvSourceHeading(source)}<span class="tv-source-status">${source.enabled ? 'Enabled' : 'Disabled'} · ${count} ${count === 1 ? 'channel' : 'channels'}${source.last_error?' · Refresh failed':''}</span>${sizeMarkup}</span>${renderIcon('chevron-right','tv-disclosure-icon')}</summary>
    <div class="tv-source-settings-body">${renderTvSourceAppearance(source)}
    ${source.last_error ? `<p class="live-tv-error">${escape(source.last_error)}</p>` : '<p>Channels and guide refresh automatically.</p>'}
    <form data-tv-source-update="${source.id}" class="live-tv-inline-form"><label><input type="checkbox" name="enabled" ${source.enabled ? 'checked' : ''} /> Enabled</label>
      <label>Simultaneous streams<input name="capacity" type="number" min="1" max="64" value="${source.capacity}" required /></label><button type="submit" class="secondary-button">Save</button></form>
    <div class="live-tv-actions"><button type="button" class="secondary-button" data-tv-source-refresh="${source.id}">Refresh channels and guide</button>${scan}
      <label class="live-tv-upload">Import XMLTV<input type="file" accept=".xml,.xmltv,.gz,text/xml,application/xml,application/gzip" data-tv-guide-upload="${source.id}" /></label></div>
    <details class="live-tv-panel" data-controlled-open data-tv-connection-source="${source.id}" ${open ? 'open' : ''}><summary>Edit connection settings</summary><p>Edit the saved connection and guide settings. Leave a saved PIN blank to keep it.</p>${editor}</details>${renderTvProviders(source,data)}${renderTvSourceChannels(source,data)}
    <div class="live-tv-actions"><button type="button" class="secondary-button" data-tv-source-remove="${source.id}">Remove source</button></div></div></details>`;
}

function renderSetup(data: LiveTvState): string {
  return `<h2>TV sources</h2><p>Add HDHomeRun, an IPTV M3U playlist, or a local Hauppauge or other tuner directly. NextPVR and Tvheadend are also available as optional sources.</p>
    <div class="live-tv-source-list">${data.sources.map(source=>renderSource(source,data)).join('')}</div>
    <details id="tv-add-source-panel" class="live-tv-panel" ${!data.sources.length || data.setupAdding ? 'open' : ''}><summary>Add a TV source</summary><label class="live-tv-kind-picker">Source type<select id="tv-source-kind">${SOURCE_TYPES.map(([kind,label]) => `<option value="${kind}" ${kind === (data.setupKind ?? 'hdhomerun') ? 'selected' : ''}>${label}</option>`).join('')}</select></label>${renderAddSource()}</details>
    ${renderTvSourceGroups(data)}
    ${renderDvrSettings(data)}`;
}

const SOURCE_TYPES = [['hdhomerun', 'HDHomeRun'], ['m3u', 'IPTV (M3U)'], ['native', 'Local tuner (Windows BDA / Linux DVB)'], ['nextpvr', 'NextPVR'], ['tvheadend', 'Tvheadend'], ['dvb', 'Linux DVB channel file (v4l-utils)']];

function renderAddSource(source?: TvSource): string {
  const kind = source?.kind ?? state.liveTv?.setupKind ?? 'hdhomerun';
  const identity = source ? `data-tv-replace-source="${source.id}"` : 'id="tv-source-form"';
  const capacity = ['native', 'dvb'].includes(kind) ? '<input type="hidden" name="capacity" value="1" />'
    : `<label>Simultaneous streams<input name="capacity" type="number" min="1" max="64" value="${source?.capacity ?? 1}" required /></label>`;
  const savedPin = Boolean(source && state.liveTv?.connections?.[source.id]?.has_pin);
  return `<form ${identity} class="live-tv-form" data-tv-source-kind="${escape(kind)}"><input type="hidden" name="kind" value="${escape(kind)}" /><input type="hidden" name="preserve_pin" value="${savedPin}" />
    <label>Name<input name="name" value="${escape(source?.name ?? '')}" required maxlength="200" placeholder="Living room TV" /></label>${capacity}
    ${renderConnection(kind,savedPin)}${renderGuideSettings(kind)}<button type="submit">${source ? 'Save connection settings' : 'Add source'}</button></form>`;
}

function renderConnection(kind: string, savedPin = false): string {
  if (kind === 'native') return renderLocalTuner();
  if (kind === 'dvb') return `<label>DVB channel file path<input name="url" required placeholder="/var/lib/koko/channels.conf" /></label>
    <label>DVB adapter<input name="adapter" type="number" min="0" max="63" value="0" /></label><label>DVB frontend<input name="frontend" type="number" min="0" max="63" value="0" /></label>
    <label>dvbv5-zap path<input name="zap_path" placeholder="dvbv5-zap" /></label><p class="live-tv-form-help">Install v4l-utils and generate this channel file with dvbv5-scan. For automatic channel scanning, choose Local tuner instead.</p>`;
  if (kind === 'm3u') return `<label>IPTV connection<select name="iptv_mode"><option value="playlist">M3U playlist</option><option value="fastchannels">FastChannels server</option></select></label>
    <label class="tv-iptv-address"><span data-tv-iptv-address-label>IPTV playlist URL</span><input name="url" type="url" required placeholder="https://provider.example/channels.m3u" /></label>
    <p class="live-tv-form-help" data-tv-iptv-server-help hidden>Enter the server address, for example http://fastchannels.example:5523. Koko imports the native lineup and guide automatically, plus PrismCast channels when the bridge is enabled in FastChannels.</p>
    <input type="hidden" name="provider_grouping_present" value="true"/><label class="live-tv-form-help"><input type="checkbox" name="provider_grouping"/> Group channels by provider (channel-id prefix before the first dot)</label>
    <p class="live-tv-form-help">Works with FastChannels, E+TV, and other playlists that include provider prefixes. Provider names and icons can be customized after importing.</p>
    <fieldset class="live-tv-guide-account" data-tv-iptv-bridge hidden disabled><legend>PrismCast</legend><label>Simultaneous bridge streams<input name="bridge_capacity" type="number" min="1" max="64" value="1"/></label><p>This limit is shared by all providers using the bridge.</p></fieldset>`;
  const label = SOURCE_TYPES.find(([id]) => kind === id)?.[1] ?? 'TV server';
  const port = ({ nextpvr: ':8866', tvheadend: ':9981' } as Record<string, string>)[kind] ?? '';
  const pinAttributes = savedPin ? 'placeholder="Leave blank to keep the saved PIN"' : 'required';
  const pin = kind === 'nextpvr' ? `<label>NextPVR PIN<input name="pin" type="password" autocomplete="off" ${pinAttributes} /></label>` : '';
  return `<fieldset class="live-tv-source-discovery"><legend>${escape(label)} connection</legend>
    <div class="live-tv-discovery-toolbar"><button type="button" data-tv-discover-source="${escape(kind)}">Find ${escape(label)}</button><p role="status" data-tv-discovery-status></p></div>
    <div class="live-tv-discovery-results" data-tv-discovery-results role="radiogroup" aria-label="Discovered ${escape(label)} sources" hidden></div>
    <label>${escape(label)} address<input name="url" type="url" required placeholder="http://tv-server.example${port}" /></label>
    <p>Discovery searches from the Koko server. You can also enter the address manually, including a custom port or HTTPS.</p></fieldset>${pin}`;
}

function renderLocalTuner(): string {
  const regions = state.liveTv?.regions ?? [];
  const options = regions.length ? regions.map(region => `<option value="${escape(region)}" ${region.toLowerCase() === 'north-america' ? 'selected' : ''}>${escape(region)}</option>`).join('') : '<option value="">Loading frequency regions…</option>';
  return `<fieldset class="live-tv-source-discovery"><legend>Local tuner</legend><div class="live-tv-discovery-toolbar"><button type="button" data-tv-discover-tuners>Find local tuners</button><p role="status" data-tv-tuner-status></p></div>
    <div class="live-tv-discovery-results" data-tv-discovery-results role="radiogroup" aria-label="Discovered local tuners" hidden></div>
    <label>Tuner location<input name="native_device" required placeholder="Windows tuner name or /dev/dvb/adapter0" /></label><p class="live-tv-form-help">Select a discovered tuner above, or enter its location manually.</p></fieldset>
    <label>Delivery system<select name="delivery_system"><option>ATSC</option><option>DVB-T</option><option>DVB-T2</option><option>ISDB-T</option></select></label>
    <label>Frequency region<select name="region" data-tv-frequency-region ${regions.length ? '' : 'disabled'} required>${options}</select></label><p role="status" data-tv-regions-status></p>
    <details class="live-tv-guide-account"><summary>Advanced tuning</summary><label>Additional tuning options<input name="tuning_options" placeholder="Optional, for example --bandwidth 8-MHz" /></label></details>
    <p class="live-tv-form-help">Install the manufacturer's driver. Koko scans channels when you add the tuner; scanning can take several minutes. Each tuner allows one simultaneous stream.</p>`;
}

function renderGuideSettings(kind: string): string {
  const broadcast = ['native', 'dvb', 'hdhomerun'].includes(kind);
  const multiplex = ['native', 'dvb'].includes(kind) ? '' : '<label>Unfiltered multiplex URLs (one per line)<textarea name="multiplex_urls" rows="3" placeholder="http://192.168.1.20:5004/auto/ch473000000"></textarea></label><p>Use physical RF channels or frequencies for HDHomeRun broadcast guide scans.</p>';
  return `<label>Guide source<select name="guide_kind"><option value="automatic">Automatic</option><option value="zap2it">Zap2It</option><option value="xmltv">XMLTV URL</option><option value="upload">Import XMLTV file after setup</option>${broadcast ? '<option value="broadcast">Over-the-air (DVB EIT / ATSC PSIP)</option>' : ''}</select></label>
    <p class="live-tv-form-help">Choose a guide for this source. Automatic uses the tuner or server guide when available.</p>
    <label>Program artwork<select name="artwork_provider"><option value="automatic">Automatic (configured TMDB / TVDB providers)</option><option value="guide">Guide images only</option><option value="tmdb">TheMovieDB</option><option value="tvdb">TheTVDB</option></select></label>
    <p class="live-tv-form-help">Artwork uses your enabled providers and saved API keys from Settings → Metadata providers. Schedules and program information still come from your selected guide. If a match is missing or ambiguous, Koko uses the guide image.</p>
    <fieldset data-tv-guide-option="zap2it" class="live-tv-guide-account" hidden disabled><legend>Zap2It lineup</legend>
      <label>Country<select name="guide_country"><option value="USA">United States</option><option value="CAN">Canada</option></select></label><label>ZIP / postal code<input name="guide_postal" maxlength="10" required /></label><button type="button" data-tv-provider-search>Find providers</button><p role="status" data-tv-provider-status></p><label>Provider<select name="guide_provider" required></select></label><label>Guide days<input name="guide_days" type="number" min="1" max="14" value="14" /></label></fieldset>
    <fieldset data-tv-guide-option="xmltv" class="live-tv-guide-account" hidden disabled><legend>XMLTV guide</legend><label>XMLTV URL<input name="xmltv_url" type="url" required placeholder="https://provider.example/guide.xml" /></label></fieldset>
    ${broadcast ? `<fieldset data-tv-guide-option="broadcast" class="live-tv-guide-account" hidden disabled><legend>Broadcast guide</legend>${multiplex}<p>Guide scans wait while recordings are due or running.</p></fieldset>` : ''}
    <label>Refresh interval (hours)<input name="refresh_hours" type="number" min="0" max="168" value="0" /></label><p class="live-tv-form-help">Zero uses the provider default: daily for Zap2It.</p>`;
}

function renderDvrSettings(data: LiveTvState): string {
  const settings = data.dvrSettings; if (!settings) return '';
  const permissions = settings.users.map(user => `<form data-tv-permission="${user.user_id}" class="live-tv-inline-form"><strong>${escape(user.username)}</strong><label><input type="checkbox" name="can_record" ${user.can_record ? 'checked' : ''} /> Allow recording</label><button type="submit">Save permission</button></form>`).join('');
  return `<details class="live-tv-panel"><summary>DVR storage and recording permissions</summary>
    <form id="tv-dvr-settings" class="live-tv-form"><label>DVR save folder<input name="recording_dir" value="${escape(settings.settings.recording_dir)}" placeholder="Default: Koko data folder/live-tv/recordings" /></label><p>Use an absolute folder path on the server. This applies to new captures; existing recordings keep their locations.</p><button type="submit">Save DVR folder</button></form>
    <p>Recordings are shared by default. Anyone can watch shared recordings; the account that scheduled a recording manages it.</p>
    ${permissions}</details>`;
}
