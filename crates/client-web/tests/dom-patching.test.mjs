/** Verifies player continuity across guide updates and intentional stream replacements. */
import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';
import { JSDOM } from 'jsdom';

const dom = new JSDOM('<main id="app"></main>', {pretendToBeVisual:true});
for (const key of ['document','Node','Element','HTMLElement','HTMLMediaElement','HTMLInputElement','HTMLTextAreaElement','HTMLSelectElement','HTMLDetailsElement']) globalThis[key] = dom.window[key];
globalThis.CSS = {escape:String};
globalThis.requestAnimationFrame = dom.window.requestAnimationFrame.bind(dom.window);
globalThis.cancelAnimationFrame = dom.window.cancelAnimationFrame.bind(dom.window);
const server = await createServer({root:fileURLToPath(new URL('..',import.meta.url)),server:{middlewareMode:true,hmr:false,ws:false}});
after(async()=>{await server.close();dom.window.close();});
const {setElementHtml} = await server.ssrLoadModule('/src/app/domPatcher.ts');
const root = document.querySelector('#app');
const player = source => `<div id="media-player-overlay"><video id="media-player" src="${source}"></video></div>`;

test('adding a navigation hint preserves the mounted video and player controls',()=>{
  root.innerHTML = `<div>${player('/buffer/one')}</div>`;
  const video = document.querySelector('video');
  setElementHtml(root,`<div><aside>Navigation hint</aside>${player('/buffer/one')}</div>`);
  assert.equal(document.querySelector('video'),video);
  assert.equal(document.querySelector('aside').textContent,'Navigation hint');
  setElementHtml(root,`<div>${player('/buffer/one')}</div>`);
  assert.equal(document.querySelector('video'),video);
});

test('changing the media source replaces the decoder element even with the same player id',()=>{
  root.innerHTML = player('/buffer/one');
  const previous = document.querySelector('video');
  setElementHtml(root,player('/buffer/two'));
  assert.notEqual(document.querySelector('video'),previous);
  assert.equal(document.querySelector('video').getAttribute('src'),'/buffer/two');
});
