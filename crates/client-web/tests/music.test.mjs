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
const { renderItemPage, selectedItemDefaultMetadataArtist, selectedItemDefaultMetadataYear } = await server.ssrLoadModule('/src/app/itemPersonView.ts');
const { searchItemMetadata, getMusicLyrics } = await server.ssrLoadModule('/src/api.ts');
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


test('album matching defaults the editable artist field from the parent and preserves edited or empty values', () => {
  const original = { ...state };
  const parent = { ...mock.getMockItem(301), display_title: 'Parent <group>' };
  const album = { ...mock.getMockItem(302), hierarchy: [parent], music: { album_artist: 'Old folder name' } };
  try {
    Object.assign(state, {
      bootstrap: { current_user: { id: 1, admin: true } }, route: { page: 'item', itemId: album.id },
      selectedItem: album, selectedItemMetadata: undefined, metadataSearchArtist: undefined,
      libraries: mock.getMockLibraries(), metadataProviders: mock.getMockMetadataProviders(),
      libraryItems: [], itemManagementOpen: true,
    });
    assert.equal(selectedItemDefaultMetadataArtist(), 'Parent <group>');
    assert.match(renderItemPage(), /id="metadata-search-artist"[^>]*value="Parent &lt;group&gt;"/);
    state.metadataSearchArtist = 'Corrected & group';
    assert.match(renderItemPage(), /id="metadata-search-artist"[^>]*value="Corrected &amp; group"/);
    state.metadataSearchArtist = '';
    assert.match(renderItemPage(), /id="metadata-search-artist"[^>]*value=""/);
    state.selectedItem = { ...album, hierarchy: [] };
    assert.equal(selectedItemDefaultMetadataArtist(), 'Old folder name');
    state.selectedItem = mock.getMockItem(101);
    assert.doesNotMatch(renderItemPage(), /id="metadata-search-artist"/);
  } finally { Object.assign(state, original); }
});

test('artist matching omits year even when the item or a previous search has one', () => {
  const original = { ...state };
  try {
    Object.assign(state, {
      bootstrap: { current_user: { id: 1, admin: true } }, route: { page: 'item', itemId: 301 },
      selectedItem: { ...mock.getMockItem(301), release_year: 1999 },
      selectedItemMetadata: { providers: mock.getMockMetadataProviders(), matches: [{ provider_id: 'musicbrainz', external_id: 'artist', title: 'Artist', locale_key: 'en-US', refresh_state: 'linked', release_year: 2001 }] }, metadataSearchYear: '2008',
      libraries: mock.getMockLibraries(), metadataProviders: mock.getMockMetadataProviders(),
      libraryItems: [], itemManagementOpen: true,
    });
    assert.equal(selectedItemDefaultMetadataYear(), '');
    assert.doesNotMatch(renderItemPage(), /id="metadata-search-year"/);
    for (const item_type of ['album', 'track', 'movie', 'show']) {
      state.selectedItem = { ...state.selectedItem, item_type };
      assert.equal(selectedItemDefaultMetadataYear(), '1999');
      assert.match(renderItemPage(), /id="metadata-search-year"/);
    }
  } finally { Object.assign(state, original); }
});

test('album searches send the artist as its own encoded filter and allow clearing it', async () => {
  const originalFetch = globalThis.fetch;
  const requests = [];
  globalThis.fetch = async (url) => { requests.push(new URL(url, location.origin)); return new Response('[]', { headers: { 'content-type': 'application/json' } }); };
  try {
    await searchItemMetadata(302, { query: ' Live: Deluxe ', artist: ' AC/DC & Guests+ ', year: '2001', providers: ['musicbrainz'] });
    assert.equal(requests[0].searchParams.get('query'), 'Live: Deluxe');
    assert.equal(requests[0].searchParams.get('artist'), 'AC/DC & Guests+');
    assert.equal(requests[0].searchParams.get('year'), '2001');
    assert.equal(requests[0].searchParams.get('providers'), 'musicbrainz');
    await searchItemMetadata(302, { query: 'Album', artist: '' });
    assert.equal(requests[1].searchParams.get('artist'), '');
    await searchItemMetadata(101, 'Movie');
    assert.equal(requests[2].searchParams.has('artist'), false);
  } finally { globalThis.fetch = originalFetch; }
});

test('track matching explains that its album must be matched first', () => {
  const original = { ...state };
  try {
    Object.assign(state, {
      bootstrap: { current_user: { id: 1, admin: true } }, route: { page: 'item', itemId: 103 },
      selectedItem: { ...track(103, 1, 1), hierarchy: [{ ...mock.getMockItem(302), has_metadata: false }] },
      selectedItemMetadata: undefined, libraryItems: [], itemManagementOpen: true,
    });
    let form = renderItemPage().match(/<form id="metadata-search-form"[\s\S]*?<\/form>/)?.[0];
    assert.match(form, /Match this track’s album first/);
    assert.match(form, /<button type="submit" disabled>/);
    state.selectedItem.hierarchy[0].has_metadata = true;
    form = renderItemPage().match(/<form id="metadata-search-form"[\s\S]*?<\/form>/)?.[0];
    assert.doesNotMatch(form, /album first|disabled/);
  } finally { Object.assign(state, original); }
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

test('playback lyrics requests only read saved lyrics without a provider refresh parameter', async () => {
  const originalFetch = globalThis.fetch;
  const requests = [];
  globalThis.fetch = async (url) => { requests.push(new URL(url, location.origin)); return new Response('{}', { headers: { 'content-type': 'application/json' } }); };
  try {
    await getMusicLyrics(303);
    await getMusicLyrics(303);
    assert.equal(requests[0].pathname, '/api/v1/items/303/lyrics');
    assert.equal(requests[0].searchParams.has('refresh'), false);
    assert.equal(requests[1].searchParams.has('refresh'), false);
  } finally { globalThis.fetch = originalFetch; }
});

test('resume shelves use listening for music, reading for books, and watching for video', () => {
  assert.equal(mock.getMockHome().shelves[0].title, 'Continue playing');
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
