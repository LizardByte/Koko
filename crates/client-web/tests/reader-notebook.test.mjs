/** Exercises private reading marks, dark text appearance, PDF geometry, and stable text anchors. */
import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { JSDOM } from 'jsdom';
import { createServer } from 'vite';
import * as CFI from 'foliate-js/epubcfi.js';

const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
const dom = new JSDOM('', { url: 'http://localhost' });
globalThis.localStorage = dom.window.localStorage;
globalThis.location = dom.window.location;
for (const key of ['window', 'document', 'Node', 'NodeFilter', 'Range', 'Element', 'HTMLElement', 'DOMParser', 'AbortController']) globalThis[key] = dom.window[key];
after(async () => { await server.close(); dom.window.close(); });
const { readerMarkup, bindReaderControls } = await server.ssrLoadModule('/src/app/readerControls.ts');
const { createReaderNotebook } = await server.ssrLoadModule('/src/app/readerAnnotations.ts');
const { readerSelectedText, readerPdfSelection, drawReaderPdfHighlights } = await server.ssrLoadModule('/src/app/readerHighlights.ts');
const { applyReaderTextTheme } = await server.ssrLoadModule('/src/app/readerTheme.ts');

function notebook() {
  const dialog = document.createElement('div'); dialog.innerHTML = readerMarkup(); document.body.replaceChildren(dialog);
  dialog.querySelector('[data-reader-location]').textContent = 'Page 3 of 12';
  const controller = new AbortController(), saves = [], navigations = [];
  let current = { revision: 'test', locator: '2', fraction: 0.25, completed: false, bookmarks: [], annotations: [] };
  let activePanel = 'contents', book;
  const render = (panel) => {
    activePanel = panel; const list = dialog.querySelector('[data-reader-targets]'); list.replaceChildren(); book.render(list, panel);
    dialog.querySelector('.book-reader-panel').hidden = false;
  };
  book = createReaderNotebook(dialog, {
    progress: () => current,
    open: render,
    navigate: (locator) => navigations.push(locator),
    changed: () => saves.push(structuredClone(current)), status: () => {},
  }, controller.signal);
  const query = (selector) => dialog.querySelector(selector);
  return { dialog, query, book, controller, saves, navigations, render,
    get progress() { return current; }, set progress(value) { current = value; },
    submit() { query('[data-reader-note-editor]').dispatchEvent(new dom.window.Event('submit', { bubbles: true, cancelable: true })); },
    refresh() { render(activePanel); },
  };
}

test('the shared player dock keeps page labels, seeking, and reading options connected after moving the timeline', () => {
  const view = notebook(); let appearances = 0;
  const update = bindReaderControls(view.dialog, { appearance: () => { appearances++; }, seek: () => Promise.resolve(), preview: () => undefined }, view.controller.signal);
  update(0.35, 'Page 14 of 40', 0.333);
  assert.equal(view.query('[data-reader-current]').textContent, '14');
  assert.equal(view.query('[data-reader-total]').textContent, '40');
  assert.equal(view.query('[data-reader-position]').value, '333');
  assert.equal(view.query('[data-reader-location]').textContent, 'Page 14 of 40 · 35%');
  update(0.5, 'Pages 20–21 of 40'); assert.equal(view.query('[data-reader-current]').textContent, '20–21');
  update(0.1, 'Opening chapter'); assert.equal(view.query('[data-reader-current]').textContent, '10%'); assert.equal(view.query('[data-reader-total]').textContent, '');
  view.query('[data-reader-options]').click(); assert.equal(view.query('[data-reader-options]').getAttribute('aria-expanded'), 'true');
  view.query('[data-reader-direction]').click(); assert.equal(view.query('[data-reader-direction]').getAttribute('aria-pressed'), 'true');
  assert.equal(appearances, 1); view.controller.abort();
});

test('bookmarks can be named, renamed, revisited, and removed without changing reading position', () => {
  const view = notebook(); view.book.bookmark();
  assert.equal(view.query('[data-reader-mark-name]').value, 'Page 3 of 12');
  view.query('[data-reader-mark-name]').value = 'A useful passage'; view.submit();
  assert.deepEqual(view.progress.bookmarks, [{ title: 'A useful passage', locator: '2' }]);
  view.query('.reader-target button').click(); assert.deepEqual(view.navigations, ['2']);
  view.query('[aria-label="Edit bookmark"]').click(); view.query('[data-reader-mark-name]').value = 'My new name'; view.submit();
  assert.equal(view.progress.bookmarks.length, 1); assert.equal(view.progress.bookmarks[0].title, 'My new name');
  assert.equal(view.progress.locator, '2');
  view.query('[aria-label="Remove bookmark"]').click(); assert.deepEqual(view.progress.bookmarks, []);
  assert.equal(view.saves.length, 3); view.controller.abort();
});

test('selected text becomes a saved highlight with an editable private note and color', () => {
  const view = notebook(), quote = '<img src=x onerror=alert(1)> is plain text';
  view.book.events.selection({ locator: 'epubcfi(/6/2!/4/2,:0,:12)', quote });
  assert.equal(view.query('[data-reader-selection]').hidden, false);
  assert.equal(view.query('[data-reader-selection-quote]').textContent, quote);
  assert.equal(view.query('[data-reader-selection] img'), null);
  view.query('[data-reader-highlight]').click(); assert.equal(view.progress.annotations.length, 1);
  const mark = view.progress.annotations[0]; assert.equal(mark.quote, quote); assert.equal(mark.color, 'yellow');
  view.book.events.activate(mark.locator);
  view.query('[data-reader-mark-note]').value = 'My thoughts <script> remain text';
  view.query('[data-reader-mark-color="blue"]').click(); view.submit();
  assert.equal(view.progress.annotations[0].color, 'blue');
  assert.equal(view.progress.annotations[0].note, 'My thoughts <script> remain text');
  assert.equal(view.query('[data-reader-targets] script'), null);
  // Restore saved state into a fresh notebook and navigate back to the same text anchor.
  const reopened = notebook(); reopened.progress = structuredClone(view.saves.at(-1)); reopened.render('notes');
  reopened.query('.reader-target button').click(); assert.deepEqual(reopened.navigations, [mark.locator]);
  reopened.query('[aria-label="Remove highlight or note"]').click(); assert.deepEqual(reopened.progress.annotations, []);
  reopened.controller.abort(); view.controller.abort();
});

test('page notes need no text selection and aborted account controls cannot change another account', () => {
  const view = notebook(); view.query('[data-reader-options]').click();
  view.dialog.querySelectorAll('[data-reader-add-note]')[1].click();
  view.query('[data-reader-mark-note]').value = 'Private page note'; view.submit();
  assert.equal(view.progress.annotations[0].quote, ''); assert.equal(view.progress.annotations[0].locator, '2');
  const edit = view.query('[aria-label="Edit highlight or note"]');
  view.controller.abort();
  view.progress = { revision: 'test', locator: '0', fraction: 0, completed: false, bookmarks: [], annotations: [] };
  view.refresh(); edit.click();
  assert.doesNotMatch(view.query('[data-reader-targets]').textContent, /Private page note/);
  assert.deepEqual(view.progress.annotations, []); assert.equal(view.saves.length, 1);
});

test('CFI highlights restore the same passage after reflow and selection capture rejects empty or huge ranges', () => {
  document.body.innerHTML = '<article><p>Before the selected passage after.</p></article>';
  const text = document.querySelector('p').firstChild, range = document.createRange(); range.setStart(text, 11); range.setEnd(text, 27);
  const selection = document.getSelection(); selection.removeAllRanges(); selection.addRange(range);
  const selected = readerSelectedText(document); assert.equal(selected.quote, 'selected passage');
  const anchor = CFI.fromRange(selected.range);
  document.documentElement.style.fontSize = '180%'; document.body.style.width = '300px';
  assert.equal(CFI.toRange(document, CFI.parse(anchor)).toString(), 'selected passage');
  selection.collapseToEnd(); assert.equal(readerSelectedText(document), undefined);
  text.textContent = 'x'.repeat(4001); range.selectNodeContents(document.querySelector('p')); selection.removeAllRanges(); selection.addRange(range);
  assert.equal(readerSelectedText(document), undefined); selection.removeAllRanges();
});

test('dark reading overrides publisher black text and white boxes without changing text anchors or illustrations', () => {
  const chapter = new JSDOM(`<html><head><style>body,#passage { color:black!important; background-color:white!important; } p span { color:#111!important; }</style></head>
    <body style="color:black!important;background-color:white!important;font-family:serif"><h2>Book title</h2><p id="passage" style="color:black!important;font-size:24px">A <span style="-webkit-text-fill-color:black!important">selected passage</span>.</p>
      <svg xmlns="http://www.w3.org/2000/svg"><rect fill="black" style="color:black;background-color:white" /></svg><img src="cover.png" /></body></html>`);
  const doc = chapter.window.document, text = doc.querySelector('span').firstChild;
  const range = doc.createRange(); range.selectNodeContents(text);
  const anchor = CFI.fromRange(range), originalText = doc.body.textContent;
  applyReaderTextTheme(doc);
  for (const element of doc.querySelectorAll('body,h2,p,span')) {
    assert.equal(chapter.window.getComputedStyle(element).color, 'rgb(244, 247, 251)');
    assert.equal(chapter.window.getComputedStyle(element).backgroundColor, 'rgba(0, 0, 0, 0)');
  }
  assert.equal(chapter.window.getComputedStyle(doc.documentElement).backgroundColor, 'rgb(12, 17, 29)');
  assert.equal(doc.querySelector('span').style.getPropertyValue('-webkit-text-fill-color').toLowerCase(), 'currentcolor');
  assert.equal(doc.querySelector('p').style.fontSize, '24px'); assert.equal(doc.body.style.fontFamily, 'serif');
  assert.equal(doc.querySelector('rect').getAttribute('fill'), 'black'); assert.equal(doc.querySelector('rect').style.color, 'black');
  assert.equal(doc.querySelector('img').getAttribute('src'), 'cover.png');
  assert.equal(doc.body.textContent, originalText); assert.equal(CFI.toRange(doc, CFI.parse(anchor)).toString(), 'selected passage');
  applyReaderTextTheme(doc); assert.equal(CFI.fromRange(range), anchor);
  chapter.window.close();
});

test('PDF highlights follow normalized line geometry and activate the right mark on a shared page', () => {
  document.body.innerHTML = '<div id="host"><div data-reader-pdf-page="2"><p>Selected text</p></div><div data-reader-pdf-page="3"><p>Other page</p></div></div>';
  const host = document.querySelector('#host'), page = host.firstChild, text = page.querySelector('p').firstChild;
  page.getBoundingClientRect = () => ({ left: 10, top: 100, width: 200, height: 300 });
  const range = document.createRange(); range.selectNodeContents(text);
  range.getClientRects = () => [{ left: 30, top: 130, right: 110, bottom: 160 }];
  const selection = document.getSelection(); selection.removeAllRanges(); selection.addRange(range);
  const selected = readerPdfSelection(host); assert.equal(selected.locator, '2');
  assert.deepEqual(selected.rects, [{ x: 0.1, y: 0.1, width: 0.4, height: 0.1 }]);
  const activated = [], mark = { id: 'one', ...selected, color: 'yellow', note: '' };
  drawReaderPdfHighlights(host, [mark, { ...mark, id: 'two', note: 'Second' }], (id) => activated.push(id));
  const buttons = host.querySelectorAll('.reader-pdf-highlight'); assert.equal(buttons.length, 2);
  assert.equal(buttons[0].style.left, '10%'); assert.equal(buttons[0].style.width, '40%');
  buttons[1].click(); assert.deepEqual(activated, ['two']);
  drawReaderPdfHighlights(host, []); assert.equal(host.querySelector('.reader-pdf-highlight'), null);
  range.setEnd(host.lastChild.querySelector('p').firstChild, 3); assert.equal(readerPdfSelection(host), undefined);
  selection.removeAllRanges();
});
