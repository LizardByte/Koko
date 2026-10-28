import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/podcasts', origin: 'https://koko.local' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { podcastLink, orderedPodcastEpisodes, renderPodcastPage, renderPodcastItem } = await server.ssrLoadModule('/src/app/podcasts.ts');
const { parseRoute } = await server.ssrLoadModule('/src/app/routes.ts');
const { AudioQueue } = await server.ssrLoadModule('/src/app/audioQueue.ts');
const { renderAudioPlayer } = await server.ssrLoadModule('/src/app/audioPlayerView.ts');
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const mock = await server.ssrLoadModule('/src/mockApi.ts');

function episode(id, extra = {}) {
  return { ...mock.getMockItem(302), id, item_type: 'podcast_episode', media_kind: 'audio', playable: true, audiobook: null, music: null,
    display_title: 'Episode ' + id, overview: 'Show notes', hierarchy: [], children: [],
    podcast: { podcast_id: 100, author: 'Publisher', categories: [], transcripts: [], people: [], published_at: id * 1000 }, ...extra };
}

test('podcast navigation has a dedicated persistent route', () => {
  assert.deepEqual(parseRoute(), { page: 'podcasts' });
});
test('publisher links reject script, file, and credential URLs', () => {
  for (const url of ['javascript:alert(1)', 'data:text/html,hi', 'file:///private', 'https://user:password@example.com', 'invalid']) assert.equal(podcastLink(url), undefined);
  assert.equal(podcastLink('https://example.com/notes'), 'https://example.com/notes');
});
test('serial episodes retain story order and episodic shows list newest first without mutation', () => {
  const episodes = [episode(3, { season_number: 2, episode_number: 1 }), episode(2, { season_number: 1, episode_number: 2 }), episode(1, { season_number: 1, episode_number: 1 })];
  assert.deepEqual(orderedPodcastEpisodes(episodes, true).map(item => item.id), [1, 2, 3]);
  assert.deepEqual(orderedPodcastEpisodes(episodes).map(item => item.id), [3, 2, 1]);
  assert.deepEqual(episodes.map(item => item.id), [3, 2, 1]);
});
test('directory failure preserves subscriptions and manual RSS entry and hides another account data', () => {
  state.bootstrap = { current_user: { id: 1 } };
  state.podcasts = { owner: 1, items: [episode(100, { item_type: 'podcast', display_title: 'Private show' })], directory: [], country: 'us', query: '', loading: false, error: 'Directory is unavailable' };
  const html = renderPodcastPage();
  assert.match(html, /Private show/);
  assert.match(html, /podcast-feed-form/);
  assert.match(html, /Directory is unavailable/);
  state.bootstrap.current_user.id = 2;
  assert.doesNotMatch(renderPodcastPage(), /Private show/);
});
test('show hierarchy links seasons without inventing a season for unnumbered episodes', () => {
  const show = episode(100, { item_type: 'podcast', playable: false, children: [episode(1), episode(2, { item_type: 'podcast_season', playable: false, display_title: 'Season 2', season_number: 2, child_count: 3 })] });
  const html = renderPodcastItem(show);
  assert.match(html, /Season 2/);
  assert.match(html, /Other episodes/);
  assert.match(html, /data-podcast-play="1"/);
  assert.doesNotMatch(html, /Season 0|Season 1/);
});
test('episode metadata, transcripts, and progress render safely without management internals', () => {
  const item = episode(1, { display_title: '<img src=x onerror=bad()>', playback_position_ms: 12000, playback_completed: false });
  item.podcast.transcripts = [{ url: 'https://example.com/transcript.vtt', language: 'en' }, { url: 'javascript:bad()' }];
  item.podcast.chapters_url = 'https://example.com/chapters.json';
  const html = renderPodcastItem(item);
  assert.match(html, /data-podcast-position="12000"/);
  assert.match(html, /Resume/);
  assert.match(html, /Transcript 1 \(en\)/);
  assert.match(html, /Episode chapters/);
  assert.doesNotMatch(html, /javascript:|<img src=x|Source files|Link metadata/);
  assert.match(html, /&lt;img/);
});
test('podcast transport provides speed and seeking without lyric controls', () => {
  const item = episode(1), queue = new AudioQueue();
  queue.replace([item]);
  const html = renderAudioPlayer({ item, queue, loading: false, paused: true, volume: 1, muted: false, playbackRate: 1.5 });
  assert.match(html, /Playback speed/);
  assert.match(html, /Back 10 seconds/);
  assert.doesNotMatch(html, /data-music-action="lyrics"|Shuffle|Repeat off/);
});
