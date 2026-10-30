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

test('background updates leave owned player controls, focus, and in-progress clicks intact',()=>{
  const markup = key => `<div id="media-player-overlay" data-player-view-key="${key}"><video id="media-player" src="/buffer/one"></video><section class="is-controls-visible"><button id="close-player"><svg></svg></button><select><option>Original</option><option>720p</option></select></section></div>`;
  root.innerHTML=markup('one');
  const video=document.querySelector('video'); video.volume=0.27;
  const section=document.querySelector('section'); section.className='is-controls-hidden';
  const select=document.querySelector('select'); select.value='720p'; select.focus();
  const icon=document.querySelector('svg');
  setElementHtml(root,markup('one'));
  assert.equal(document.querySelector('select'),select);
  assert.equal(document.activeElement,select);
  assert.equal(select.value,'720p');
  assert.equal(document.querySelector('svg'),icon);
  assert.equal(section.className,'is-controls-hidden');
  assert.equal(video.volume,0.27);
  setElementHtml(root,markup('expanded'));
  assert.equal(section.className,'is-controls-visible');
});

test('a pending patch restore does not undo subsequent keyboard or pointer scrolling',async()=>{
  root.innerHTML='<div class="table-shell" id="guide"></div>';
  const guide=document.querySelector('#guide'); guide.scrollTop=100;
  setElementHtml(root,'<div class="table-shell" id="guide"><button>Next row</button></div>');
  guide.scrollTop=300;
  await new Promise(resolve=>requestAnimationFrame(resolve));
  assert.equal(guide.scrollTop,300);
});
