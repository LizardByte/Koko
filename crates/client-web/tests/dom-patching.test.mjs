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
const {bindTvGuideMappings} = await server.ssrLoadModule('/src/app/liveTvMapping.ts');
const {rememberTvGuideFocus,tvGuideNavigationTarget,tvGuideNavigationTime,restoreTvGuideFocus} = await server.ssrLoadModule('/src/app/tvGuideNavigation.ts');
const root = document.querySelector('#app');
const player = source => `<div id="media-player-overlay"><video id="media-player" src="${source}"></video></div>`;

test('background refreshes preserve discovered connection drafts and changing type resets them',()=>{
  const form = (kind,owner=1) => `<form id="tv-source-form" data-tv-source-kind="${kind}" data-tv-form-key="${owner}:${kind}"><input name="capacity" value="1"><div data-tv-discovery-results></div></form>`;
  root.innerHTML=form('hdhomerun');
  const capacity=root.querySelector('input');capacity.value='4';
  root.querySelector('[data-tv-discovery-results]').textContent='Living room tuner selected';
  setElementHtml(root,form('hdhomerun'));
  assert.equal(root.querySelector('input'),capacity);
  assert.equal(capacity.value,'4');
  assert.equal(root.querySelector('[data-tv-discovery-results]').textContent,'Living room tuner selected');
  setElementHtml(root,form('nextpvr'));
  assert.equal(root.querySelector('input').value,'1');
  assert.equal(root.querySelector('[data-tv-discovery-results]').textContent,'');
  root.querySelector('input').value='8';
  setElementHtml(root,form('nextpvr',2));
  assert.equal(root.querySelector('input').value,'1');
});

test('an open station selector survives background patches and compacts when focus leaves',()=>{
  const markup='<select data-tv-guide-source="1"><option value="">Automatic</option></select>';
  root.innerHTML=markup;
  const control=root.querySelector('select');
  const controller=new dom.window.AbortController();
  bindTvGuideMappings(root,controller.signal,[
    {source_id:1,guide_key:'chosen',name:'Station',number:'1',aliases:[]},
    {source_id:1,guide_key:'other',name:'Other station',number:'2',aliases:[]},
  ]);
  control.focus();control.value='chosen';
  setElementHtml(root,markup);
  assert.equal(document.activeElement,control);
  assert.equal(control.options.length,3);
  assert.equal(control.value,'chosen');
  control.blur();
  assert.equal(control.options.length,2);
  controller.abort();
});

test('adding a navigation hint preserves the mounted video and player controls',()=>{
  root.innerHTML = `<div>${player('/buffer/one')}</div>`;
  const video = document.querySelector('video');
  setElementHtml(root,`<div><aside>Navigation hint</aside>${player('/buffer/one')}</div>`);
  assert.equal(document.querySelector('video'),video);
  assert.equal(document.querySelector('aside').textContent,'Navigation hint');
  setElementHtml(root,`<div>${player('/buffer/one')}</div>`);
  assert.equal(document.querySelector('video'),video);
});

test('controlled source editors honor their state while ordinary settings panels retain expansion',()=>{
  root.innerHTML='<details id="source" data-controlled-open open><summary>Edit source</summary></details><details id="ordinary" open><summary>Settings</summary></details>';
  setElementHtml(root,'<details id="source" data-controlled-open><summary>Edit source</summary></details><details id="ordinary"><summary>Settings</summary></details>');
  assert.equal(document.querySelector('#source').open,false);
  assert.equal(document.querySelector('#ordinary').open,true);
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

function guideFixture(start = 0) {
  const ranges=[[[0,1800],[1800,3600],[3600,7200]],[[0,7200]],[[0,900],[900,1800],[1800,2700],[2700,3600],[3600,7200]],[],[[0,1800],[2400,7200]]];
  const rows=ranges.map((row,index)=>{
    const programs=row.map(([from,to],id)=>to>start?`<button id="p-${index}-${id}" class="tv-guide-program" data-tv-select-channel="${index}" data-tv-select-program="${id+1}" data-tv-start="${from}" data-tv-end="${to}"></button>`:'').join('');
    return `<div class="tv-guide-row"><button class="tv-channel-select" data-tv-select-channel="${index}"></button>${programs}</div>`;
  }).join('');
  return `<div id="tv-guide-scroll" data-tv-guide-source="1:1" data-tv-window-start="${start}">${rows}</div>`;
}

test('guide navigation retains a time through long broadcasts, short entries, gaps, and empty rows',()=>{
  rememberTvGuideFocus(null); root.innerHTML=guideFixture();
  let control=document.querySelector('#p-0-1'); control.focus(); rememberTvGuideFocus(control);
  const move=direction=>{control=tvGuideNavigationTarget(control,direction).target;control.focus();rememberTvGuideFocus(control);return control.id;};
  assert.equal(move('down'),'p-1-0');
  assert.equal(tvGuideNavigationTime(control),1800);
  assert.equal(move('down'),'p-2-2');
  assert.equal(move('down'),'');
  assert.equal(move('down'),'p-4-0');
  assert.equal(tvGuideNavigationTime(control),1800);
  assert.equal(move('up'),'');
  assert.equal(move('up'),'p-2-2');
  assert.equal(move('right'),'p-2-3');
  assert.equal(tvGuideNavigationTime(control),2700);
  assert.equal(move('up'),'p-1-0');
  assert.equal(move('up'),'p-0-1');
  assert.equal(tvGuideNavigationTime(control),2700);
});

test('an expired focused broadcast moves to the same channel at the new guide start',()=>{
  rememberTvGuideFocus(null); root.innerHTML=guideFixture();
  const old=document.querySelector('#p-0-0');old.focus();rememberTvGuideFocus(old);
  setElementHtml(root,guideFixture(1800));
  assert.equal(document.activeElement,document.body);
  const target=restoreTvGuideFocus();
  assert.equal(target.id,'p-0-1');
  assert.equal(tvGuideNavigationTime(target),1800);
  target.focus();rememberTvGuideFocus(target);
  assert.equal(restoreTvGuideFocus(),undefined);
  target.blur();
  assert.equal(target.isConnected,true);
  assert.equal(restoreTvGuideFocus(),target);
});

test('a queued render restore cannot undo a newer navigation focus',async()=>{
  rememberTvGuideFocus(null);root.innerHTML='<button id="before">Before</button><button id="after">After</button>';
  document.querySelector('#before').focus();
  setElementHtml(root,'<button id="before">Before refresh</button><button id="after">After refresh</button>');
  document.querySelector('#after').focus();
  await new Promise(resolve=>requestAnimationFrame(resolve));
  assert.equal(document.activeElement.id,'after');
});
