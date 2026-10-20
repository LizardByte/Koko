import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';
globalThis.location = { pathname: '/', origin: 'http://localhost' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { AudioQueue } = await server.ssrLoadModule('/src/app/audioQueue.ts');
const { parseTimedLyrics, renderAudioPlayer } = await server.ssrLoadModule('/src/app/audioPlayerView.ts');
const mock = await server.ssrLoadModule('/src/mockApi.ts');
const songs = [1,2,3,4,5].map((id) => ({ ...mock.getMockItem(103), id, display_title: `Song ${id}` }));

test('play next and append preserve duplicates, the playing entry, and previous history', () => {
  const queue = new AudioQueue(); queue.replace(songs, 1);
  const current = queue.current.key;
  queue.enqueue([songs[1]], true); queue.enqueue([songs[0]]);
  assert.equal(queue.current.key, current);
  assert.deepEqual(queue.ordered.map((entry) => entry.item.id), [1,2,2,3,4,5,1]);
  queue.advance(); assert.equal(queue.current.item.id, 2); assert.notEqual(queue.current.key, current);
  queue.previous(); assert.equal(queue.current.key, current);
});
test('shuffle changes only upcoming entries, has no repeated entries, and previous follows actual history', () => {
  const queue = new AudioQueue(() => 0); queue.replace(songs, 1); queue.toggleShuffle();
  assert.deepEqual(queue.ordered.slice(0,2).map((entry) => entry.item.id), [1,2]);
  assert.deepEqual(queue.ordered.slice(2).map((entry) => entry.item.id), [4,5,3]);
  assert.equal(new Set(queue.order).size, songs.length);
  queue.advance(); const key = queue.current.key; queue.advance(); queue.previous(); assert.equal(queue.current.key, key);
  const played = queue.order.slice(0, queue.cursor + 1); queue.toggleShuffle(); assert.deepEqual(queue.order.slice(0,queue.cursor+1), played);
});
test('repeat one applies only to automatic completion, while repeat all wraps and off stops', () => {
  const queue = new AudioQueue(); queue.replace(songs.slice(0,2)); queue.repeat = 'one';
  assert.equal(queue.advance(true), true); assert.equal(queue.current.item.id, 1);
  assert.equal(queue.advance(false), true); assert.equal(queue.current.item.id, 2);
  queue.repeat = 'off'; assert.equal(queue.advance(true), false); assert.equal(queue.current.item.id, 2);
  queue.repeat = 'all'; assert.equal(queue.advance(true), true); assert.equal(queue.current.item.id, 1);
  assert.equal(queue.previous(), true); assert.equal(queue.current.item.id, 2);
});
test('queue edits cannot remove the playing song or move upcoming songs into played history', () => {
  const queue = new AudioQueue(); queue.replace(songs, 1); const key = queue.current.key;
  queue.remove(key); assert.equal(queue.current.key, key);
  queue.remove(queue.entries[0].key); assert.equal(queue.current.key, key); assert.equal(queue.cursor, 0);
  queue.move(queue.order[1], -1); assert.equal(queue.current.key, key);
  queue.move(queue.order[2], -1); assert.deepEqual(queue.ordered.map((entry) => entry.item.id), [2,4,3,5]);
  queue.clearUpcoming(); assert.deepEqual(queue.ordered.map((entry) => entry.item.id), [2]);
  queue.replace([]); assert.equal(queue.advance(), false); assert.equal(queue.current, undefined);
});
test('timed lyrics handle repeated timestamps, offsets, decimals, and malformed metadata', () => {
  assert.deepEqual(parseTimedLyrics('[offset:-500]\n[ar:Someone]\n[00:02.50][01:00]Hello <world>\n[bad]Unknown\n[00:01.2]First'), [
    { time: 0.7, text: 'First' }, { time: 2, text: 'Hello <world>' }, { time: 59.5, text: 'Hello <world>' },
  ]);
});
test('music transport presents queue and lyrics without video or fullscreen controls and escapes content', () => {
  const queue = new AudioQueue(); queue.replace([{...songs[0], display_title: '<script>bad</script>'}]);
  const view = { queue, item: queue.current.item, loading: false, paused: true, volume: 1, muted: false, panel: 'queue', lyricsLoading: false, saving: false };
  const html = renderAudioPlayer(view);
  for (const label of ['Next song','Previous song','Back 10 seconds','Forward 10 seconds','Shuffle','Repeat off','Save as playlist','Lyrics']) assert.ok(html.includes(label));
  assert.match(html, /&lt;script&gt;bad/);
  assert.equal(html.includes('<script'), false);
  assert.equal(html.includes('<video'), false);
  assert.equal(html.includes('Fullscreen'), false);
  view.panel = 'lyrics'; view.lyrics = { plain_lyrics: '<unsafe>', source: 'LRCLIB', source_url: 'https://lrclib.net' };
  assert.match(renderAudioPlayer(view), /&lt;unsafe&gt;/);
});

test('sign-out cancels an album lookup before it can create an audio element or a session', async () => {
  const { state } = await server.ssrLoadModule('/src/app/state.ts');
  const player = await server.ssrLoadModule('/src/app/audioPlayer.ts');
  const originalFetch = globalThis.fetch;
  let complete;
  globalThis.fetch = () => new Promise((resolve) => { complete = resolve; });
  state.bootstrap = { current_user: { id: 1 } };
  try {
    const pending = player.playMusicItem(mock.getMockItem(103));
    assert.equal(typeof complete, 'function');
    player.stopMusic();
    state.bootstrap = { current_user: undefined };
    complete(new Response(JSON.stringify(mock.getMockItem(302)), { headers: { 'Content-Type': 'application/json' } }));
    await pending;
    assert.equal(player.audioQueue.current, undefined);
    await assert.rejects(player.playMusicItems(songs), /Sign in/);
  } finally { globalThis.fetch = originalFetch; }
});
