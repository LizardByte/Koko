import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/', origin: 'http://localhost' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const { renderSettingsPage, buildSettingsFromForm } = await server.ssrLoadModule('/src/app/settingsView.ts');
const { renderShelfStack } = await server.ssrLoadModule('/src/app/homeView.ts');
const { homePreviewCandidates, topLevelLibraryItems, categorySummaries } = await server.ssrLoadModule('/src/app/selectors.ts');
const mock = await server.ssrLoadModule('/src/mockApi.ts');

test('all library types offer Exclude from Home for creation and preserve its edited value', () => {
  const original = { ...state };
  try {
    const settings = mock.getMockSettings();
    settings.settings.media.libraries = ['movies', 'shows', 'music', 'books', 'photos', 'home_videos', 'mixed', 'future'].map((kind, index) => ({ ...settings.settings.media.libraries[0], kind, name: kind, exclude_from_home: index % 2 === 1 }));
    Object.assign(state, { route: { page: 'settings', section: 'libraries' }, settingsResponse: settings, settingsCreation: 'libraries', libraries: [], metadataProviders: mock.getMockMetadataProviders(), bootstrap: { current_user: { id: 1, admin: true } } });
    const html = renderSettingsPage();
    assert.match(html, /name="library_exclude_from_home" type="checkbox"/);
    for (const [index, library] of settings.settings.media.libraries.entries()) {
      assert.match(html, new RegExp(`name="existing_library_exclude_from_home_${index}" type="checkbox" ${library.exclude_from_home ? 'checked' : ''}`));
    }
    const form = new FormData();
    for (const [index, library] of settings.settings.media.libraries.entries()) {
      form.set(`existing_library_paths_${index}`, library.path);
      if (index % 2 === 0) form.set(`existing_library_exclude_from_home_${index}`, 'on');
    }
    assert.deepEqual(buildSettingsFromForm(form).media.libraries.map(library => library.exclude_from_home), [true, false, true, false, true, false, true, false]);
    // A settings section without library controls must preserve their preferences.
    state.route = { page: 'settings', section: 'providers' };
    assert.deepEqual(buildSettingsFromForm(new FormData()).media.libraries, settings.settings.media.libraries);
  } finally { Object.assign(state, original); }
});

test('combined Home says Continue playing even with only music or radio while libraries keep their own label', () => {
  const original = { ...state };
  const item = mock.getMockItem(302);
  const radio = { playlist: { id: 1, name: 'Radio', current_index: 0 }, items: [mock.getMockItem(103)] };
  try {
    for (const items of [[], [item]]) {
      state.home = { library_id: null, shelves: [{ id: 'continue_watching', title: 'Continue listening', items }], listening_queues: [radio] };
      assert.match(renderShelfStack(), /<h3>Continue playing<\/h3>/);
    }
    for (const title of ['Continue listening', 'Continue reading', 'Continue watching']) {
      state.home = { library_id: 3, shelves: [{ id: 'continue_watching', title, items: [item] }] };
      assert.ok(renderShelfStack().includes(`<h3>${title}</h3>`));
    }
  } finally { Object.assign(state, original); }
});

test('excluded libraries remain browsable but cannot return through empty Home preview fallbacks', () => {
  const original = { ...state }, settings = mock.getMockSettings();
  try {
    const next = structuredClone(settings.settings);
    next.media.libraries.forEach(library => { library.exclude_from_home = true; });
    mock.updateMockSettings(next);
    const home = mock.getMockHome();
    assert.ok(home.shelves.every(shelf => shelf.items.length === 0));
    assert.equal(home.collections.length, 0);
    assert.ok(mock.getMockItems().length > 0);
    assert.equal(mock.getMockLibraries().length, next.media.libraries.length);
    Object.assign(state, { route: { page: 'home' }, homeTab: 'recommended', searchQuery: '', home, libraries: mock.getMockLibraries(), libraryItems: mock.getMockItems() });
    assert.deepEqual(homePreviewCandidates(), []);
    assert.deepEqual(topLevelLibraryItems(), []);
    assert.deepEqual(categorySummaries(), []);
    const library = state.libraries[0];
    assert.ok(mock.getMockHome(library.id).shelves.some(shelf => shelf.items.length));
    state.route = { page: 'home', libraryId: library.id };
    state.libraryItems = mock.getMockItems(library.id);
    assert.ok(topLevelLibraryItems().length > 0);
  } finally { mock.updateMockSettings(settings.settings); Object.assign(state, original); }
});
