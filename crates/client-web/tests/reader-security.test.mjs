/** Exercises book sanitization in a real DOM, including hostile head markup and fixed-layout metadata. */
import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { JSDOM } from 'jsdom';
import { createServer } from 'vite';

const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
const dom = new JSDOM('');
globalThis.window = dom.window;
globalThis.document = dom.window.document;
globalThis.DOMParser = dom.window.DOMParser;
after(async () => { await server.close(); dom.window.close(); });
const { sanitizeBookDocument } = await server.ssrLoadModule('/src/app/readerSupport.ts');
const sanitize = (html) => new DOMParser().parseFromString(sanitizeBookDocument(html), 'text/html');

test('book documents discard executable markup and attacker-supplied head policies', () => {
  const doc = sanitize(`<html><head><base href="https://attacker.invalid/"><meta http-equiv="refresh" content="0;url=https://attacker.invalid/">
    <meta http-equiv="Content-Security-Policy" content="script-src *"><link rel="stylesheet" href="https://attacker.invalid/style.css"></head>
    <body onload="alert(1)"><script>alert(1)</script><iframe srcdoc="<script>alert(1)</script>"></iframe><form><input name="x"></form>
    <img src="cover.png" onerror="alert(1)"><svg onload="alert(1)"><script>alert(1)</script></svg><p id="safe">The book remains readable.</p></body></html>`);
  assert.equal(doc.querySelector('script,iframe,object,embed,form,base,link,[onload],[onerror]'), null);
  assert.equal(doc.querySelector('#safe').textContent, 'The book remains readable.');
  const policies = doc.querySelectorAll('meta');
  assert.equal(policies.length, 1);
  assert.equal(policies[0].httpEquiv, 'Content-Security-Policy');
  assert.match(policies[0].content, /script-src 'none'/);
  assert.match(policies[0].content, /img-src blob: data:/);
  assert.match(policies[0].content, /connect-src 'none'/);
});

test('fixed-layout books keep styles, language, and a rebuilt numeric viewport', () => {
  const doc = sanitize(`<html class="fixed-layout" lang="ar" dir="rtl"><head><meta name="viewport" content="width=600,height=800" onload="alert(1)">
    <link rel="stylesheet" href="blob:https://koko.local/styles" onload="alert(1)">
    <style>p { color: red; }</style></head><body class="chapter" style="margin: 1em" onclick="alert(1)"><p>Styled text</p></body></html>`);
  assert.equal(doc.documentElement.lang, 'ar');
  assert.equal(doc.documentElement.className, 'fixed-layout');
  assert.equal(doc.documentElement.dir, 'rtl');
  assert.match(doc.querySelector('style').textContent, /color: red/);
  assert.equal(doc.body.className, 'chapter');
  assert.equal(doc.body.style.margin, '1em');
  assert.equal(doc.body.hasAttribute('onclick'), false);
  assert.equal(doc.querySelector('link').getAttribute('href'), 'blob:https://koko.local/styles');
  assert.deepEqual(Array.from(doc.querySelector('link').attributes, (attribute) => attribute.name).sort(), ['href', 'rel']);
  const viewport = doc.querySelector('meta[name="viewport"]');
  assert.equal(viewport.content, 'width=600,height=800');
  assert.deepEqual(Array.from(viewport.attributes, (attribute) => attribute.name).sort(), ['content', 'name']);
  assert.equal(doc.head.firstElementChild.httpEquiv, 'Content-Security-Policy');
});

test('invalid viewports are discarded and SVG covers derive dimensions from their safe viewBox', () => {
  const doc = sanitize('<meta name="viewport" content="width=Infinity,height=10"><svg viewBox="0 0 400 700" xmlns="http://www.w3.org/2000/svg"><rect width="400" height="700" /></svg>');
  assert.equal(doc.querySelector('meta[name="viewport"]').content, 'width=400,height=700');
  assert.equal(doc.querySelector('svg').getAttribute('viewBox'), '0 0 400 700');
  assert.equal(sanitize('<meta name="viewport" content="width=600,height=800;url=javascript:alert(1)"><p>Text</p>').querySelector('meta[name="viewport"]'), null);
});

test('unknown and executable URI schemes cannot become book links while local resources survive', () => {
  const doc = sanitize(`<a id="script" href="java&#10;script:alert(1)">bad</a><a id="unknown" href="custom0:payload">bad</a>
    <a id="data" href="data:text/html,attack">bad</a><a id="local" href="chapter2.xhtml#section">next</a>
    <a id="web" href="https://example.com">source</a><img id="image" src="blob:https://koko.local/cover"><img id="data-image" src="data:image/png;base64,AA==">`);
  for (const id of ['script', 'unknown', 'data']) assert.equal(doc.getElementById(id).hasAttribute('href'), false);
  assert.equal(doc.getElementById('local').getAttribute('href'), 'chapter2.xhtml#section');
  assert.equal(doc.getElementById('web').getAttribute('href'), 'https://example.com');
  assert.equal(doc.getElementById('image').getAttribute('src'), 'blob:https://koko.local/cover');
  assert.equal(doc.getElementById('data-image').getAttribute('src'), 'data:image/png;base64,AA==');
});
