/** Regressions for document actions, page ordering, and authenticated reader requests. */
import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/', origin: 'https://koko.local' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const api = await server.ssrLoadModule('/src/api.ts');
const { renderBookActions } = await server.ssrLoadModule('/src/app/bookReader.ts');
const { compareReaderPaths, isComicPage, readingFraction, readerPageIndex, readerTextPosition, readerPageSpread, readerPageLabel, publisherPageCount } = await server.ssrLoadModule('/src/app/readerSupport.ts');
const { SectionProgress } = await server.ssrLoadModule('/node_modules/foliate-js/progress.js');
const { readerMarkup } = await server.ssrLoadModule('/src/app/readerControls.ts');
const { formatChildCount } = await server.ssrLoadModule('/src/app/ui.ts');
const { resetBookReadingProgress, updateBookReadingState } = await server.ssrLoadModule('/src/app/bookActions.ts');
const { state: appState } = await server.ssrLoadModule('/src/app/state.ts');
const { createReaderFullscreen } = await server.ssrLoadModule('/src/app/readerFullscreen.ts');
const selectors = await server.ssrLoadModule('/src/app/selectors.ts');
const { renderCollectionDetailPage } = await server.ssrLoadModule('/src/app/homeView.ts');
const mock = await server.ssrLoadModule('/src/mockApi.ts');
const book = (id, path, extra = {}) => ({ id, item_type: 'book', display_title: 'Book <one>', relative_path: path, children: [], ...extra });

test('curated collection pages and related rails show Books below Authors without duplicating formats', () => {
  const original = { home: appState.home, route: appState.route, libraryItems: appState.libraryItems, selectedItem: appState.selectedItem, browseFilter: appState.browseFilter };
  const item = (id, item_type, parent_id) => ({ ...mock.getMockItem(302), id, item_type, parent_id, library_id: 1, hierarchy: [], children: [] });
  const author = item(10, 'book_author', null), first = item(11, 'book_work', 10), second = item(12, 'book_work', 10), epub = item(13, 'book', 11), audio = item(14, 'audiobook', 11);
  const collection = { id: 'openlibrary:/collections/favorites', provider_id: 'openlibrary', external_id: '/collections/favorites', name: 'Favorites', item_ids: [11,12], item_count: 2 };
  try {
    appState.libraryItems = [author, first, second, epub, audio]; appState.home = { collections: [collection] };
    appState.route = { page: 'browse-detail', kind: 'collection', key: collection.id, libraryId: 1 };
    appState.selectedItem = { ...epub, hierarchy: [author, first] };
    assert.deepEqual(selectors.itemsForCollection(collection).map((item) => item.id), [11,12]);
    assert.equal(selectors.selectedItemRoot().id, 11);
    assert.deepEqual(selectors.selectedItemCollectionRails()[0].items.map((item) => item.id), [12]);
    const html = renderCollectionDetailPage();
    assert.match(html, /2 titles/); assert.match(html, /data-item-id="11"/); assert.match(html, /data-item-id="12"/);
    assert.doesNotMatch(html, /data-item-id="(?:10|13|14)"/);
    appState.browseFilter = { kind: 'collection', label: 'Favorites', itemIds: [11,12] };
    assert.deepEqual(selectors.filteredTopLevelLibraryItems().map((item) => item.id), [11,12]);
    appState.selectedItem = { ...audio, hierarchy: [author, first] }; assert.equal(selectors.selectedItemRoot().id, 11);
  } finally { Object.assign(appState, original); }
});

test('fullscreen uses the inner surface and owns entry, exit, rejection, and pending-close cleanup', async () => {
  let entered = 0, exited = 0, finish;
  const owner = { fullscreenElement: null, async exitFullscreen() { exited++; this.fullscreenElement = null; } };
  const surface = { async requestFullscreen() { entered++; owner.fullscreenElement = this; } };
  const dialog = { ownerDocument: owner, querySelector: (selector) => { assert.equal(selector, '[data-reader-surface]'); return surface; }, requestFullscreen() { throw new Error('Dialogs cannot be fullscreen'); } };
  let controller = createReaderFullscreen(dialog);
  await controller.toggle(); assert.equal(entered, 1); assert.equal(controller.active(), true);
  await controller.toggle(); assert.equal(exited, 1); assert.equal(controller.active(), false);
  owner.fullscreenElement = {}; controller.close(); assert.equal(exited, 1);
  owner.fullscreenElement = null; controller = createReaderFullscreen(dialog);
  surface.requestFullscreen = () => new Promise((resolve) => { finish = () => { owner.fullscreenElement = surface; resolve(); }; });
  const pending = controller.toggle(); controller.close(); finish(); await pending;
  assert.equal(owner.fullscreenElement, null); assert.equal(exited, 2);
  controller = createReaderFullscreen(dialog); surface.requestFullscreen = async () => { throw new Error('Permission denied'); };
  await assert.rejects(controller.toggle(), /Permission denied/);
});

test('book work offers independent format selection and listening without missing editions', () => {
  const html = renderBookActions(book(1, '', { item_type: 'book_work', children: [book(2, 'book.epub'), book(3, 'book.pdf'), book(4, 'missing.mobi', { missing_since: 1 }), book(5, '', { item_type: 'audiobook' })] }));
  assert.match(html, /data-read-book="2"/); assert.match(html, /data-listen-book="5"/);
  assert.match(html, /EPUB/); assert.match(html, /PDF/); assert.doesNotMatch(html, /MOBI/);
  assert.match(html, /Book &lt;one&gt;/); assert.doesNotMatch(html, /Book <one>/);
  assert.equal(renderBookActions(book(1, '', { item_type: 'book_author' })), '');
  assert.match(renderBookActions(book(8, 'Issue.cbz', { item_type: 'comic_issue' })), /data-read-book="8"/);
  assert.match(renderBookActions(book(9, '', { item_type: 'comic_series', children: [book(10, '1.cbz', { item_type: 'comic_issue' }), book(11, '2.cbz', { item_type: 'comic_issue' })] })), />Issue<select/);
});

test('reading actions distinguish unopened, resumable, completed, and reset accounts', () => {
  assert.match(renderBookActions(book(1, 'book.epub')), />Read</);
  const progress = { fraction: 0.35, completed: false, started: true, page_count: 123 };
  const html = renderBookActions(book(1, 'book.epub', { reading_progress: progress }));
  assert.match(html, />Resume</); assert.match(html, /35% read/);
  assert.match(html, /data-reset-reading/); assert.match(html, /Hold for reading options/);
  assert.match(html, /class="secondary-button" data-reset-reading/);
  assert.doesNotMatch(html, />Continue</);
  assert.match(renderBookActions(book(1, 'book.epub', { reading_progress: { ...progress, completed: true } })), />Read again</);
  assert.match(renderBookActions(book(1, 'book.epub', { reading_progress: { fraction: 0, started: false, page_count: 100 } })), />Read</);
  const work = renderBookActions(book(1, '', { item_type: 'book_work', children: [book(2, 'first.epub'), book(3, 'second.pdf', { reading_progress: progress })] }));
  assert.match(work, /data-read-book="3"/); assert.match(work, /value="3" selected/);
  assert.match(work, /selected>PDF ·/);
});

test('Book listening resumes the most recently started unfinished available narration independently of reading', () => {
  const narration = (id, extra = {}) => book(id, '', { item_type: 'audiobook', ...extra });
  const work = book(1, '', { item_type: 'book_work', children: [book(2, 'book.epub'), narration(3), narration(4, { playback_position_ms: 35000, last_played_at: 100 }), narration(5, { playback_position_ms: 12000, last_played_at: 200 }), narration(6, { playback_position_ms: 60000, playback_completed: true, last_played_at: 300 }), narration(7, { playback_position_ms: 30000, last_played_at: 400, missing_since: 1 })] });
  let html = renderBookActions(work);
  assert.match(html, />Read</); assert.match(html, />Resume listening</); assert.match(html, /data-listen-book="5"/);
  work.children = [book(2, 'book.epub', { reading_progress: { fraction: 0.35, started: true } }), narration(3)];
  html = renderBookActions(work);
  assert.match(html, />Resume</); assert.match(html, />Listen</); assert.doesNotMatch(html, />Resume listening</);
  assert.match(html, /data-listen-book="3"/);
});

test('ebook badges show saved page counts including generated EPUB layouts', () => {
  assert.equal(formatChildCount(book(1, 'book.epub')), '');
  assert.equal(formatChildCount(book(1, 'book.FB2.zip')), '');
  assert.equal(formatChildCount(book(1, 'book')), '');
  assert.equal(formatChildCount(book(1, 'issue.cbz', { item_type: 'comic_issue', reading_progress: { page_count: 1 } })), '1 page');
  assert.equal(formatChildCount(book(1, 'book.pdf', { reading_progress: { page_count: 123 } })), '123 pages');
  assert.equal(formatChildCount(book(1, 'book.epub', { reading_progress: { page_count: 80 } })), '80 pages');
  assert.equal(formatChildCount(book(1, 'book.epub', { reading_progress: { page_count: 0 } })), '');
  assert.equal(publisherPageCount(), undefined);
  assert.equal(publisherPageCount(null), undefined);
  assert.equal(publisherPageCount([{ href: 'book#1' }, { subitems: [{ href: 'book#2' }, { href: 'book#1' }] }]), 2);
});

test('completed layout counts reveal ebook badges and survive account progress refreshes', () => {
  const original = { document: globalThis.document, selectedItem: appState.selectedItem };
  const badge = { hidden: true, textContent: '' };
  try {
    appState.selectedItem = book(17, 'book.epub');
    globalThis.document = { querySelector: () => null, querySelectorAll: (selector) => selector.includes('.media-card-duration') ? [badge] : [] };
    const progress = { revision: 'hash', locator: 'epubcfi(/6/2)', fraction: .25, completed: false, bookmarks: [], page_count: 80 };
    updateBookReadingState(17, progress);
    assert.equal(badge.textContent, '80 pages'); assert.equal(badge.hidden, false);
    assert.equal(appState.selectedItem.reading_progress.page_count, 80);
    assert.equal(appState.selectedItem.reading_progress.fraction, .25);
    assert.equal(appState.selectedItem.reading_progress.started, true);
  } finally { globalThis.document = original.document; appState.selectedItem = original.selectedItem; }
});

test('physical spreads put the cover on the right and preserve even left and odd right pages', () => {
  assert.deepEqual(readerPageSpread(0, 6, true), [null, 0]);
  assert.deepEqual(readerPageSpread(1, 6, true), [1, 2]);
  assert.deepEqual(readerPageSpread(2, 6, true), [1, 2]);
  assert.deepEqual(readerPageSpread(3, 6, true), [3, 4]);
  assert.deepEqual(readerPageSpread(5, 6, true), [5, null]);
  assert.deepEqual(readerPageSpread(0, 1, true), [null, 0]);
  assert.deepEqual(readerPageSpread(2, 6, false), [2]);
  assert.equal(readerPageLabel([null, 0], 6), 'Page 1 of 6');
  assert.equal(readerPageLabel([1, 2], 6), 'Pages 2–3 of 6');
  assert.equal(readerPageLabel([5, null], 6), 'Page 6 of 6');
});

test('page slider destinations round-trip without advancing from the end of the displayed page', () => {
  for (const count of [1, 2, 7, 121]) {
    for (let index = 0; index < count; index++) {
      const position = count > 1 ? index / (count - 1) : 0;
      assert.equal(readerPageIndex(Math.round(position * 1000) / 1000, count), index);
    }
    assert.equal(readerPageIndex(0, count), 0); assert.equal(readerPageIndex(1, count), count - 1);
  }
  assert.equal(readerPageIndex(0.5, 121), 60);
  assert.equal(readerPageLabel(readerPageSpread(readerPageIndex(0.5, 121), 121, true), 121), 'Pages 60–61 of 121');
});

test('text slider positions round-trip through Foliate without slipping a page or crossing chapter boundaries', () => {
  const sections = [{ size: 30000 }, { size: 1000, linear: 'no' }, { size: 70000 }];
  const progress = new SectionProgress(sections, 1500, 1600), sizes = progress.sizes;
  for (const index of [0, 2]) for (const pages of [1, 2, 10, 100]) for (let page = 0; page < pages; page++) {
    const position = readerTextPosition(sizes, index, page / pages, pages);
    const [section, anchor] = progress.getSection(Math.round(position * 1000) / 1000);
    assert.equal(section, index);
    assert.equal(Math.round(anchor * (pages - 1)), page);
  }
  assert.equal(readerTextPosition([0], 0, 0), 0);
  assert.equal(readerTextPosition(sizes, 2, .5), .65);
});

test('one bottom controller uses named icons and pressed toggles without a theme switcher', () => {
  const html = readerMarkup(), footer = html.slice(html.indexOf('<footer'));
  for (const control of ['close', 'previous', 'next', 'options', 'fullscreen', 'bookmark', 'size', 'direction', 'spread']) assert.match(footer, new RegExp(`data-reader-${control}`));
  for (const name of ['Contents', 'Bookmarks', 'Add bookmark', 'Search this book', 'Two pages']) assert.ok(html.includes(`aria-label="${name}"`));
  assert.match(html, /data-reader-spread[^>]*aria-pressed="false"/);
  assert.match(html, /aria-describedby="reader-seek-preview"/);
  assert.doesNotMatch(html, /<header|type="checkbox"|data-reader-theme|>Theme</);
});

test('comic image pages retain natural numeric order and ignore archive metadata', () => {
  const entries = ['page10.jpg', '__MACOSX/._page0.png', 'page2.jpg', 'page1.png', '.hidden/page0.gif', 'ComicInfo.xml'];
  assert.deepEqual(entries.filter(isComicPage).sort(compareReaderPaths), ['page1.png', 'page2.jpg', 'page10.jpg']);
  assert.ok(isComicPage('Chapter 1/page 01.AVIF')); assert.ok(isComicPage('page.svg'));
  for (const value of [NaN, Infinity, -1]) assert.equal(readingFraction(value), 0);
  assert.equal(readingFraction(1.5), 1); assert.equal(readingFraction(0.65), 0.65);
});

test('reader API sends credentials and propagates denials instead of returning mock books', async () => {
  const original = globalThis.fetch; const requests = [];
  try {
    globalThis.fetch = async (url, options) => { requests.push([url, options]); return new Response('{"message":"Library access denied"}', { status: 404, headers: { 'Content-Type': 'application/json' } }); };
    await assert.rejects(api.getReaderDocument(17), /Library access denied/);
    await assert.rejects(api.getReaderFile({ item_id: 17 }), /Library access denied/);
    assert.ok(requests.every(([, options]) => options.credentials === 'include'));
    globalThis.fetch = async () => new Response('book', { headers: { 'Content-Type': 'application/octet-stream' } });
    for (const format of ['fb2', 'fbz', 'epub', 'mobi']) {
      const file = await api.getReaderFile({ item_id: 17, format, source_format: format, file_name: `BOOK.${format.toUpperCase()}` });
      assert.equal(file.name, `book.${format}`); assert.equal(await file.text(), 'book');
    }
    const state = { revision: 'hash', locator: '2', fraction: 0.3, completed: false, bookmarks: [] };
    globalThis.fetch = async (url, options) => { requests.push([url, options]); return new Response(null, { status: 204 }); };
    await api.saveReadingProgress(17, state);
    const [url, options] = requests.at(-1); assert.match(url, /items\/17\/reader\/progress$/);
    assert.equal(options.method, 'PUT'); assert.equal(options.keepalive, true); assert.deepEqual(JSON.parse(options.body), state);
  } finally { globalThis.fetch = original; }
});

test('reset preserves bookmarks and page counts and cannot copy state across an account change', async () => {
  const original = { fetch: globalThis.fetch, document: globalThis.document, bootstrap: appState.bootstrap, selectedItem: appState.selectedItem };
  const progress = { revision: 'fixture', locator: '2', fraction: 0.35, completed: false, page_count: 123, bookmarks: [{ title: 'Private bookmark', locator: '1' }], annotations: [{ id: 'note', locator: '1', quote: 'Passage', note: 'Private note', color: 'yellow' }] };
  const requests = [];
  try {
    appState.bootstrap = { current_user: { id: 1 } }; appState.selectedItem = book(17, 'book.pdf');
    globalThis.document = { querySelector: () => null, querySelectorAll: () => [] };
    globalThis.fetch = async (url, options) => {
      requests.push([url, options]);
      return options.method === 'PUT' ? new Response(null, { status: 204 }) : Response.json({ revision: 'fixture', progress });
    };
    await resetBookReadingProgress(17);
    const saved = JSON.parse(requests.at(-1)[1].body);
    assert.equal(saved.locator, ''); assert.equal(saved.fraction, 0); assert.equal(saved.completed, false);
    assert.deepEqual(saved.bookmarks, progress.bookmarks); assert.equal(saved.page_count, 123);
    assert.deepEqual(saved.annotations, progress.annotations);
    assert.equal(appState.selectedItem.reading_progress.started, false);
    requests.length = 0;
    globalThis.fetch = async (url, options) => { requests.push([url, options]); appState.bootstrap = { current_user: { id: 2 } }; return Response.json({ revision: 'fixture', progress }); };
    await resetBookReadingProgress(17);
    assert.equal(requests.length, 1); assert.notEqual(requests[0][1].method, 'PUT');
  } finally {
    globalThis.fetch = original.fetch; globalThis.document = original.document; appState.bootstrap = original.bootstrap; appState.selectedItem = original.selectedItem;
  }
});
