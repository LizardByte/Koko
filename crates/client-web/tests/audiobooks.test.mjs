import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/', origin: 'https://koko.local' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { orderedAudiobookParts, audiobookChapterTargets, audiobookChapterStatus, audiobookRestartTarget, renderAudiobookContents } = await server.ssrLoadModule('/src/app/audiobooks.ts');
const { renderBookActions } = await server.ssrLoadModule('/src/app/bookActions.ts');
const { renderAudiobookAction } = await server.ssrLoadModule('/src/app/audiobookActions.ts');
const { setMusicPlayback, clearMusicPlayback } = await server.ssrLoadModule('/src/app/musicPlaybackState.ts');
const { formatChildCount } = await server.ssrLoadModule('/src/app/ui.ts');
const { renderItemCard } = await server.ssrLoadModule('/src/app/homeView.ts');
const { AudioQueue } = await server.ssrLoadModule('/src/app/audioQueue.ts');
const { renderAudioPlayer, formatAudioTime } = await server.ssrLoadModule('/src/app/audioPlayerView.ts');
const { renderItemPage } = await server.ssrLoadModule('/src/app/itemPersonView.ts');
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const mock = await server.ssrLoadModule('/src/mockApi.ts');

function part(id, path, extra = {}) {
  return { ...mock.getMockItem(103), id, parent_id: 500, item_type: 'audiobook_part', media_kind: 'audio', playable: true, display_title: path, relative_path: path, duration_ms: 60000, music: null, audiobook: { title: 'Book', author: 'Author', narrator: '', chapters: [] }, ...extra };
}
function book(children, chapters = []) {
  return { ...mock.getMockItem(302), id: 500, display_title: 'Book', item_type: 'audiobook', media_kind: 'audio', playable: false, music: null, hierarchy: [], children, audiobook: { title: 'Book', author: 'A <writer>', narrator: 'The Reader', recording: { provider_id: 'audiosilo', external_id: 'book/reader', title: 'Book', narrator: 'The Reader', asins: [] }, chapters } };
}

test('long recordings display hours while short file positions remain compact', () => {
  assert.equal(formatAudioTime(8 * 3600 + 23 * 60 + 18), '8:23:18');
  assert.equal(formatAudioTime(35.25), '0:35');
});

test('untagged CD tracks use natural numeric order without mutating the inventory', () => {
  const parts = [part(3, 'CD 2/Track 1.mp3'), part(2, 'CD 1/Track 10.mp3'), part(1, 'CD 1/Track 2.mp3')];
  assert.deepEqual(orderedAudiobookParts(parts).map((part) => part.id), [1, 2, 3]);
  assert.deepEqual(parts.map((part) => part.id), [3, 2, 1]);
  parts[0].audiobook.disc_number = 1;
  parts[0].audiobook.part_number = 1;
  assert.deepEqual(orderedAudiobookParts(parts).map((part) => part.id), [1, 2, 3]);
  parts[1].audiobook.part_number = 3;
  parts[2].audiobook.part_number = 2;
  assert.equal(orderedAudiobookParts(parts)[0].id, 3);
});

test('original LibriVox filenames retain order when only one file has a track tag', () => {
  const parts = [3, 0, 1, 2, 4, 5, 6].map((number) => part(number + 1, `commonsense_${number}_paine_64kb.mp3`));
  parts[0].audiobook.part_number = 4;
  assert.deepEqual(orderedAudiobookParts(parts).map((part) => part.id), [1, 2, 3, 4, 5, 6, 7]);
});

test('recording chapters can begin mid-file and span file boundaries', () => {
  const item = book([part(1, '1.mp3'), part(2, '2.mp3'), part(3, '3.mp3')], [
    { title: 'One', start_ms: 0, end_ms: 90000 },
    { title: 'Two', start_ms: 90000, end_ms: 180000 },
  ]);
  assert.deepEqual(audiobookChapterTargets(item), [
    { title: 'One', itemId: 1, startMs: 0 },
    { title: 'Two', itemId: 2, startMs: 30000 },
  ]);
  item.audiobook.chapters[1].start_ms = 60000;
  assert.equal(audiobookChapterTargets(item)[1].itemId, 2);
  assert.equal(audiobookChapterTargets(item)[1].startMs, 0);
  item.children[0].missing_since = 1;
  assert.deepEqual(audiobookChapterTargets(item), []);
  item.children[0].missing_since = null;
  item.children[0].duration_ms = 30000;
  assert.deepEqual(audiobookChapterTargets(item), []);
});

test('embedded chapter offsets take precedence and never become track numbers', () => {
  const item = book([part(1, '1.m4b', { audiobook: { title: 'Book', author: '', narrator: '', chapters: [{ title: '<Chapter>', start_ms: 35250, end_ms: 60000 }] } })]);
  assert.deepEqual(audiobookChapterTargets(item), [{ title: '<Chapter>', itemId: 1, startMs: 35250 }]);
  const html = renderAudiobookContents(item);
  assert.match(html, /data-playback-target-start-ms="35250"/);
  assert.match(html, /&lt;Chapter&gt;/);
  assert.match(html, /A &lt;writer&gt;/);
  assert.match(html, /Narrated by The Reader/);
  assert.match(html, /Audio files/);
});

test('audiobook dock provides speed and chapters with original file ordering', () => {
  const queue = new AudioQueue(); queue.replace([part(1, 'Track 1')]);
  const html = renderAudioPlayer({ queue, item: queue.current.item, loading: false, paused: false, volume: 1, muted: false, lyricsLoading: false, saving: false, panel: 'chapters', playbackRate: 1.5, chapters: [{ title: 'Chapter <two>', itemId: 1, startMs: 35250 }] });
  assert.match(html, /aria-label="Playback speed"/);
  assert.match(html, /value="1.5" selected/);
  assert.match(html, /data-audio-chapter="0"/);
  assert.match(html, /Chapter &lt;two&gt;/);
  assert.match(html, /class="music-title"[^>]*data-music-open-item="500">Book<\/button>/);
  assert.doesNotMatch(html, />Track 1<\/button>/);
  for (const label of ['Shuffle', 'Lyrics', 'Repeat off', 'Save as playlist']) assert.equal(html.includes(label), false);
});

test('LibriVox sections use matched local file starts despite accumulated encoder padding', () => {
  const item = book([part(1, 'Book/war_01_malins_128kb.mp3', { duration_ms: 62000 }), part(2, 'Book/war_02_malins_128kb.mp3', { duration_ms: 67000 })], [
    { title: 'Part I, Chapters I and II', start_ms: 0, end_ms: 60000 },
    { title: 'Part II', start_ms: 60000, end_ms: 125000 },
  ]);
  item.audiobook.recording = { provider_id: 'librivox', external_id: '6112/recording', title: 'Book', narrator: 'R. S. Steinberg', asins: [] };
  item.audiobook.chapter_files = ['war_01_malins_64kb.mp3', 'war_02_malins_64kb.mp3'];
  assert.deepEqual(audiobookChapterTargets(item), [
    { title: 'Part I, Chapters I and II', itemId: 1, startMs: 0 }, { title: 'Part II', itemId: 2, startMs: 0 },
  ]);
  item.children[1].duration_ms = 90000;
  assert.deepEqual(audiobookChapterTargets(item), []);
  item.children[1].duration_ms = 67000; item.children[1].relative_path = item.children[0].relative_path;
  assert.deepEqual(audiobookChapterTargets(item), []);
  item.children[1].relative_path = 'Book/war_02_malins_128kb.mp3'; item.children[1].missing_since = 1;
  assert.deepEqual(audiobookChapterTargets(item), []);
});

test('a combined LibriVox file uses supplied timeline boundaries when the runtime agrees', () => {
  const item = book([part(1, 'Book.m4b', { duration_ms: 125000 })], [
    { title: 'One', start_ms: 0, end_ms: 60000 }, { title: 'Two', start_ms: 60000, end_ms: 125000 },
  ]);
  item.audiobook.recording.provider_id = 'librivox';
  item.audiobook.chapter_files = ['one.mp3', 'two.mp3'];
  assert.deepEqual(audiobookChapterTargets(item), [{ title: 'One', itemId: 1, startMs: 0 }, { title: 'Two', itemId: 1, startMs: 60000 }]);
});

test('audiobook detail uses the server resume target and exposes recording metadata matching', () => {
  const item = book([part(1, '1.mp3')]);
  item.playback_target = { item_id: 1, start_ms: 35000, label: 'Continue audiobook', display_title: 'Track 1', resume: true };
  state.bootstrap = { current_user: { id: 1, admin: true } };
  state.selectedItem = item;
  state.route = { page: 'item', itemId: 500 };
  state.libraryItems = [item, ...item.children];
  state.metadataProviders = mock.getMockMetadataProviders();
  const html = renderItemPage();
  assert.match(html, /Resume listening/);
  assert.match(html, /data-listen-book="500"[^>]*data-resume-hold/);
  assert.match(html, /aria-controls="audiobook-progress-menu"/);
  assert.match(html, /data-restart-audiobook="track"/); assert.match(html, /data-restart-audiobook="book"/);
  assert.match(html, /audiobook-contents/);
  assert.match(html, /audiobook-detail-page/);
});

test('both Book and audiobook pages expose the same hold actions, including during paused or active listening', () => {
  const bootstrap = state.bootstrap;
  const item = book([part(1, '1.mp3'), part(2, '2.mp3')]);
  item.playback_position_ms = 95000;
  const work = { ...item, id: 499, item_type: 'book_work', children: [item] };
  state.bootstrap = { current_user: { id: 1 } };
  try {
    for (const html of [renderAudiobookAction(item), renderBookActions(work)]) {
      assert.match(html, />Resume listening</); assert.match(html, /data-resume-hold/);
      assert.match(html, /Restart track/); assert.match(html, /Restart audiobook/);
      assert.doesNotMatch(html, /Restart episode/);
    }
    state.selectedItem = work; state.route = { page: 'item', itemId: work.id };
    const page = renderItemPage();
    assert.equal((page.match(/id="audiobook-progress-menu"/g) ?? []).length, 1);
    for (const paused of [true, false]) {
      setMusicPlayback({ userId: 1, item: item.children[1], paused, positionMs: 35000, durationMs: 60000, ended: false });
      const html = renderBookActions(work);
      assert.match(html, new RegExp(`>${paused ? 'Resume' : 'Pause'} listening<`));
      assert.match(html, /data-restart-audiobook="book"/);
    }
  } finally { clearMusicPlayback(); state.bootstrap = bootstrap; }
});

test('restart targets distinguish the first file, saved current track, and this account live track', () => {
  const bootstrap = state.bootstrap;
  const item = book([part(3, 'CD 2/Track 1.mp3', { audiobook: { disc_number: 2 } }), part(2, 'CD 1/Track 2.mp3'), part(1, 'CD 1/Track 1.mp3')]);
  item.playback_target = { item_id: 2, start_ms: 35000, resume: true };
  state.bootstrap = { current_user: { id: 1 } };
  try {
    assert.equal(audiobookRestartTarget(item, 'book').id, 1);
    assert.equal(audiobookRestartTarget(item, 'track').id, 2);
    setMusicPlayback({ userId: 1, item: item.children[0], paused: true, ended: false });
    assert.equal(audiobookRestartTarget(item, 'track').id, 3);
    assert.equal(audiobookRestartTarget(item, 'book').id, 1);
    state.bootstrap = { current_user: { id: 2 } };
    assert.equal(audiobookRestartTarget(item, 'track').id, 2);
    clearMusicPlayback(); item.playback_target = null;
    item.children[0].last_played_at = 200;
    assert.equal(audiobookRestartTarget(item, 'track').id, 3);
    item.children.forEach((part) => { part.missing_since = 1; });
    assert.equal(audiobookRestartTarget(item, 'book'), undefined);
  } finally { clearMusicPlayback(); state.bootstrap = bootstrap; }
});

test('audiobook files reuse music rows with separate numbers, clean titles, durations, and file details', () => {
  const parts = ['0 Introduction and Preface', '1 Origin of Government', '2 Monarchy', '3a Thoughts pt. 1', '3b Thoughts pt. 2', '4 The Present Ability', '5 Appendix'].map((title, index) => part(index + 1, title, { playback_position_ms: index === 1 ? 10000 : 0 }));
  const html = renderAudiobookContents(book(parts));
  assert.match(html, /music-track-list audiobook-track-list/);
  assert.match(html, /music-track-number">0<\/span>/);
  assert.match(html, /music-track-number">3a<\/span>/);
  assert.match(html, /music-track-number">3b<\/span>/);
  assert.match(html, /<strong>Introduction and Preface<\/strong>/);
  assert.doesNotMatch(html, /<strong>0 Introduction|audiobook-file|Start radio|Add to queue|Metadata not matched/);
  assert.match(html, /music-track-duration">1:00<\/span>/);
  assert.match(html, /In progress: 17% listened/);
  assert.match(html, /data-play-music-track-id="1"/);
  assert.match(html, /data-item-id="1">File details/);
  const literal = renderAudiobookContents(book([part(10, '1984 A Novel'), part(11, '1.mp3')]));
  assert.match(literal, /<strong>1984 A Novel<\/strong>/); assert.match(literal, /<strong>1.mp3<\/strong>/);
  parts[0].missing_since = 1;
  assert.match(renderAudiobookContents(book(parts)), /data-play-music-track-id="1"[^>]*disabled/);
});

test('track prefixes handle delimiters and long whitespace without consuming literal titles', () => {
  for (const [title, expected] of [
    ['Track 01 - Introduction', 'Introduction'], ['Part 1: Introduction', 'Introduction'],
    ['1_Introduction', 'Introduction'], ['1. Introduction', 'Introduction'],
    ['1.Introduction', '1.Introduction'], ['1984 A Novel', '1984 A Novel'],
    [`1${' '.repeat(100000)}Introduction`, 'Introduction'],
  ]) {
    const html = renderAudiobookContents(book([part(1, title)]));
    assert.ok(html.includes(`<strong>${expected}</strong>`));
  }
  const literal = `1${' '.repeat(100000)}-`;
  assert.ok(renderAudiobookContents(book([part(1, literal)])).includes(`<strong>${literal}</strong>`));
});

test('audiobook badges show total runtime and ebook badges show only real page counts', () => {
  const item = { ...book([part(1, '1.mp3')]), child_count: 45, duration_ms: 18600000 };
  assert.equal(formatChildCount(item), '5:10:00');
  assert.match(renderItemCard(item), /media-card-duration" >5:10:00/);
  assert.doesNotMatch(renderItemCard(item), /45 audio files/);
  const ebook = { ...item, item_type: 'book', relative_path: 'book.epub', child_count: 0, reading_progress: { page_count: 123 } };
  assert.match(renderItemCard(ebook), /media-card-duration" >123 pages/);
  ebook.reading_progress = null;
  assert.match(renderItemCard(ebook), /media-card-duration" hidden><\/span>/);
  assert.doesNotMatch(renderItemCard(ebook), />EPUB<|>Unknown</);
  item.duration_ms = undefined;
  assert.equal(formatChildCount(item), '');
});

test('Book listening follows the saved audio file and propagates access failures instead of restarting', async () => {
  const { startPlaybackForItemId } = await server.ssrLoadModule('/src/app/playbackController.ts');
  const original = { fetch: globalThis.fetch, bootstrap: state.bootstrap, selectedItem: state.selectedItem };
  const recording = book([part(1, '1.mp3'), part(2, '2.mp3')]);
  recording.playback_target = { item_id: 2, start_ms: 35000, label: 'Continue audiobook', display_title: 'Part 2', resume: true };
  recording.playback_position_ms = 95000;
  const requests = [];
  try {
    state.bootstrap = { current_user: { id: 1 } }; state.selectedItem = { ...recording, id: 499, item_type: 'book_work' };
    globalThis.fetch = async (url) => {
      requests.push(String(url));
      if (String(url).endsWith('/items/500')) return Response.json(recording);
      return Response.json({ message: 'Library access denied' }, { status: 403 });
    };
    await assert.rejects(startPlaybackForItemId(500), /Library access denied/);
    assert.deepEqual(requests.map((url) => url.split('/').at(-1)), ['500', '2']);
  } finally { globalThis.fetch = original.fetch; state.bootstrap = original.bootstrap; state.selectedItem = original.selectedItem; }
});

test('changing accounts during a Book listening lookup cannot start the previous account narration', async () => {
  const { startPlaybackForItemId } = await server.ssrLoadModule('/src/app/playbackController.ts');
  const original = { fetch: globalThis.fetch, bootstrap: state.bootstrap, selectedItem: state.selectedItem };
  let finish; const requests = [];
  try {
    state.bootstrap = { current_user: { id: 1 } }; state.selectedItem = undefined;
    globalThis.fetch = (url) => { requests.push(String(url)); return new Promise((resolve) => { finish = resolve; }); };
    const pending = startPlaybackForItemId(500);
    state.bootstrap = { current_user: { id: 2 } };
    finish(Response.json({ ...book([]), playback_target: { item_id: 2, start_ms: 35000 } }));
    await pending;
    assert.equal(requests.length, 1);
  } finally { globalThis.fetch = original.fetch; state.bootstrap = original.bootstrap; state.selectedItem = original.selectedItem; }
});

test('audiobook cards use square covers and expose a separate written-work identity', async () => {
  const { renderItemCard } = await server.ssrLoadModule('/src/app/homeView.ts');
  const item = book([part(1, '1.mp3')]);
  item.audiobook.openlibrary_work_id = 'works/OL6036787W';
  assert.match(renderItemCard(item), /media-card-art audio audiobook-art/);
  assert.match(renderAudiobookContents(item), /https:\/\/openlibrary.org\/works\/OL6036787W/);
});

test('recording confirmation and chapter timing failures have different explanations', () => {
  const item = book([part(1, '1.mp3')], [{ title: 'One', start_ms: 0, end_ms: 120000 }]);
  assert.match(audiobookChapterStatus(item), /provider timeline is 2:00, and the local recording is 1:00/);
  item.audiobook.recording = null;
  assert.deepEqual(audiobookChapterTargets(item), []);
  assert.match(audiobookChapterStatus(item), /Choose the recording/);
  item.children[0].audiobook.chapters = [{ title: 'Embedded', start_ms: 35250, end_ms: 60000 }];
  assert.equal(audiobookChapterStatus(item), '');
});

test('recording picker identifies narrations and ignores choices from another item', () => {
  const item = book([part(1, '1.mp3')]);
  const choices = { itemId: item.id, loading: false, choices: [{ provider_id: 'audiosilo', external_id: 'book/another', title: 'Book', narrator: '<Another reader>', runtime_ms: 120000, language: 'en', release_year: 2009, abridged: false, asins: [] }] };
  const html = renderAudiobookContents(item, true, choices);
  assert.match(html, /&lt;Another reader&gt; · 2009 · en · Unabridged · 2:00/);
  assert.match(html, /data-link-metadata="500:audiosilo:book\/another:audiobook"/);
  assert.doesNotMatch(renderAudiobookContents(item, false, choices), /Use this recording/);
  assert.doesNotMatch(renderAudiobookContents(item, true, { ...choices, itemId: 999 }), /Use this recording/);
});

test('recording choices show provider attribution images and each narration own artwork', () => {
  const original = { metadataProviders: state.metadataProviders, selectedItemMetadata: state.selectedItemMetadata };
  try {
    state.selectedItemMetadata = undefined;
    state.metadataProviders = ['libex', 'audiosilo', 'librivox'].map((id) => ({ id, display_name: id, logo_dark_url: `/assets/providers/${id}.png` }));
    const item = book([]);
    const choices = { itemId: item.id, loading: false, choices: ['libex', 'audiosilo', 'librivox'].map((id) => ({ provider_id: id, external_id: `book/${id}`, title: 'Book <one>', narrator: id, asins: [], artwork_url: `https://covers.example/${id}.jpg?x=1&y=2` })) };
    const html = renderAudiobookContents(item, true, choices);
    for (const id of ['libex', 'audiosilo', 'librivox']) {
      assert.match(html, new RegExp(`metadata-attribution-logo" src="https://koko.local/assets/providers/${id}.png" alt="${id}"`));
      assert.match(html, new RegExp(`audiobook-recording-cover" src="https://covers.example/${id}.jpg\\?x=1&amp;y=2" alt=""`));
    }
    assert.match(html, /· Book &lt;one&gt;/);
    choices.choices[0].artwork_url = null;
    assert.equal((renderAudiobookContents(item, true, choices).match(/class="audiobook-recording-cover"/g) ?? []).length, 2);
    state.metadataProviders = [];
    assert.match(renderAudiobookContents(item, true, choices), /<span>audiosilo<\/span>/);
  } finally { Object.assign(state, original); }
});

test('book pages group local versions and ebook files without offering audiobook playback', () => {
  const version = book([part(1, '1.mp3')]);
  const author = { ...version, id: 601, item_type: 'book_author', display_title: 'The Author', parent_id: null };
  const work = { ...version, id: 600, parent_id: author.id, hierarchy: [author], item_type: 'book_work', media_kind: 'book', children: [version, { ...mock.getMockItem(103), item_type: 'book', music: null, media_kind: 'book', playable: false }], playback_target: { item_id: 1, start_ms: 0, label: 'Play album', display_title: 'Wrong default' }, restart_playback_target: null };
  state.selectedItem = work;
  state.route = { page: 'item', itemId: work.id };
  const html = renderItemPage();
  assert.match(html, /<h3>Audiobook versions <span class="item-row-count">· 1<\/span><\/h3>/);
  assert.match(html, /<h3>Ebook files <span class="item-row-count">· 1<\/span><\/h3>/);
  assert.match(html, /item-title-fallback">Book</);
  assert.doesNotMatch(html, /audiobook-detail-page|audiobook-contents|Play album/);
  state.selectedItem = { ...work, children: [] };
  assert.doesNotMatch(renderItemPage(), /data-nav-section="(?:audiobook|book)"/);
});

test('author profiles show biographies and dates and offer Open Library metadata management', async () => {
  const { canManuallyLinkMetadata } = await server.ssrLoadModule('/src/app/selectors.ts');
  const { selectedItemMetadataProviderOptions } = await server.ssrLoadModule('/src/app/itemPersonView.ts');
  const work = { ...book([]), item_type: 'book_work', id: 601, parent_id: 600 };
  const author = { ...work, id: 600, parent_id: null, item_type: 'book_author', media_kind: 'book', display_title: 'The Author', audiobook: null,
    overview: 'A biography with <untrusted text>', tagline: 'Born 1900 · Died 1980', poster_url: '/api/v1/items/600/artwork?kind=poster', children: [work] };
  state.selectedItem = author;
  state.route = { page: 'item', itemId: author.id };
  state.selectedItemMetadata = undefined;
  state.metadataProviders = ['libex', 'open_library', 'audiosilo'].map((id) => ({ id, display_name: id, role: id === 'audiosilo' ? 'secondary' : 'primary', configured: true, implemented: true, supported_kinds: ['books'] }));
  assert.equal(canManuallyLinkMetadata(author), true);
  assert.deepEqual(selectedItemMetadataProviderOptions().map((provider) => provider.id), ['open_library']);
  const html = renderItemPage();
  assert.match(html, /A biography with &lt;untrusted text&gt;/);
  assert.match(html, /Born 1900 · Died 1980/);
  assert.match(html, /metadata-search-form/);
  assert.doesNotMatch(html, /id="metadata-search-year"|id="metadata-search-artist"|audiobook-contents/);
});

test('author search identifies author profiles and keeps book results and identity confidence out', async () => {
  const { renderMetadataSearchResults } = await server.ssrLoadModule('/src/app/itemPersonView.ts');
  state.selectedItem = { ...book([]), item_type: 'book_author', audiobook: null, display_title: 'Thomas Paine' };
  const result = { provider_id: 'open_library', external_id: 'authors/OL31439A', media_type: 'author', title: 'Thomas Paine', overview: 'Born 1737 · Known for: Rights of Man · 919 works listed', score: 1 };
  state.metadataSearchResults = [result,
    { ...result, title: 'Book result must not appear', media_type: 'book', external_id: 'works/OL1W' },
    { ...result, title: 'Invalid identity must not appear', external_id: 'authors/../bad' }];
  const html = renderMetadataSearchResults();
  assert.match(html, /Author profile/);
  assert.match(html, /Known for: Rights of Man/);
  assert.match(html, /919 works listed/);
  assert.match(html, /https:\/\/openlibrary.org\/authors\/OL31439A/);
  assert.doesNotMatch(html, /Unknown year|100% match|must not appear/);
  state.metadataSearchResults = [];
});
