import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/', origin: 'http://localhost' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const { renderHomeShelfEntry, renderItemCard, renderPlaylistDetailPage } = await server.ssrLoadModule('/src/app/homeView.ts');
const { playlistPlaybackButtonContent, renderPlaylistTracks } = await server.ssrLoadModule('/src/app/music.ts');
const { musicCardStatus, setMusicPlayback, clearMusicPlayback, activeMusicPlayback, musicPlaybackPlaylist } = await server.ssrLoadModule('/src/app/musicPlaybackState.ts');
const { configureAudioPlayer, resumeMusicQueue, audioQueue } = await server.ssrLoadModule('/src/app/audioPlayer.ts');
const { savePlaylist, getArtworkUrl } = await server.ssrLoadModule('/src/api.ts');
const mock = await server.ssrLoadModule('/src/mockApi.ts');
const song = { ...mock.getMockItem(103), artwork_item_id: 302 }, album = mock.getMockItem(302);
const playback = { userId: 1, item: song, positionMs: 123000, durationMs: 215000, paused: false, ended: false };

test('playlist transport labels follow its own active queue and saved progress', () => {
  const detail = { playlist: { id: 77, name: 'Radio', temporary: true, current_index: 1, position_ms: 50000 }, items: [song, mock.getMockItem(104)] };
  try {
    assert.match(playlistPlaybackButtonContent(detail), /Continue radio/);
    for (const paused of [false, true]) {
      setMusicPlayback({ ...playback, queueId: 77, paused });
      assert.match(playlistPlaybackButtonContent(detail), new RegExp(`${paused ? 'Resume' : 'Pause'} radio`));
      assert.match(playlistPlaybackButtonContent(detail), /Mock Song/);
      assert.match(playlistPlaybackButtonContent({ ...detail, playlist: { ...detail.playlist, temporary: false } }), new RegExp(`${paused ? 'Resume' : 'Pause'} playlist`));
      assert.match(playlistPlaybackButtonContent({ ...detail, playlist: { ...detail.playlist, id: 78 } }), /Continue radio/);
    }
    setMusicPlayback({ ...playback, queueId: 77, ended: true });
    assert.match(playlistPlaybackButtonContent(musicPlaybackPlaylist(detail)), /Play radio/);
    clearMusicPlayback();
    assert.match(playlistPlaybackButtonContent({ ...detail, playlist: { ...detail.playlist, completed: true } }), /Play radio/);
  } finally { clearMusicPlayback(); }
});

test('radio detail uses ordered song rows with small covers, occurrence actions, and disabled missing files', () => {
  const original = { route: state.route, selectedPlaylist: state.selectedPlaylist };
  const unavailable = { ...mock.getMockItem(104), playable: false, missing_since: 1 };
  const detail = { playlist: { id: 77, name: 'Summer <radio>', temporary: true }, items: [song, unavailable, song] };
  try {
    state.route = { page: 'browse-detail', kind: 'playlist', key: '77' };
    state.selectedPlaylist = detail;
    setMusicPlayback({ ...playback, queueId: 77, queueIndex: 2, queueItems: detail.items });
    const html = renderPlaylistDetailPage();
    assert.match(html, /Pause radio/);
    assert.match(html, /<p class="eyebrow">Radio<\/p>/);
    assert.match(html, /Summer &lt;radio&gt;/);
    assert.doesNotMatch(html, /class="item-grid"/);
    assert.equal((html.match(/class="music-track-cover"/g) ?? []).length, 3);
    assert.equal((html.match(/aria-current="true"/g) ?? []).length, 1);
    assert.match(html, /data-music-playlist-index="2"[^>]*aria-current="true"/);
    assert.match(html, /data-play-playlist-track="77" data-playlist-index="1"[^>]*disabled/);
    assert.match(html, /id="song-actions-playlist-77-0"/);
    assert.match(html, /id="song-actions-playlist-77-2"/);
    assert.match(html, /items\/302\/artwork/);
  } finally { Object.assign(state, original); clearMusicPlayback(); }
});

test('live playlist details follow queue edits and highlight exactly one repeated occurrence', () => {
  const second = mock.getMockItem(104);
  const detail = { playlist: { id: 77, name: 'Radio', current_index: 0 }, items: [song, second] };
  const order = [second, song, song];
  try {
    setMusicPlayback({ ...playback, queueId: 77, queueIndex: 2, queueItems: order });
    const live = musicPlaybackPlaylist(detail);
    assert.deepEqual(live.items.map(item => item.id), [104, 103, 103]);
    assert.equal(live.playlist.current_index, 2);
    assert.equal(live.playlist.position_ms, playback.positionMs);
    assert.equal(detail.items.length, 2);
    const html = renderPlaylistTracks(detail);
    assert.equal((html.match(/aria-current="true"/g) ?? []).length, 1);
    assert.ok(html.indexOf('data-music-track-id="104"') < html.indexOf('data-music-track-id="103"'));
    assert.match(html, /data-music-playlist-index="2"[^>]*aria-current="true"/);
    assert.equal(musicPlaybackPlaylist({ ...detail, playlist: { ...detail.playlist, id: 78 } }).items, detail.items);
  } finally { clearMusicPlayback(); }
});

test('radio covers follow the seed artist or album rather than the cursor or shuffled first song', () => {
  const other = { ...song, id: 999, parent_id: 900, artwork_item_id: 900, display_title: 'Another song' };
  for (const item_id of [301, 302]) {
    const radio = { playlist: { id: 77, name: 'Radio', current_index: 0 }, items: [song, other], artwork: { item_id, library_id: 3, updated_at: 100 } };
    const image = html => /background-image:url\('([^']+)'\)/.exec(html)?.[1];
    const initial = image(renderHomeShelfEntry({ radio }));
    assert.equal(initial, getArtworkUrl(item_id, 'poster', 100).replaceAll('&', '&amp;'));
    radio.playlist.current_index = 1;
    assert.equal(image(renderHomeShelfEntry({ radio })), initial);
    radio.items.reverse();
    radio.playlist.current_index = 0;
    assert.equal(image(renderHomeShelfEntry({ radio })), initial);
    radio.artwork.updated_at = 200;
    assert.notEqual(image(renderHomeShelfEntry({ radio })), initial);
    radio.artwork = null;
    assert.equal(image(renderHomeShelfEntry({ radio })), undefined);
  }
});

test('starting radio passes the selected artist album or song as its seed, while ordinary playlists omit it', async () => {
  const originalFetch = globalThis.fetch, requests = [];
  globalThis.fetch = async (_url, options) => { requests.push(JSON.parse(options.body)); return new Response('{}', { headers: { 'content-type': 'application/json' } }); };
  try {
    for (const seed of [301, 302, 103]) await savePlaylist('Radio', [103,104], true, seed);
    await savePlaylist('Playlist', [103,104]);
    assert.deepEqual(requests.slice(0,3).map(request => request.seed_item_id), [301,302,103]);
    assert.ok(requests.slice(0,3).every(request => request.temporary));
    assert.equal('seed_item_id' in requests[3], false);
    assert.equal(requests[3].temporary, false);
  } finally { globalThis.fetch = originalFetch; }
});

test('active cards match queue identity separately from the current song and album', () => {
  try {
    setMusicPlayback({ ...playback, queueId: 77 });
    assert.equal(musicCardStatus({ queueId: 77 }), 'playing');
    assert.equal(musicCardStatus({ queueId: 78 }), undefined);
    assert.equal(musicCardStatus({ itemId: song.id }), 'playing');
    assert.equal(musicCardStatus({ itemId: album.id, albumId: album.id }), undefined);
    assert.equal(musicCardStatus({ itemId: 77 }), undefined);
    setMusicPlayback({ ...playback, paused: true });
    assert.equal(musicCardStatus({ itemId: album.id, albumId: album.id }), 'paused');
    assert.equal(musicCardStatus({ queueId: song.id }), undefined);
    setMusicPlayback({ ...playback, ended: true });
    assert.equal(musicCardStatus({ itemId: song.id }), undefined);
    clearMusicPlayback();
    assert.equal(musicCardStatus({ itemId: song.id }), undefined);
  } finally { clearMusicPlayback(); }
});

test('active radio and album cards expose a visible state and current semantics without changing their navigation identity', () => {
  const radio = { playlist: { id: 77, name: 'Summer <radio>', current_index: 0 }, items: [song] };
  try {
    for (const paused of [false, true]) {
      setMusicPlayback({ ...playback, queueId: 77, paused });
      const html = renderHomeShelfEntry({ radio });
      assert.match(html, new RegExp(`class="media-card radio-card is-current is-${paused ? 'paused' : 'playing'}"`));
      assert.match(html, /data-resume-radio="77"[^>]*aria-current="true"/);
      assert.match(html, /aria-label="Open Summer &lt;radio&gt;"/);
      assert.match(html, new RegExp(`<span class="sr-only">${paused ? 'Paused' : 'Playing'}</span>`));
      assert.doesNotMatch(renderHomeShelfEntry({ radio: { ...radio, playlist: { ...radio.playlist, id: 78 } } }), /aria-current|is-current/);
    }
    setMusicPlayback(playback);
    assert.match(renderItemCard(album), /is-current is-playing/);
    assert.match(renderItemCard(album), /data-item-id="302"[^>]*aria-current="true"/);
    clearMusicPlayback();
    assert.doesNotMatch(renderItemCard(album), /aria-current|is-current/);
  } finally { clearMusicPlayback(); }
});

test('reopening the active radio navigates without fetching, resetting position, or rebuilding a shuffled queue', async () => {
  const original = state.bootstrap, originalFetch = globalThis.fetch;
  const paths = [], requests = [];
  try {
    state.bootstrap = { current_user: { id: 1 } };
    configureAudioPlayer({ navigate: path => paths.push(path), changed: () => {}, saved: async () => {} });
    globalThis.fetch = async (url) => { requests.push(url); return new Response('{}', { status: 404 }); };
    audioQueue.replace([song, mock.getMockItem(104), song]);
    audioQueue.cursor = 1;
    audioQueue.shuffle = true;
    audioQueue.repeat = 'all';
    const order = [...audioQueue.order], entries = audioQueue.entries;
    for (const paused of [false, true]) {
      const active = { ...playback, queueId: 77, paused };
      setMusicPlayback(active);
      await resumeMusicQueue(77, '/libraries/3/items/playlists/77');
      assert.equal(activeMusicPlayback(), active);
      assert.equal(audioQueue.entries, entries);
      assert.deepEqual(audioQueue.order, order);
      assert.equal(audioQueue.cursor, 1);
      assert.equal(audioQueue.shuffle, true);
      assert.equal(audioQueue.repeat, 'all');
    }
    assert.deepEqual(paths, ['/libraries/3/items/playlists/77', '/libraries/3/items/playlists/77']);
    assert.deepEqual(requests, []);
    // Another radio, a completed queue, and another account must still use the normal resume path.
    await assert.rejects(resumeMusicQueue(78), error => error.status === 404);
    setMusicPlayback({ ...playback, queueId: 77, ended: true });
    await assert.rejects(resumeMusicQueue(77), error => error.status === 404);
    setMusicPlayback({ ...playback, userId: 2, queueId: 77 });
    await assert.rejects(resumeMusicQueue(77), error => error.status === 404);
    assert.equal(requests.length, 3);
    assert.equal(paths.length, 2);
  } finally {
    globalThis.fetch = originalFetch;
    state.bootstrap = original;
    clearMusicPlayback();
    audioQueue.replace([]);
    audioQueue.shuffle = false;
    audioQueue.repeat = 'off';
    configureAudioPlayer({ navigate: () => {}, changed: () => {}, saved: async () => {} });
  }
});
