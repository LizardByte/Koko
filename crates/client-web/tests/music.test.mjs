import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/', origin: 'http://localhost' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { orderedAlbumTracks, albumPlaybackQueue, renderAlbumTracks } = await server.ssrLoadModule('/src/app/music.ts');
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const { renderItemPage } = await server.ssrLoadModule('/src/app/itemPersonView.ts');
const { canManuallyLinkMetadata } = await server.ssrLoadModule('/src/app/selectors.ts');
const mock = await server.ssrLoadModule('/src/mockApi.ts');

function track(id, disc, number, extra = {}) {
  return { ...mock.getMockItem(203), id, item_type: 'track', media_kind: 'audio', display_title: `Track ${id}`, playable: true, missing_since: null, music: { artist: 'A <guest>', album_artist: 'Artist', album: 'Album', disc_number: disc, track_number: number, genres: [] }, ...extra };
}

test('album queue orders discs and tracks numerically, skips missing tracks, and starts at a selected track', () => {
  const children = [track(1, 2, 1), track(2, 1, 10), track(3, 1, 2), track(4, 1, 1, { missing_since: 1 }), track(5, 1, null)];
  const originalIds = children.map((item) => item.id);
  const album = { children };
  assert.deepEqual(orderedAlbumTracks(children).map((item) => item.id), [4, 3, 2, 5, 1]);
  assert.deepEqual(children.map((item) => item.id), originalIds);
  assert.deepEqual(albumPlaybackQueue(album), [3, 2, 5, 1]);
  assert.deepEqual(albumPlaybackQueue(album, 2), [2, 5, 1]);
  assert.deepEqual(albumPlaybackQueue(album, 4), []);
  const html = renderAlbumTracks(album);
  assert.match(html, /A &lt;guest&gt;/);
  assert.match(html, /data-play-music-track-id="4"[^>]*disabled/);
  assert.ok(html.indexOf('data-item-id="3"') < html.indexOf('data-item-id="2"'));
});

test('music detail pages expose album playback, track rows, artist albums, and metadata management', () => {
  const album = { ...mock.getMockItem(202), item_type: 'album', media_kind: 'audio', music: { artist: 'Artist', album_artist: 'Artist', album: 'Album', genres: [] }, hierarchy: [], children: [track(10, 1, 1)], playback_target: undefined, restart_playback_target: undefined };
  state.bootstrap = { current_user: { id: 1, admin: true } };
  state.route = { page: 'item', itemId: album.id };
  state.selectedItem = album;
  state.selectedItemMetadata = undefined;
  state.metadataProviders = mock.getMockMetadataProviders();
  state.libraryItems = [album, ...album.children];
  assert.match(renderItemPage(), /data-play-music-album-id/);
  assert.match(renderItemPage(), /music-track-list/);
  assert.match(renderItemPage(), /music-detail-page/);
  for (const item_type of ['artist', 'album', 'track']) assert.equal(canManuallyLinkMetadata({ item_type }), true);
  state.selectedItem = { ...album, item_type: 'artist', children: [album] };
  assert.match(renderItemPage(), /<h3>Albums<\/h3>/);
});


test('music tag reading is opt-in for each library and is saved independently', async () => {
  const { renderSettingsPage, buildSettingsFromForm } = await server.ssrLoadModule('/src/app/settingsView.ts');
  state.settingsResponse = mock.getMockSettings();
  state.route = { page: 'settings', section: 'libraries' };
  state.libraries = mock.getMockLibraries();
  const libraries = state.settingsResponse.settings.media.libraries;
  const index = libraries.findIndex((library) => library.kind === 'music');
  const field = `existing_library_use_embedded_music_tags_${index}`;
  assert.match(renderSettingsPage(), new RegExp(`name="${field}" type="checkbox"\\s*/>`));
  const form = new FormData();
  form.set(`existing_library_name_${index}`, libraries[index].name);
  form.set(`existing_library_paths_${index}`, libraries[index].path);
  form.set(field, 'on');
  assert.equal(buildSettingsFromForm(form).media.libraries[index].use_embedded_music_tags, true);
  form.delete(field);
  assert.equal(buildSettingsFromForm(form).media.libraries[index].use_embedded_music_tags, false);
});

test('resume shelves use listening for music, reading for books, and watching for video', () => {
  assert.equal(mock.getMockHome(3).shelves[0].title, 'Continue listening');
  assert.equal(mock.getMockHome(1).shelves[0].title, 'Continue watching');
  assert.equal(mock.getMockHome(2).shelves[0].title, 'Continue watching');
  const originalCount = mock.getMockSettings().settings.media.libraries.length;
  mock.addMockLibrary({ library: { ...mock.getMockSettings().settings.media.libraries[0], name: 'Books', kind: 'books' } });
  try {
    const books = mock.getMockLibraries().find((library) => library.kind === 'books');
    assert.equal(mock.getMockHome(books.id).shelves[0].title, 'Continue reading');
  } finally {
    mock.removeMockLibrary(originalCount);
  }
});
