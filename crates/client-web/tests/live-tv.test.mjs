/** Tests account isolation, station mapping, safe metadata rendering, and guide request races. */
import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';
import { JSDOM } from 'jsdom';

globalThis.location = { pathname: '/live-tv', origin: 'https://koko.local' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { channelPrograms, renderLiveTvPage, tvImage } = await server.ssrLoadModule('/src/app/liveTv.ts');
const { loadLiveTv } = await server.ssrLoadModule('/src/app/liveTvActions.ts');
const { renderLiveTvSettings } = await server.ssrLoadModule('/src/app/liveTvSetup.ts');
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const { parseRoute } = await server.ssrLoadModule('/src/app/routes.ts');
const { programPosition, programBadges, matchesTvCategory, visibleGuideChannels, guideSources } = await server.ssrLoadModule('/src/app/liveTvGuideModel.ts');
const { renderPlayerOverlay } = await server.ssrLoadModule('/src/app/playbackController.ts');
const { TvPlaybackWatchdog } = await server.ssrLoadModule('/src/app/tvPlaybackRecovery.ts');
const now = Math.floor(Date.now() / 1000);
const channel = { id: 1, source_id: 1, source_key: 'source', name: 'Test TV', number: '7.1', group_name: 'News', guide_key: 'station', enabled: true, available: true };
const program = { id: 1, source_id: 1, guide_key: 'station', title: 'News', starts_at: now - 30, ends_at: now + 3600, metadata: { description: 'Evening news', categories: ['News'], credits: ['Presenter'] } };
function data(extra = {}) {
  return { owner: 1, channels: [channel], programs: [program], stations: [], recordings: [], rules: [], sources: [], conflicts: [], windowStart: now, tab: 'guide', loading: false, ...extra };
}
function app(extra = {}) { return { bootstrap: { current_user: { id: 1, admin: false } }, route: { page: 'live-tv' }, liveTv: data(), ...extra }; }
function api(extra = {}) {
  return { getTvChannels: async () => [channel], getTvGuide: async () => ({ programs: [program], stations: [] }), getTvRecordings: async () => ({ recordings: [], conflicts: [] }), getTvRules: async () => [], getTvSources: async () => [], ...extra };
}

test('live television has its own persistent route', () => { assert.deepEqual(parseRoute(), { page: 'live-tv' }); });

test('the guide clips broadcasts to a shared timeline without closing schedule gaps', () => {
  const clipped = programPosition({...program,starts_at:now-900,ends_at:now+900},now);
  assert.equal(clipped.left,0); assert.ok(Math.abs(clipped.width-900/21600*100)<0.001);
  const later = programPosition({...program,starts_at:now+7200,ends_at:now+10800},now);
  assert.ok(Math.abs(later.left-100/3)<0.001); assert.ok(Math.abs(later.width-100/6)<0.001);
  assert.equal(programPosition({...program,starts_at:now-900,ends_at:now},now),undefined);
});

test('live broadcasts, premieres, and new episodes retain separate flags and genre filters', () => {
  assert.deepEqual(programBadges({is_new:true,is_live:false}),['NEW']);
  assert.deepEqual(programBadges({is_live:true,is_premiere:true,is_finale:true}),['LIVE','PREMIERE','FINALE']);
  assert.equal(matchesTvCategory({...program,metadata:{categories:['Sports event']}},'Sports'),true);
  assert.equal(matchesTvCategory({...program,metadata:{categories:['Movie / drama']}},'Movies'),true);
  assert.equal(matchesTvCategory({...program,metadata:{categories:['News / Current affairs']}},'News'),true);
  assert.equal(matchesTvCategory({...program,metadata:{}},'Sports'),false);
});

test('source navigation and personal channel views retain source identity', () => {
  const other = {...channel,id:2,source_id:2,name:'Other source'};
  const model = data({channels:[channel,other],guideSources:[{id:1,name:'Antenna'},{id:2,name:'Cable'}],preferences:{favorites:[2],recent:[2,1]}});
  assert.deepEqual(guideSources(model).map(s=>s.name),['Antenna','Cable']);
  assert.deepEqual(visibleGuideChannels(model).map(c=>c.id),[1]);
  model.sourceId=2; model.channelView='favorites';
  assert.deepEqual(visibleGuideChannels(model).map(c=>c.id),[2]);
  model.sourceId=1; assert.deepEqual(visibleGuideChannels(model),[]);
});

test('Zap2It logos and cover art request larger sizes while other hosts remain unchanged', () => {
  assert.equal(tvImage('https://zpmc.tmsimg.com/h3/NowShowing/45738/logo.png?w=55'),'https://zpmc.tmsimg.com/h3/NowShowing/45738/logo.png?w=320');
  assert.equal(tvImage('https://zpmc.tmsimg.com/assets/cover.jpg?w=165'),'https://zpmc.tmsimg.com/assets/cover.jpg?w=960');
  assert.equal(tvImage('https://example.com/cover.jpg?w=165'),'https://example.com/cover.jpg?w=165');
});

test('TV uses the shared video player in both overlay and hero modes without exposing another account', () => {
  state.bootstrap={current_user:{id:1,admin:true}};
  state.activeTvPlayback={owner:1,kind:'channels',id:5,title:'Live news',logo:'/channel-logo.png',url:'/api/v1/live-tv/sessions/test/stream',buffer:{session_id:'test',start_ms:0,end_ms:12000,can_keep:true}};
  state.isPlayerOpen=true;
  let dom=new JSDOM(renderPlayerOverlay());
  assert.ok(dom.window.document.querySelector('.media-player-shell video#media-player'));
  assert.ok(dom.window.document.querySelector('#player-quality'));
  assert.ok(dom.window.document.querySelector('#player-keep-tv'));
  assert.ok(dom.window.document.querySelector('.player-bottom-controls #player-keep-tv'));
  assert.equal(dom.window.document.querySelector('.player-top-controls #player-keep-tv'),null);
  assert.equal(dom.window.document.querySelector('#player-recording-menu').hidden,true);
  assert.equal(dom.window.document.querySelectorAll('#player-recording-menu [role="menuitem"]').length,2);
  assert.equal(dom.window.document.querySelector('.player-channel-logo').getAttribute('src'),'/channel-logo.png');
  assert.equal(dom.window.document.querySelector('.player-channel-heading h2').textContent,'Live news');
  assert.equal(dom.window.document.querySelector('video').hasAttribute('controls'),false);
  const source=dom.window.document.querySelector('video').getAttribute('src'); dom.window.close();
  state.isPlayerOpen=false; dom=new JSDOM(renderPlayerOverlay());
  assert.ok(dom.window.document.querySelector('.tv-preview-player #media-player'));
  assert.equal(dom.window.document.querySelector('video').getAttribute('src'),source); dom.window.close();
  state.bootstrap.current_user.id=2;
  assert.equal(renderPlayerOverlay(),'');
  state.activeTvPlayback=undefined;
});

test('TV recovery detects frozen clocks, respects pauses, and caps retries across decoder replacements',()=>{
  const watchdog=new TvPlaybackWatchdog();
  const playing={position:1000,paused:false,interrupted:false,unavailable:false};
  watchdog.attach(1000,0);
  assert.equal(watchdog.observe(playing,11999),undefined);
  assert.equal(watchdog.observe(playing,12000),'recover');
  watchdog.attach(1000,12000);
  assert.equal(watchdog.observe({...playing,interrupted:true},13000),undefined);
  assert.equal(watchdog.observe(playing,27000),'recover');
  watchdog.attach(1000,27000);
  assert.equal(watchdog.observe(playing,42000),'recover');
  watchdog.attach(1000,42000);
  assert.equal(watchdog.observe(playing,57000),'exhausted');
  assert.equal(watchdog.observe({...playing,paused:true},90000),undefined);
  assert.equal(watchdog.observe(playing,91000),undefined);
  for(let second=92;second<=124;second+=2) watchdog.observe({...playing,position:1000+(second-91)*1000},second*1000);
  assert.equal(watchdog.attempts,0);
  assert.equal(watchdog.observe({...playing,position:34000,interrupted:true},125000),'recover');
  assert.equal(watchdog.observe({...playing,unavailable:true,interrupted:true},200000),undefined);
});
test('guide mappings require the correct source and honor explicit overrides', () => {
  const rows = [program, { ...program, id: 2, source_id: 2 }, { ...program, id: 3, guide_key: 'override' }];
  assert.deepEqual(channelPrograms(channel, rows).map(p => p.id), [1]);
  assert.deepEqual(channelPrograms({ ...channel, guide_override: 'override' }, rows).map(p => p.id), [3]);
});
test('broadcast metadata is escaped and active artwork URLs never become image elements', () => {
  state.bootstrap = { current_user: { id: 1, admin: false } };
  state.liveTv = data({ programs: [{ ...program, title: '<script>bad()</script>', metadata: { description: '<img src=x onerror=bad()>', image: 'javascript:bad()', credits: ['<actor>'] } }] });
  const html = renderLiveTvPage();
  const dom = new JSDOM(html);
  assert.ok(dom.window.document.querySelector('.home-pinned-header > .home-feature > .home-feature-copy'));
  assert.ok(dom.window.document.querySelector('.home-navbar .browse-tabs'));
  assert.equal(dom.window.document.querySelector('.tv-guide-hero'),null);
  dom.window.close();
  assert.match(html, /&lt;script&gt;/); assert.match(html, /&lt;img/); assert.doesNotMatch(html, /javascript:|<script>|<img src=x/);
  assert.match(html, /Record program/); assert.match(html, /Record series/); assert.doesNotMatch(html, /data-tv-tab="setup"/);
});
test('a previous account cannot reveal its cached DVR or setup data', () => {
  state.bootstrap = { current_user: { id: 2, admin: true } };
  state.liveTv = data({ tab: 'recordings', recordings: [{ title: 'Private recording' }] });
  assert.doesNotMatch(renderLiveTvPage(), /Private recording/);
});
test('setup lives in Settings and each source has its own connection form', () => {
  state.bootstrap = { current_user: { id: 1, admin: true } };
  state.liveTv = data({ setupKind: 'm3u' });
  assert.doesNotMatch(renderLiveTvPage(), /data-tv-tab="setup"|tv-source-form/);
  assert.match(renderLiveTvPage(), /settings\/live-tv/);
  for (const [kind,required,excluded] of [
    ['m3u','input[name="url"]','[name="native_device"], [name="pin"], [name="adapter"]'],
    ['nextpvr','input[name="pin"]','[name="native_device"], [name="adapter"]'],
    ['native','select[name="region"]','[name="url"], [name="pin"], [name="adapter"]'],
  ]) {
    state.liveTv.setupKind = kind;
    const dom = new JSDOM(renderLiveTvSettings()); const form = dom.window.document.querySelector('#tv-source-form');
    assert.ok(form.querySelector(required)); assert.equal(form.querySelector(excluded),null); dom.window.close();
  }
  globalThis.location.pathname = '/settings/live-tv'; assert.deepEqual(parseRoute(), { page:'settings', section:'live-tv' }); globalThis.location.pathname = '/live-tv';
});
test('only saved captures offer playback and conflicts stay visible', () => {
  state.bootstrap = { current_user: { id: 1, admin: false } };
  state.liveTv = data({ tab: 'recordings', recordings: [{ id: 7, title: 'Scheduled', starts_at: now, ends_at: now + 3600, state: 'scheduled', has_video: false }, { id: 8, title: 'Saved', starts_at: now, ends_at: now + 3600, state: 'completed', has_video: true }], conflicts: [7] });
  const html = renderLiveTvPage(); assert.match(html, /Tuner conflict/); assert.match(html, /data-tv-recording-play="8"/); assert.doesNotMatch(html, /data-tv-recording-play="7"/);
});
test('artwork rejects credentials and local or active URLs', () => {
  for (const value of ['file:///secret', 'data:image/png,abc', 'javascript:bad()', 'https://user:secret@example.com/image', 'invalid']) assert.equal(tvImage(value), '');
  assert.equal(tvImage('https://example.com/logo.png'), 'https://example.com/logo.png');
});

test('DVR controls honor scheduling permission and shared recording ownership', () => {
  state.bootstrap = { current_user: { id: 1, admin: false } };
  state.liveTv = data({ tab: 'recordings', canRecord: false, recordings: [{ id: 9, user_id: 2, shared: true, title: 'Shared show', state: 'completed', has_video: true, starts_at: now, ends_at: now+3600 }], rules: [{ id: 5, user_id: 2, shared: true, title: 'Shared series', channel_id: 1 }] });
  const html = renderLiveTvPage();
  assert.match(html, /Shared show/); assert.match(html, /data-tv-recording-play="9"/);
  assert.match(html, /Recording is disabled/); assert.doesNotMatch(html, /data-tv-delete="9"|tv-manual-record-form|data-tv-delete-rule="5"/);
});

test('setup includes Zap2It lineup discovery, DVR storage, and per-user permissions', () => {
  state.bootstrap = { current_user: { id: 1, admin: true } };
  state.liveTv = data({ dvrSettings: { settings: { recording_dir: 'D:\\TV' }, users: [{ user_id: 2, username: '<viewer>', can_record: false }] } });
  const html = renderLiveTvSettings();
  assert.match(html, /Zap2It/); assert.doesNotMatch(html, /Gracenote/); assert.match(html, /name="guide_country"/); assert.match(html, /name="refresh_hours"/);
  assert.match(html, /DVR save folder/); assert.match(html, /data-tv-permission="2"/); assert.match(html, /&lt;viewer&gt;/);
  assert.doesNotMatch(html, /Install TSDuck/);
});

test('native frequency regions come from supported options and manual tuner entry remains available', () => {
  state.bootstrap = { current_user: { id: 1, admin: true } };
  state.liveTv = data({ setupKind:'native', regions:['Europe','North-America'] });
  const dom = new JSDOM(renderLiveTvSettings());
  const select = dom.window.document.querySelector('select[name="region"]');
  assert.deepEqual([...select.options].map(o=>o.value),['Europe','North-America']);
  assert.equal(select.value,'North-America'); assert.ok(dom.window.document.querySelector('input[name="native_device"]')); dom.window.close();
});

test('the settings route requests administrator setup data without needing a Live TV setup tab', async () => {
  const target = app({ route:{page:'settings',section:'live-tv'}, bootstrap:{current_user:{id:1,admin:true}} });
  let requested = false;
  await loadLiveTv(target,()=>true,api({getTvChannels:async setup=>{assert.equal(setup,true);return[channel];},getTvSources:async()=>{requested=true;return[];},getTvDvrSettings:async()=>({settings:{recording_dir:''},users:[]})}));
  assert.ok(requested); assert.ok(target.liveTv.dvrSettings);
});
test('non-administrators never request setup endpoints', async () => {
  const target = app();
  await loadLiveTv(target, () => true, api({ getTvChannels: async setup => { assert.equal(setup, false); return [channel]; }, getTvSources: async () => { throw new Error('Setup must not be requested'); } }));
  assert.equal(target.liveTv.error, undefined); assert.equal(target.liveTv.programs.length, 1);
});
test('late account responses cannot publish private recording data', async () => {
  const target = app(); let release;
  const request = loadLiveTv(target, () => true, api({ getTvRecordings: () => new Promise(resolve => { release = resolve; }) }));
  target.bootstrap.current_user.id = 2;
  release({ recordings: [{ title: 'Private' }], conflicts: [] }); await request;
  assert.equal(target.liveTv.recordings.length, 0);
});
test('a slow previous guide window cannot overwrite a more recent navigation', async () => {
  const target = app(); let release;
  const first = loadLiveTv(target, () => true, api({ getTvGuide: () => new Promise(resolve => { release = resolve; }) }));
  target.liveTv.windowStart += 21600;
  await loadLiveTv(target, () => true, api({ getTvGuide: async () => ({ programs: [{ ...program, title: 'New window' }], stations: [] }) }));
  release({ programs: [{ ...program, title: 'Old window' }], stations: [] }); await first;
  assert.equal(target.liveTv.programs[0].title, 'New window');
});
