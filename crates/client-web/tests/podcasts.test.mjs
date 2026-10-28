import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/podcasts', origin: 'https://koko.local' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { podcastLink, orderedPodcastEpisodes, renderPodcastPage, renderPodcastContents, renderPodcastControls, renderPodcastDetails } = await server.ssrLoadModule('/src/app/podcasts.ts');
const { renderItemPage } = await server.ssrLoadModule('/src/app/itemPersonView.ts');
const { parseRoute } = await server.ssrLoadModule('/src/app/routes.ts');
const { AudioQueue } = await server.ssrLoadModule('/src/app/audioQueue.ts');
const { renderAudioPlayer } = await server.ssrLoadModule('/src/app/audioPlayerView.ts');
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const mock = await server.ssrLoadModule('/src/mockApi.ts');

function episode(id, extra = {}) {
  return { ...mock.getMockItem(302), id, item_type: 'podcast_episode', media_kind: 'audio', playable: true, audiobook: null, music: null,
    has_metadata: true, display_title: 'Episode ' + id, overview: 'Show notes', hierarchy: [], children: [],
    podcast: { podcast_id: 100, author: 'Publisher', categories: [], transcripts: [], people: [], published_at: id * 1000 }, ...extra };
}

test('podcast navigation has a dedicated persistent route', () => {
  assert.deepEqual(parseRoute(), { page: 'podcasts' });
});
test('publisher links reject script, file, and credential URLs', () => {
  for (const url of ['javascript:alert(1)', 'data:text/html,hi', 'file:///private', 'https://user:password@example.com', 'invalid']) assert.equal(podcastLink(url), undefined);
  assert.equal(podcastLink('https://example.com/notes'), 'https://example.com/notes');
});
test('episodes default oldest first and can reverse chronologically without mutation', () => {
  const episodes = [episode(3, { season_number: 2, episode_number: 1 }), episode(2, { season_number: 1, episode_number: 2 }), episode(1, { season_number: 1, episode_number: 1 })];
  assert.deepEqual(orderedPodcastEpisodes(episodes, 'oldest_first').map(item => item.id), [1, 2, 3]);
  assert.deepEqual(orderedPodcastEpisodes(episodes, 'newest_first').map(item => item.id), [3, 2, 1]);
  assert.deepEqual(episodes.map(item => item.id), [3, 2, 1]);
});
test('directory failure preserves subscriptions and manual RSS entry and hides another account data', () => {
  state.bootstrap = { current_user: { id: 1 } };
  state.podcasts = { owner: 1, items: [episode(100, { item_type: 'podcast', display_title: 'Private show' })], directory: [], country: 'us', query: '', loading: false, error: 'Directory is unavailable', addFeedOpen: true };
  const html = renderPodcastPage();
  assert.match(html, /Private show/);
  assert.match(html, /podcast-feed-form/);
  state.podcasts.tab = 'discover';
  assert.match(renderPodcastPage(), /Directory is unavailable/);
  state.bootstrap.current_user.id = 2;
  assert.doesNotMatch(renderPodcastPage(), /Private show/);
});
test('podcast item pages share music layout and TV season cards including year groups', () => {
  const numbered = episode(2, { item_type: 'podcast_season', playable: false, display_title: 'Season 2', season_number: 2, child_count: 3 });
  const year = episode(3, { item_type: 'podcast_season', playable: false, display_title: '2025', child_count: 4 });
  year.podcast.season_year = 2025;
  const show = episode(100, { item_type: 'podcast', playable: false, children: [numbered, year] });
  state.selectedItem = show;
  state.route = { page: 'item', itemId: 100 };
  state.bootstrap = { current_user: { id: 1, admin: true } };
  const html = renderItemPage();
  assert.match(html, /music-detail-page/);
  assert.match(html, /item-identity/);
  assert.match(html, /Season 2/);
  assert.match(html, /2025/);
  assert.match(html, /4 episodes/);
  assert.match(html, /data-podcast-order="100"/);
  assert.doesNotMatch(html, /Other episodes|Source files|Link metadata/);
  const season = { ...year, children: [episode(5), episode(4)] };
  const rows = renderPodcastContents(season);
  assert.match(rows, /music-track-list/);
  assert.ok(rows.indexOf('data-podcast-play="5"') < rows.indexOf('data-podcast-play="4"'));
  assert.match(renderPodcastControls(show), /value="oldest_first" selected/);
});

test('episode metadata, transcripts, and progress render safely without management internals', () => {
  const item = episode(1, { display_title: '<img src=x onerror=bad()>', playback_position_ms: 12000, playback_completed: false });
  item.podcast.transcripts = [{ url: 'https://example.com/transcript.vtt', language: 'en' }, { url: 'javascript:bad()' }];
  item.podcast.chapters_url = 'https://example.com/chapters.json';
  const html = renderPodcastContents({ ...item, playable: false, children: [item] }) + renderPodcastDetails(item);
  assert.match(html, /data-podcast-position="12000"/);
  assert.match(html, /Downloads on first play/);
  assert.match(html, /Transcript 1 \(en\)/);
  assert.match(html, /Episode chapters/);
  assert.doesNotMatch(html, /javascript:|<img src=x|Source files|Link metadata/);
  assert.match(html, /&lt;img/);
  item.podcast.download = { state: 'ready', size_bytes: 1234 };
  assert.match(renderPodcastDetails(item), /Downloaded/);
});
test('podcast transport provides speed and seeking without lyric controls', () => {
  const item = episode(1), queue = new AudioQueue();
  queue.replace([item]);
  const html = renderAudioPlayer({ item, queue, loading: false, paused: true, volume: 1, muted: false, playbackRate: 1.5 });
  assert.match(html, /Playback speed/);
  assert.match(html, /Back 10 seconds/);
  assert.doesNotMatch(html, /data-music-action="lyrics"|Shuffle|Repeat off/);
});

test('undated episodes stay last in either playback order', () => {
  const unknown = episode(8);
  unknown.podcast.published_at = null;
  for (const order of ['oldest_first', 'newest_first']) {
    const result = orderedPodcastEpisodes([unknown, episode(2), episode(1)], order);
    assert.equal(result.at(-1).id, 8);
  }
});

test('podcast queues show episodes without music playlist actions', () => {
  const item = episode(1), queue = new AudioQueue();
  queue.replace([item, episode(2)]);
  const html = renderAudioPlayer({ item, queue, panel: 'queue', loading: false, paused: true, volume: 1, muted: false });
  assert.match(html, /2 episodes/);
  assert.doesNotMatch(html, /Save queue|playlist-name|Shuffle|Lyrics/);
});


test('switching accounts cancels a pending podcast queue before an audio session starts', async () => {
  const player = await server.ssrLoadModule('/src/app/audioPlayer.ts');
  const originalFetch = globalThis.fetch;
  const owner = state.bootstrap;
  let complete;
  globalThis.fetch = () => new Promise(resolve => { complete = resolve; });
  state.bootstrap = { current_user: { id: 1 } };
  try {
    const pending = player.playPodcastItem(episode(1));
    assert.equal(typeof complete, 'function');
    state.bootstrap = { current_user: { id: 2 } };
    complete(new Response(JSON.stringify([episode(1), episode(2)]), { headers: { 'Content-Type': 'application/json' } }));
    await pending;
    assert.equal(player.audioQueue.current, undefined);
  } finally { globalThis.fetch = originalFetch; state.bootstrap = owner; }
});


test('podcast navbar exposes recommendations, global search, a cover-based hero, and all-Koko results', () => {
  const item = episode(100, { item_type: 'podcast', playable: false, display_title: 'Subscribed show' });
  state.bootstrap = { current_user: { id: 1 } };
  state.searchQuery = ''; state.showFullSearchResults = false;
  state.podcasts = { owner: 1, items: [item], directory: [], country: 'us', query: '', loading: false,
    home: { library_id: 3, shelves: [{ id: 'recommended', title: 'Recommended', items: [item] }] } };
  const html = renderPodcastPage();
  assert.match(html, /data-podcast-tab="recommended"/);
  assert.match(html, /id="search-form"/);
  assert.match(html, /podcast-feature-cover/);
  assert.match(html, /Subscribed show/);
  state.searchQuery = 'movie'; state.showFullSearchResults = true;
  state.searchResults = [{ result_type: 'item', item: { ...mock.getMockItem(101), display_title: 'A movie outside podcasts' } }];
  assert.match(renderPodcastPage(), /A movie outside podcasts/);
  state.searchQuery = ''; state.showFullSearchResults = false; state.searchResults = [];
});

test('discovery uses cover overlay actions and a publisher menu', () => {
  state.podcasts = { owner: 1, tab: 'discover', items: [], directory: [{ id: '9', title: 'New show', feed_url: 'https://example.com/rss', apple_url: 'https://podcasts.apple.com/show/9' }], country: 'us', query: '', loading: false };
  const html = renderPodcastPage();
  assert.match(html, /podcast-cover-add/);
  assert.match(html, /aria-label="Add New show"/);
  assert.match(html, /popovertarget="podcast-directory-0"/);
  assert.match(html, /Open in Apple Podcasts/);
});

test('season display stays newest first independently of playback order and exposes another page', () => {
  const season = episode(99, { item_type: 'podcast_season', playable: false, child_count: 1000, children: [episode(1), episode(2)] });
  season.podcast.playback_order = 'oldest_first';
  const html = renderPodcastContents(season);
  assert.ok(html.indexOf('data-podcast-play="2"') < html.indexOf('data-podcast-play="1"'));
  assert.match(html, /2 of 1000 episodes/);
  assert.match(html, /data-podcast-more="99"/);
  assert.match(html, /aria-label="Downloads on first play"/);
  assert.doesNotMatch(html, /· Downloads on first play/);
});

test('the playback cover opens the podcast season', () => {
  const item = episode(1, { parent_id: 45 }), queue = new AudioQueue();
  queue.replace([item]);
  const html = renderAudioPlayer({ item, queue, loading: false, paused: true, volume: 1, muted: false });
  assert.match(html, /data-music-open-item="45" aria-label="Open podcast season"/);
});


test('chronological episode numbers increase toward newer releases across loaded pages', () => {
  const season = episode(900, { item_type: 'podcast_season', playable: false, child_count: 736, children: [episode(4), episode(3), episode(2)] });
  const numbers = html => [...html.matchAll(/music-track-number">(\d+)<\/span>/g)].map(match => Number(match[1]));
  assert.deepEqual(numbers(renderPodcastContents(season)), [736, 735, 734]);
  season.podcast.playback_order = 'newest_first';
  assert.deepEqual(numbers(renderPodcastContents(season)), [736, 735, 734]);
  season.children.push(episode(1));
  assert.deepEqual(numbers(renderPodcastContents(season)), [736, 735, 734, 733]);
  season.children[1].episode_number = 42;
  assert.deepEqual(numbers(renderPodcastContents(season)), [736, 42, 734, 733]);
});

test('podcast pages use the existing Settings page and respect server availability', () => {
  state.bootstrap = { current_user: { id: 1, admin: false }, podcasts_enabled: true };
  assert.doesNotMatch(renderPodcastPage(), /Podcast settings|podcast-personal-settings|Show podcasts on Home/);
  state.bootstrap.podcasts_enabled = false;
  assert.match(renderPodcastPage(), /disabled by your administrator/);
  assert.doesNotMatch(renderPodcastPage(), /Add RSS feed/);
});

test('Retry repeats the failed podcast operation and refreshes after success', async () => {
  const { executePodcastAction, retryPodcastAction } = await server.ssrLoadModule('/src/app/podcastActions.ts');
  state.bootstrap = { current_user: { id: 1 } }; state.podcasts.busy = false;
  let attempts = 0, refreshed = 0, completed = 0;
  const context = { render() {}, async refreshData() { refreshed++; } };
  await executePodcastAction(context, async () => { attempts++; if (attempts === 1) { throw new Error('Publisher unavailable'); } }, () => { completed++; });
  assert.equal(attempts, 1); assert.equal(refreshed, 0);
  assert.equal(state.error, 'Publisher unavailable');
  assert.equal(await retryPodcastAction(), true);
  assert.equal(attempts, 2); assert.equal(refreshed, 1); assert.equal(completed, 1);
  assert.equal(state.error, undefined);
  assert.equal(await retryPodcastAction(), false);
});

test('Retry cannot replay an abandoned account request or an unrelated error', async () => {
  const { executePodcastAction, retryPodcastAction } = await server.ssrLoadModule('/src/app/podcastActions.ts');
  state.bootstrap = { current_user: { id: 1 } }; state.podcasts.busy = false;
  let attempts = 0;
  await executePodcastAction({ render() {}, async refreshData() {} }, async () => { attempts++; throw new Error('Failed add'); });
  state.bootstrap = { current_user: { id: 2 } };
  assert.equal(await retryPodcastAction(), false);
  state.bootstrap.current_user.id = 1;
  const failedRoute = state.route;
  state.route = { page: 'home' };
  assert.equal(await retryPodcastAction(), false);
  state.route = failedRoute; state.error = 'Different failure';
  assert.equal(await retryPodcastAction(), false); assert.equal(attempts, 1);
});
