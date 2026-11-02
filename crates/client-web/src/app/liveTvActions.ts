/** Loads account-scoped Live TV data and binds source, guide, playback, and DVR actions. */
import { formDataString } from './formUtils';
import * as api from '../liveTvApi';
import type { AppRoute, AppState } from './types';
import { state } from './state';
import { startTvPlayback, stopTvPlayback } from './playbackController';
import { bindTvDiscovery } from './liveTvDiscovery';
import { bindTvGuideActions } from './liveTvGuideActions';
import { channelArtwork, channelArtworkFallback, tvImage } from './liveTvGuideModel';
import { currentGuideWindow, TV_SLOT_SECONDS, TV_WINDOW_SECONDS } from './tvGuideTime';
import { tvChannelPlayback } from './tvPlayer';
import { bindTvGuideMappings } from './liveTvMapping';
import type { TvGuideViewport } from './tvGuideViewport';
import { bindTvGuideViewport } from './liveTvGuideViewport';
import { hydrateTvConnections, bindTvSourceSettings } from './liveTvSourceActions';
import { bindHoldActions } from './holdActions';
import { openTvGuideOptions, openTvRecordingOptions, syncTvDialogs } from './liveTvDialogs';

/** Live TV data belongs to the account that fetched it; setup data is administrator-only. */
export interface LiveTvState {
  owner: number; channels: api.TvChannel[]; programs: api.TvProgram[]; stations: api.TvStation[];
  recordings: api.TvRecording[]; rules: api.TvSeriesRule[]; sources: api.TvSource[]; conflicts: number[];
  windowStart: number; tab: 'guide' | 'recordings'; query?: string; group?: string;
  loading: boolean; error?: string; message?: string;
  canRecord?: boolean; dvrSettings?: api.TvDvrSettings;
  setupKind?: string; setupAdding?: boolean; regions?: string[];
  sourceGroups?:api.TvSourceGroup[]; sourceGroupId?:string;
  connections?:Record<number,api.TvSourceConnection>;connectionOpen?:number;connectionLoading?:number;
  sourceCardsOpen?:number[];
  connectionDrafts?:Record<number,Record<string,string>>;
  mappingSource?:number;mappingQuery?:string;mappingPage?:number;
  mappingProvider?:string;providerEditor?:string;providerView?:string;providers?:api.TvProvider[];
  providerDrafts?:Record<string,{name:string;enabled:boolean;icon_channel:string}>;
  guideSources?: api.TvGuideSource[]; preferences?: api.TvPreferences;
  sourceId?: number; category?: string; channelView?: 'all' | 'favorites' | 'recent';
  selectedProgramId?: number; selectedChannelId?: number;
  filtersOpen?: boolean;
  guideViewport?: TvGuideViewport;
}

/** Identify the administrator's dedicated Live TV settings section. */
export function isLiveTvSettings(route: AppRoute): boolean { return route.page === 'settings' && route.section === 'live-tv'; }

let loadGeneration = 0;
let nextGuideRefreshAt = 0;
let guideClockOwner: number | undefined;
type LiveTvApi = Pick<typeof api, 'getTvChannels' | 'getTvGuide' | 'getTvRecordings' | 'getTvRules' | 'getTvSources'> & Partial<Pick<typeof api, 'getTvDvrSettings'>>;

/** Ignore older guide windows as well as responses belonging to previous accounts. */
export async function loadLiveTv(app: AppState, isCurrent: () => boolean, source: LiveTvApi = api): Promise<void> {
  const generation = ++loadGeneration;
  const owner = app.bootstrap?.current_user?.id;
  if (owner === undefined) { app.liveTv = undefined; return; }
  const previous = app.liveTv?.owner === owner ? app.liveTv : undefined;
  const data: LiveTvState = previous ?? { owner, channels: [], programs: [], stations: [], recordings: [], rules: [], sources: [], conflicts: [], windowStart: currentGuideWindow(), tab: 'guide', loading: true };
  data.windowStart = currentGuideWindow(data.windowStart);
  app.liveTv = data;
  data.loading = true;
  const admin = app.bootstrap?.current_user?.admin === true;
  const setup = admin && isLiveTvSettings(app.route);
  try {
    const [channels, guide, recordings, rules, sources, dvrSettings] = await Promise.all([
      source.getTvChannels(setup), source.getTvGuide(data.windowStart, data.windowStart + TV_WINDOW_SECONDS),
      source.getTvRecordings(), source.getTvRules(), setup ? source.getTvSources() : Promise.resolve([]),
      setup ? source.getTvDvrSettings?.() : undefined,
    ]);
    if (generation !== loadGeneration || !isCurrent() || app.bootstrap?.current_user?.id !== owner) return;
    app.liveTv = { ...data, channels, programs: guide.programs, stations: guide.stations, guideSources:guide.sources, sourceGroups:guide.source_groups ?? [], providers:guide.providers ?? [], preferences:guide.preferences, recordings: recordings.recordings, conflicts: recordings.conflicts, canRecord: recordings.can_record, dvrSettings, rules, sources, error: undefined, loading: false };
  } catch (error) {
    if (generation === loadGeneration && isCurrent() && app.bootstrap?.current_user?.id === owner) {
      data.error = error instanceof Error ? error.message : 'Live TV could not be loaded.';
      data.loading = false;
    }
  }
}

function value(form: FormData, name: string): string { return formDataString(form.get(name)).trim(); }
/** Read editable fields while retaining saved settings for inactive guide modes. */
export function sourceConfig(form: FormData, original?: api.TvSourceConfig): api.TvSourceConfig {
  const selection = value(form, 'guide_provider');
  const zap2it = selection ? JSON.parse(selection) as api.TvGuideSelection : undefined;
  if (value(form, 'guide_kind') === 'zap2it' && (zap2it?.country !== value(form, 'guide_country') || zap2it.postal_code !== value(form, 'guide_postal').toUpperCase().replace(/ /g, ''))) throw new Error('Search for and select a Zap2It provider for this postal code.');
  const config:api.TvSourceConfig = { url: value(form, 'url'), pin: value(form, 'pin'), preserve_pin:value(form,'preserve_pin') === 'true', xmltv_url: value(form, 'xmltv_url'), guide_kind: value(form, 'guide_kind'), artwork_provider:value(form,'artwork_provider'),
    iptv_mode:value(form,'iptv_mode'),provider_grouping:form.has('provider_grouping'),bridge_capacity:Number(value(form,'bridge_capacity')),
    zap2it: zap2it ? { ...zap2it, days: Number(value(form, 'guide_days')) } : undefined, refresh_hours: Number(value(form, 'refresh_hours')),
    native_device: value(form, 'native_device'), delivery_system: value(form, 'delivery_system'), region: value(form, 'region'), tuning_options: value(form, 'tuning_options'),
    adapter: Number(value(form, 'adapter')), frontend: Number(value(form, 'frontend')), zap_path: value(form, 'zap_path'),
    multiplex_urls: value(form, 'multiplex_urls').split(/\r?\n/).map(s => s.trim()).filter(Boolean) };
  const saved = original as Record<string,unknown> | undefined;
  const merged = config as unknown as Record<string,unknown>;
  for (const key of Object.keys(saved ?? {})) {
    if (key==='provider_grouping'&&form.has('provider_grouping_present')) continue;
    if (key === 'zap2it' ? !selection : !form.has(key)) merged[key]=saved![key];
  }
  return config;
}
function padding(form: FormData): { padding_before: number; padding_after: number } {
  return { padding_before: Number(value(form, 'before')) * 60, padding_after: Number(value(form, 'after')) * 60 };
}

interface Context { navigateTo: (path: string) => void; render: () => void; renderTvSelection?: () => void }
/** Stop TV streams when leaving Live TV or changing accounts, including minimized playback. */
export function syncLiveTvAccount(): void {
  syncTvDialogs();
  if (state.route.page !== 'live-tv' || guideClockOwner !== state.bootstrap?.current_user?.id) { nextGuideRefreshAt = 0; guideClockOwner = undefined; }
  const tv = state.activeTvPlayback;
  if (tv && (tv.owner !== state.bootstrap?.current_user?.id || state.route.page !== 'live-tv')) stopTvPlayback();
}

function advanceGuideWindow(context: Context): void {
  const data = state.liveTv;
  if (!data) return;
  const start = currentGuideWindow(data.windowStart);
  if (start === data.windowStart) return;
  const scroll = document.querySelector<HTMLElement>('#tv-guide-scroll');
  const width = scroll?.querySelector('.tv-guide-time-track')?.getBoundingClientRect().width ?? 0;
  if (scroll) scroll.scrollLeft = Math.max(0,scroll.scrollLeft-(start-data.windowStart)/TV_WINDOW_SECONDS*width);
  data.windowStart = start;
  // Reclip cached listings immediately, including while the guide request is slow.
  context.render();
}

function bindGuideClock(signal: AbortSignal, owner: number | undefined, reload: () => Promise<void>): void {
  if (isLiveTvSettings(state.route)) return;
  const now = Date.now();
  if (guideClockOwner !== owner || !nextGuideRefreshAt) { guideClockOwner = owner; nextGuideRefreshAt = now+60000; }
  const slotExpires = (currentGuideWindow()+TV_SLOT_SECONDS)*1000+25;
  // A fixed deadline survives focus-triggered redraws; resetting a full minute
  // on every render would postpone both guide refreshes and half-hour rollover.
  const timer = globalThis.setTimeout(() => {
    nextGuideRefreshAt = Date.now()+60000;
    void reload();
  },Math.max(0,Math.min(60000,nextGuideRefreshAt-now,slotExpires-now)));
  signal.addEventListener('abort',() => globalThis.clearTimeout(timer),{once:true});
}

function play(kind: 'channels' | 'recordings', id: number): void {
  const data = state.liveTv;
  if (!data || data.owner !== state.bootstrap?.current_user?.id) return;
  if (kind === 'channels') {
    const channel = data.channels.find(c => c.id === id);
    if (!channel) return;
    data.selectedChannelId = id; data.selectedProgramId = undefined;
    void startTvPlayback(tvChannelPlayback(channel));
    return;
  }
  const recording = data.recordings.find(r => r.id === id);
  const channel = data.channels.find(c => c.id === recording?.channel_id);
  const title = recording?.title;
  if (!title) return;
  const image = tvImage(recording?.metadata.image);
  const logo = channel ? channelArtwork(channel,data) : undefined;
  const logoFallback = channel ? channelArtworkFallback(channel,data) : undefined;
  void startTvPlayback({kind,id,title,image,logo,logoFallback,url:api.tvStreamUrl(kind,id)});
}
/** Bind route controls once per render and cancel listeners when the app replaces its DOM. */
export function bindLiveTvActions(signal: AbortSignal, context: Context): void {
  document.querySelectorAll<HTMLElement>('[data-nav-live-tv]').forEach(button => button.addEventListener('click', () => context.navigateTo('/live-tv'), { signal }));
  syncLiveTvAccount();
  const setup = isLiveTvSettings(state.route);
  if ((!setup && state.route.page !== 'live-tv') || state.liveTv?.owner !== state.bootstrap?.current_user?.id) return;
  const owner = state.liveTv?.owner;
  const current = () => (setup ? isLiveTvSettings(state.route) : state.route.page === 'live-tv') && state.bootstrap?.current_user?.id === owner;
  const reload = async () => {
    if (!current()) return;
    advanceGuideWindow(context);
    await loadLiveTv(state, current);
    if (current()) context.render();
  };
  const run = async (operation: () => Promise<unknown>, button?: HTMLButtonElement) => {
    if (button) button.disabled = true;
    try {
      await operation();
      if (current() && state.liveTv) state.liveTv.error = undefined;
      if (current()) await reload();
    } catch (error) {
      if (current() && state.liveTv) { state.liveTv.error = error instanceof Error ? error.message : 'The TV operation failed.'; context.render(); }
    } finally { if (button) button.disabled = false; }
  };
  document.querySelector('[data-tv-refresh]')?.addEventListener('click', () => { void reload(); }, { signal });
  document.querySelectorAll<HTMLButtonElement>('[data-tv-tab]').forEach(button => button.addEventListener('click', () => {
    if (!state.liveTv) return;
    state.liveTv.tab = button.dataset.tvTab as LiveTvState['tab']; context.render(); void reload();
  }, { signal }));
  document.querySelectorAll<HTMLButtonElement>('[data-tv-window]').forEach(button => button.addEventListener('click', () => {
    if (!state.liveTv) return;
    const step = Number(button.dataset.tvWindow);
    state.liveTv.windowStart = currentGuideWindow(step ? state.liveTv.windowStart + step * TV_WINDOW_SECONDS : 0);
    const scroll = document.querySelector('#tv-guide-scroll'); if (scroll) scroll.scrollLeft = 0;
    void reload();
  }, { signal }));
  document.querySelector<HTMLFormElement>('#tv-filter-form')?.addEventListener('submit', event => {
    event.preventDefault(); if (!state.liveTv) return;
    const form = new FormData(event.currentTarget as HTMLFormElement);
    state.liveTv.query = value(form, 'query'); state.liveTv.group = value(form, 'group'); context.render();
  }, { signal });
  bindPlayback(signal,()=>{void reload();});
  bindTvGuideActions(signal, context, run);
  bindTvGuideViewport(signal);
  bindRecordingForms(signal, run);
  bindRecordingButtons(signal, run);
  if (setup && state.bootstrap?.current_user?.admin) bindSetup(signal, run, context);
  bindGuideClock(signal,owner,reload);
}

type Run = (operation: () => Promise<unknown>, button?: HTMLButtonElement) => Promise<void>;

function bindGuideChoices(signal: AbortSignal): void {
  document.querySelectorAll<HTMLSelectElement>('select[name="guide_kind"]').forEach(select => {
    const update = () => select.closest('form')?.querySelectorAll<HTMLFieldSetElement>('[data-tv-guide-option]').forEach(fieldset => {
      fieldset.hidden = fieldset.dataset.tvGuideOption !== select.value;
      fieldset.disabled = fieldset.hidden;
    });
    update(); select.addEventListener('change', update, { signal });
  });
}

function loadTunerRegions(signal: AbortSignal): void {
  const selects = [...document.querySelectorAll<HTMLSelectElement>('[data-tv-frequency-region]')];
  if (!selects.length || state.liveTv?.regions?.length) return;
  void api.getTvTunerRegions().then(regions => {
    if (signal.aborted || !state.liveTv) return;
    state.liveTv.regions = regions;
    for (const select of selects) {
      const saved = select.value;
      select.replaceChildren();
      for (const region of regions) { const option = document.createElement('option'); option.value = region; option.textContent = region; option.selected = saved ? region === saved : region.toLowerCase() === 'north-america'; select.append(option); }
      if (saved && !regions.includes(saved)) {const option=document.createElement('option');option.value=saved;option.textContent=saved;option.selected=true;select.append(option);}
      select.disabled = false;
    }
  }).catch((error: unknown) => {
    if (signal.aborted) return;
    document.querySelectorAll<HTMLElement>('[data-tv-regions-status]').forEach(status => { status.textContent = error instanceof Error ? error.message : 'Unable to load frequency regions.'; });
  });
}

function bindPlayback(signal: AbortSignal, changed:()=>void): void {
  const watch=(id:number)=>play('channels',id);
  bindHoldActions('[data-tv-guide-activate]',button=>watch(Number(button.dataset.tvSelectChannel)),button=>openTvGuideOptions(Number(button.dataset.tvSelectChannel),Number(button.dataset.tvSelectProgram)||undefined,{play:watch,changed}),signal);
  document.addEventListener('click', event => {
    const button = (event.target as Element | null)?.closest<HTMLElement>('[data-tv-play], [data-tv-recording-play], [data-tv-record-options]');
    if (button?.dataset.tvPlay) play('channels',Number(button.dataset.tvPlay));
    else if (button?.dataset.tvRecordingPlay) play('recordings',Number(button.dataset.tvRecordingPlay));
    else if(button?.dataset.tvRecordOptions)openTvRecordingOptions(Number(button.dataset.tvRecordOptions),Number(button.dataset.tvRecordProgram),changed);
  },{signal});
}
function bindRecordingForms(signal: AbortSignal, run: Run): void {
  document.querySelector<HTMLFormElement>('#tv-manual-record-form')?.addEventListener('submit', event => {
    event.preventDefault(); const input = new FormData(event.currentTarget as HTMLFormElement);
    void run(() => api.scheduleTvRecording({ channel_id: Number(value(input, 'channel')), title: value(input, 'title'), starts_at: Math.floor(new Date(value(input, 'start')).getTime() / 1000), ends_at: Math.floor(new Date(value(input, 'end')).getTime() / 1000), series: false, new_only: false, private: input.has('private'), ...padding(input) }));
  }, { signal });
}
function bindRecordingButtons(signal: AbortSignal, run: Run): void {
  const actions = [
    { selector: '[data-tv-cancel]', key: 'tvCancel', action: api.cancelTvRecording },
    { selector: '[data-tv-delete]', key: 'tvDelete', action: api.deleteTvRecording },
    { selector: '[data-tv-delete-rule]', key: 'tvDeleteRule', action: api.deleteTvRule },
    { selector: '[data-tv-source-refresh]', key: 'tvSourceRefresh', action: api.refreshTvSource },
    { selector: '[data-tv-source-scan]', key: 'tvSourceScan', action: api.rescanTvTuner },
  ];
  for (const item of actions) document.querySelectorAll<HTMLButtonElement>(item.selector).forEach(button => button.addEventListener('click', () => { void run(() => item.action(Number(button.dataset[item.key])), button); }, { signal }));
}
function bindSetup(signal: AbortSignal, run: Run, context: Context): void {
  hydrateTvConnections();
  bindTvSourceSettings(signal,context,run);
  document.querySelector<HTMLSelectElement>('#tv-source-kind')?.addEventListener('change', event => {
    if (state.liveTv) { state.liveTv.setupKind = (event.currentTarget as HTMLSelectElement).value; state.liveTv.setupAdding = true; }
    context.render();
  }, { signal });
  document.querySelector<HTMLDetailsElement>('#tv-add-source-panel')?.addEventListener('toggle', event => {
    if (state.liveTv) state.liveTv.setupAdding = (event.currentTarget as HTMLDetailsElement).open;
  }, { signal });
  bindGuideChoices(signal);
  bindTvGuideMappings(document, signal, state.liveTv?.stations ?? []);
  bindTvDiscovery(signal);
  loadTunerRegions(signal);
  document.querySelector<HTMLFormElement>('#tv-dvr-settings')?.addEventListener('submit', event => {
    event.preventDefault(); const input = new FormData(event.currentTarget as HTMLFormElement);
    void run(() => api.setTvDvrSettings(value(input, 'recording_dir')));
  }, { signal });
  document.querySelectorAll<HTMLFormElement>('[data-tv-permission]').forEach(form => form.addEventListener('submit', event => {
    event.preventDefault(); const input = new FormData(form);
    void run(() => api.setTvRecordingPermission(Number(form.dataset.tvPermission), input.has('can_record')));
  }, { signal }));
  document.querySelectorAll<HTMLButtonElement>('[data-tv-provider-search]').forEach(button => button.addEventListener('click', () => {
    const form = button.closest('form'); if (!form) return;
    const input = new FormData(form); const country = value(input, 'guide_country'); const postal = value(input, 'guide_postal').toUpperCase().replace(/ /g, '');
    const select = form.querySelector<HTMLSelectElement>('[name="guide_provider"]'); const status = form.querySelector<HTMLElement>('[data-tv-provider-status]');
    if (!select || !status) return;
    button.disabled = true; status.textContent = 'Finding providers…'; select.replaceChildren();
    void api.findTvGuideProviders(country, postal).then(providers => {
      if (signal.aborted) return;
      for (const provider of providers) {
        const option = document.createElement('option');
        const location = provider.location ? ' · ' + provider.location : '';
        option.textContent = `${provider.type} · ${provider.name}${location}`;
        option.value = JSON.stringify({ country, postal_code: postal, lineup_id: provider.lineupId, headend_id: provider.headendId, device: provider.device, timezone: provider.timezone }); select.append(option);
      }
      status.textContent = providers.length ? 'Select your provider, then save the source.' : 'No providers found.';
    }).catch((error: unknown) => { if (!signal.aborted) status.textContent = error instanceof Error ? error.message : 'Provider lookup failed.'; }).finally(() => { button.disabled = false; });
  }, { signal }));
  document.querySelectorAll<HTMLFormElement>('[data-tv-replace-source]').forEach(form => form.addEventListener('submit', event => {
    event.preventDefault(); const input = new FormData(form);
    void run(async () => {
      const id=Number(form.dataset.tvReplaceSource);
      await api.replaceTvSource(id, { name: value(input, 'name'), kind: value(input, 'kind'), capacity: Number(value(input, 'capacity')), config: sourceConfig(input,state.liveTv?.connections?.[id]?.source.config) });
      if (state.liveTv) {delete state.liveTv.connections?.[id];delete state.liveTv.connectionDrafts?.[id];state.liveTv.connectionOpen=undefined;}
    }, (event as SubmitEvent).submitter as HTMLButtonElement | undefined);
  }, { signal }));
  document.querySelector<HTMLFormElement>('#tv-source-form')?.addEventListener('submit', event => {
    event.preventDefault(); const form = event.currentTarget as HTMLFormElement;
    const input = new FormData(form);
    void run(async () => {
      const result = await api.addTvSource({ name: value(input, 'name'), kind: value(input, 'kind'), capacity: Number(value(input, 'capacity')), config: sourceConfig(input) });
      form.reset();
      if (state.liveTv) state.liveTv.setupAdding = false;
      if (state.liveTv) state.liveTv.message = result.refresh_error ?? 'TV source added. Channels and guide will refresh in the background.';
    }, (event as SubmitEvent).submitter as HTMLButtonElement | undefined);
  }, { signal });
  document.querySelectorAll<HTMLFormElement>('[data-tv-source-update]').forEach(form => form.addEventListener('submit', event => {
    event.preventDefault(); const input = new FormData(form);
    void run(() => api.setTvSource({ id: Number(form.dataset.tvSourceUpdate), enabled: input.has('enabled'), capacity: Number(value(input, 'capacity')) }));
  }, { signal }));
  document.querySelectorAll<HTMLFormElement>('[data-tv-channel-update]').forEach(form => form.addEventListener('submit', event => {
    event.preventDefault(); const input = new FormData(form);
    void run(() => api.setTvChannel(Number(form.dataset.tvChannelUpdate), input.has('enabled'), value(input, 'guide') || null));
  }, { signal }));
  document.querySelectorAll<HTMLInputElement>('[data-tv-guide-upload]').forEach(input => input.addEventListener('change', () => {
    const file = input.files?.[0]; if (file) void run(() => api.uploadTvGuide(Number(input.dataset.tvGuideUpload), file));
  }, { signal }));
}
