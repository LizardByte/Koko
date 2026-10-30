/** Tests account isolation, station mapping, safe metadata rendering, and guide request races. */
import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/live-tv', origin: 'https://koko.local' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { channelPrograms, renderLiveTvPage, tvImage } = await server.ssrLoadModule('/src/app/liveTv.ts');
const { loadLiveTv } = await server.ssrLoadModule('/src/app/liveTvActions.ts');
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const { parseRoute } = await server.ssrLoadModule('/src/app/routes.ts');
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
test('guide mappings require the correct source and honor explicit overrides', () => {
  const rows = [program, { ...program, id: 2, source_id: 2 }, { ...program, id: 3, guide_key: 'override' }];
  assert.deepEqual(channelPrograms(channel, rows).map(p => p.id), [1]);
  assert.deepEqual(channelPrograms({ ...channel, guide_override: 'override' }, rows).map(p => p.id), [3]);
});
test('broadcast metadata is escaped and active artwork URLs never become image elements', () => {
  state.bootstrap = { current_user: { id: 1, admin: false } };
  state.liveTv = data({ programs: [{ ...program, title: '<script>bad()</script>', metadata: { description: '<img src=x onerror=bad()>', image: 'javascript:bad()', credits: ['<actor>'] } }] });
  const html = renderLiveTvPage();
  assert.match(html, /&lt;script&gt;/); assert.match(html, /&lt;img/); assert.doesNotMatch(html, /javascript:|<script>|<img src=x/);
  assert.match(html, /Record program/); assert.match(html, /Record series/); assert.doesNotMatch(html, /data-tv-tab="setup"/);
});
test('a previous account cannot reveal its cached DVR or setup data', () => {
  state.bootstrap = { current_user: { id: 2, admin: true } };
  state.liveTv = data({ tab: 'recordings', recordings: [{ title: 'Private recording' }] });
  assert.doesNotMatch(renderLiveTvPage(), /Private recording/);
});
test('setup offers native broadcast guides without a direct Schedules Direct account', () => {
  state.bootstrap = { current_user: { id: 1, admin: true } };
  state.liveTv = data({ tab: 'setup' });
  const html = renderLiveTvPage();
  assert.match(html, /Windows BDA \/ Linux DVB/); assert.match(html, /DVB EIT \/ ATSC PSIP/); assert.match(html, /NextPVR/); assert.match(html, /Import|XMLTV/);
  assert.doesNotMatch(html, /Schedules Direct|Find lineups|name="password"/);
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

test('setup includes Gracenote lineup discovery, DVR storage, and per-user permissions', () => {
  state.bootstrap = { current_user: { id: 1, admin: true } };
  state.liveTv = data({ tab: 'setup', dvrSettings: { settings: { recording_dir: 'D:\\TV' }, users: [{ user_id: 2, username: '<viewer>', can_record: false }] } });
  const html = renderLiveTvPage();
  assert.match(html, /Gracenote \(formerly Zap2It\)/); assert.match(html, /name="guide_country"/); assert.match(html, /name="refresh_hours"/);
  assert.match(html, /DVR save folder/); assert.match(html, /data-tv-permission="2"/); assert.match(html, /&lt;viewer&gt;/);
  assert.doesNotMatch(html, /Install TSDuck/);
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
