import assert from 'node:assert/strict';
import { after, beforeEach, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/', origin: 'http://localhost' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const { heroPresentation, itemLineage } = await server.ssrLoadModule('/src/app/hierarchy.ts');
const { itemArtwork, renderTvTitle } = await server.ssrLoadModule('/src/app/tvItemView.ts');
const { renderHomeFeature, renderItemCard } = await server.ssrLoadModule('/src/app/homeView.ts');
const { renderPersonPage, renderPersonCredit, renderItemPage, personBackdropUrl } = await server.ssrLoadModule('/src/app/itemPersonView.ts');
const mock = await server.ssrLoadModule('/src/mockApi.ts');
let artist, album, track;

beforeEach(() => {
  artist = { ...mock.getMockItem(301), parent_id: null, logo_url: '/artist-logo', backdrop_url: '/artist-fanart', artwork_updated_at: 100, hierarchy: [] };
  album = { ...mock.getMockItem(302), parent_id: artist.id, poster_url: '/album-cover', logo_url: undefined, backdrop_url: undefined, artwork_updated_at: 200, hierarchy: [artist] };
  track = { ...mock.getMockItem(103), parent_id: album.id, logo_url: undefined, backdrop_url: undefined, hierarchy: [artist, album] };
  Object.assign(state, { selectedItemMetadata: undefined, selectedItem: album, selectedPerson: undefined,
    route: { page: 'home', libraryId: 3 }, homeTab: 'recommended', homePreviewItemId: album.id,
    home: { shelves: [{ items: [album] }] }, libraryItems: [artist, album, track], searchQuery: '',
    metadataProviders: [], libraries: mock.getMockLibraries(), bootstrap: { current_user: { id: 1, admin: true } } });
});

test('music heroes inherit the artist identity and fanart while keeping the album title and cover', () => {
  const html = renderHomeFeature();
  assert.match(html, /items\/301\/artwork\?kind=backdrop&amp;rev=100/);
  assert.match(html, /home-feature-logo[^>]*items\/301\/artwork\?kind=logo&amp;rev=100/);
  assert.match(html, /home-feature-context">Mock Album/);
  assert.match(html, /data-item-id="302"/);
  assert.match(itemArtwork(album).poster, /items\/302\/artwork\?kind=poster/);
  assert.match(itemArtwork(track).backdrop, /items\/301\/artwork\?kind=backdrop/);
  assert.match(renderTvTitle(track, itemArtwork(track).logo), /Mock Artist/);
  artist.logo_url = undefined;
  assert.match(renderHomeFeature(), /<h2>Mock Artist<\/h2>/);
});

test('a nearer child logo or backdrop takes precedence and arbitrary media hierarchies work', () => {
  album.logo_url = '/album-logo';
  album.backdrop_url = '/album-fanart';
  assert.equal(heroPresentation(track).identity.id, album.id);
  assert.equal(heroPresentation(track).backdrop.id, album.id);
  track.logo_url = '/track-logo';
  track.backdrop_url = '/track-fanart';
  assert.equal(heroPresentation(track).identity.id, track.id);
  assert.equal(heroPresentation(track).backdrop.id, track.id);
  for (const types of [['show', 'season', 'episode'], ['collection', 'volume', 'chapter']]) {
    const root = { ...artist, item_type: types[0] };
    const parent = { ...album, item_type: types[1], logo_url: undefined, backdrop_url: undefined };
    const child = { ...track, item_type: types[2], logo_url: undefined, backdrop_url: undefined, hierarchy: [root, parent] };
    assert.equal(heroPresentation(child).identity.id, root.id);
    assert.equal(heroPresentation(child).backdrop.id, root.id);
  }
});

test('hierarchy traversal rejects cycles and unrelated libraries', () => {
  artist.parent_id = track.id;
  assert.deepEqual(itemLineage(track).map((item) => item.id), [track.id, album.id, artist.id]);
  artist.library_id = 99;
  assert.deepEqual(itemLineage(track).map((item) => item.id), [track.id, album.id]);
});

test('person profiles show albums directly, reuse identity-linked artwork, and omit empty screen credits', () => {
  state.selectedPerson = { ...mock.getMockPerson(1), person: { ...mock.getMockPerson(1).person, cached_image_path: undefined, image_url: undefined }, artists: [artist], albums: [album], credits: [] };
  const html = renderPersonPage();
  assert.match(html, /items\/301\/artwork\?kind=poster&amp;rev=100/);
  assert.match(html, /data-item-id="302"/);
  assert.doesNotMatch(html, /data-item-id="301"|0 items|data-nav-section="credits"/);
  assert.match(personBackdropUrl(), /items\/301\/artwork\?kind=backdrop/);
  state.selectedPerson.person.image_url = '/own-portrait';
  assert.match(renderPersonPage(), /src="http:\/\/localhost\/own-portrait"/);
});

test('members retain their own identities and use full width group cards with compact empty portraits', () => {
  state.selectedPerson = { ...mock.getMockPerson(1), person: { ...mock.getMockPerson(1).person, cached_image_path: undefined, image_url: undefined }, artists: [], albums: [], credits: [], groups: [{ item: artist, credit: { role: 'Member · bass' } }] };
  const html = renderPersonPage();
  assert.match(html, /person-hero-without-photo/);
  assert.match(html, /class="music-membership-card"/);
  assert.match(html, /Member · bass/);
  assert.doesNotMatch(html, /<img|data-nav-section="credits"|0 items/);
  state.selectedItem = artist;
  assert.match(renderPersonCredit({ person_id: 1, name: 'Solo artist', department: 'Artists' }), /items\/301\/artwork/);
  assert.doesNotMatch(renderPersonCredit({ person_id: 2, name: 'Group member', department: 'Members' }), /items\/301\/artwork/);
});

test('album resume action preserves its track target and position without an unmatched track warning', () => {
  album.has_metadata = true;
  album.playback_target = { item_id: track.id, start_ms: 5000, label: 'Continue album', display_title: track.display_title, resume: true };
  state.selectedItem = album;
  state.route = { page: 'item', itemId: album.id };
  const html = renderItemPage();
  assert.match(html, /data-playback-target-item-id="103" data-playback-target-start-ms="5000"/);
  assert.match(html, /Continue album/);
  assert.match(html, /Restart album/);
  assert.match(html, /Restart track/);
  assert.doesNotMatch(renderItemCard(album), /Metadata is not linked yet/);
});
