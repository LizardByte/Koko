import assert from 'node:assert/strict';
import { after, beforeEach, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/podcasts', origin: 'https://koko.local' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const { renderPodcastFeature } = await server.ssrLoadModule('/src/app/podcasts.ts');
const { featureArtworkStyle } = await server.ssrLoadModule('/src/app/homeView.ts');
const { bindPodcastDirectoryPreviews, bindPodcastAddActions } = await server.ssrLoadModule('/src/app/podcastActions.ts');
const mock = await server.ssrLoadModule('/src/mockApi.ts');

function show(id, title) {
  return { ...mock.getMockItem(302), id, item_type: 'podcast', display_title: title, overview: title + ' description',
    hierarchy: [], parent_id: null, audiobook: null, music: null, podcast: { podcast_id: id } };
}

beforeEach(() => {
  state.route = { page: 'podcasts' };
  state.bootstrap = { current_user: { id: 1 } };
  state.searchQuery = ''; state.searchResults = []; state.homePreviewItemId = undefined;
  state.podcasts = { owner: 1, tab: 'library', items: [show(1, 'Subscribed A'), show(2, 'Subscribed B')],
    directory: [
      { id: 'directory-a', title: 'Result A', author: 'Publisher A', feed_url: 'https://example.com/feed-a', artwork_url: 'https://example.com/a.jpg' },
      { id: 'directory-b', title: 'Result B', author: 'Publisher B', feed_url: 'https://example.com/feed-b', artwork_url: 'https://example.com/b.jpg' },
    ], country: 'us', query: '', loading: false };
});

test('library and recommended heroes follow the highlighted subscription or shelf item', () => {
  state.homePreviewItemId = 2;
  assert.match(renderPodcastFeature(), /<h2>Subscribed B<\/h2>/);
  state.podcasts.tab = 'recommended';
  state.podcasts.home = { shelves: [{ items: [show(3, 'A listening shelf item')] }] };
  state.homePreviewItemId = undefined;
  assert.match(renderPodcastFeature(), /<h2>A listening shelf item<\/h2>/);
  state.homePreviewItemId = 3;
  assert.match(renderPodcastFeature(), /data-item-id="3"/);
  assert.match(renderPodcastFeature(), /<h2>A listening shelf item<\/h2>/);
});

test('Discover uses current results even when subscriptions and library highlights exist', () => {
  state.podcasts.tab = 'discover'; state.homePreviewItemId = 2;
  assert.match(renderPodcastFeature(), /<h2>Result A<\/h2>/);
  state.podcasts.previewDirectoryId = 'directory-b';
  const html = renderPodcastFeature();
  assert.match(html, /<h2>Result B<\/h2>/);
  assert.match(html, /Publisher B/);
  assert.match(html, /https:\/\/example.com\/b.jpg/);
  assert.match(html, /home-feature-podcast has-artwork/);
  assert.match(html, /--home-feature-image: url\('https:\/\/example.com\/b.jpg'\)/);
  assert.match(html, /data-podcast-add="1"/);
  assert.doesNotMatch(html, /Subscribed [AB]/);
});

test('directory heroes use available descriptions, genres, and episode counts without fetching', () => {
  state.podcasts.tab = 'discover';
  Object.assign(state.podcasts.directory[0], { description: 'Full show description <script>unsafe</script>', genres: ['Science', 'Podcasts'], episode_count: 42 });
  const html = renderPodcastFeature();
  assert.match(html, /Full show description &lt;script&gt;unsafe&lt;\/script&gt;/);
  assert.match(html, /<span class="tag">Science<\/span>/);
  assert.match(html, /42 episodes/);
  assert.doesNotMatch(html, /<span class="tag">Podcasts<\/span>|<script>/i);
});

test('an added show can reuse its saved description while unknown shows retain their publisher', () => {
  state.podcasts.tab = 'discover';
  state.podcasts.items[0].podcast.feed_url = 'https://example.com/feed-a';
  assert.match(renderPodcastFeature(), /Subscribed A description/);
  state.podcasts.previewDirectoryId = 'directory-b';
  assert.match(renderPodcastFeature(), /<p>Publisher B<\/p>/);
  assert.doesNotMatch(renderPodcastFeature(), /Subscribed A description/);
});

test('cover backgrounds keep CSS and markup characters inside the publisher URL', () => {
  const style = featureArtworkStyle("https://example.com/cover');color:red;('image.png?x=1&y=2");
  assert.match(style, /cover%27\);color:red;\(%27image.png/);
  assert.match(style, /x=1&amp;y=2/);
  assert.doesNotMatch(style, /cover'\);/);
  state.podcasts.tab = 'discover'; state.podcasts.directory[0].artwork_url = 'javascript:bad()';
  assert.doesNotMatch(renderPodcastFeature(), /has-artwork|--home-feature-image|javascript:/);
});

test('a new result set cannot retain an old directory preview or fall back to subscriptions', () => {
  state.podcasts.tab = 'discover'; state.podcasts.previewDirectoryId = 'directory-b';
  state.podcasts.directory = [{ id: 'different-result', title: 'New query result', feed_url: 'https://example.com/new' }];
  assert.match(renderPodcastFeature(), /<h2>New query result<\/h2>/);
  assert.match(renderPodcastFeature(), /data-podcast-add="0"/);
  state.podcasts.directory = [];
  assert.doesNotMatch(renderPodcastFeature(), /Subscribed [AB]|New query result|Result B/);
  assert.match(renderPodcastFeature(), /<h2>Podcasts<\/h2>/);
});

test('global Koko search previews remain available while browsing podcasts', () => {
  state.podcasts.tab = 'discover'; state.searchQuery = 'a movie';
  state.searchResults = [{ result_type: 'item', item: { ...show(40, 'A movie outside podcasts'), item_type: 'movie', podcast: null } }];
  state.homePreviewItemId = 40;
  assert.match(renderPodcastFeature(), /A movie outside podcasts/);
  assert.match(renderPodcastFeature(), /data-item-id="40"/);
});

class PreviewCard extends EventTarget {
  isConnected = true;
  hovered = true;
  constructor(id) { super(); this.dataset = { podcastPreviewId: id }; }
  matches() { return this.hovered; }
}

test('directory hover uses the shared delay; focus is immediate and stale cards cannot select', t => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  state.podcasts.tab = 'discover';
  const first = new PreviewCard('directory-a'), second = new PreviewCard('directory-b');
  let refreshed = 0;
  bindPodcastDirectoryPreviews({ querySelectorAll: () => [first, second] }, () => { refreshed++; });
  second.dispatchEvent(new Event('mouseenter')); t.mock.timers.tick(219);
  assert.equal(refreshed, 0);
  second.dispatchEvent(new Event('mouseleave')); t.mock.timers.tick(500);
  assert.equal(refreshed, 0);
  second.dispatchEvent(new Event('mouseenter')); t.mock.timers.tick(220);
  assert.equal(state.podcasts.previewDirectoryId, 'directory-b');
  assert.equal(refreshed, 1);
  first.dispatchEvent(new Event('focusin'));
  assert.equal(state.podcasts.previewDirectoryId, 'directory-a');
  assert.equal(refreshed, 2);
  state.podcasts.directory = [state.podcasts.directory[0]];
  second.dispatchEvent(new Event('focus')); assert.equal(refreshed, 2);
  state.bootstrap.current_user.id = 2; state.podcasts.owner = 2;
  state.podcasts.previewDirectoryId = undefined;
  first.dispatchEvent(new Event('focus')); assert.equal(refreshed, 2);
});

test('the Add action on a replaced hero subscribes to its selected result exactly once', async () => {
  const originalFetch = globalThis.fetch;
  const button = new EventTarget(); button.dataset = { podcastAdd: '1' };
  const requests = [];
  let refreshed = 0;
  globalThis.fetch = async (_url, options) => { requests.push(JSON.parse(options.body)); return Response.json({ item_id: 22 }); };
  try {
    bindPodcastAddActions({ querySelectorAll: () => [button] }, { render() {}, async refreshData() { refreshed++; } });
    button.dispatchEvent(new Event('click'));
    await new Promise(resolve => setImmediate(resolve));
    assert.deepEqual(requests, [{ feed_url: 'https://example.com/feed-b' }]);
    assert.equal(refreshed, 1);
    state.bootstrap.current_user.id = 2;
    button.dispatchEvent(new Event('click'));
    assert.equal(requests.length, 1);
  } finally { globalThis.fetch = originalFetch; }
});
