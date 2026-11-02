/** Exercises delegated guide holds, cancellation, release suppression, and virtualized controls. */
import assert from 'node:assert/strict';
import {after,test} from 'node:test';
import {fileURLToPath} from 'node:url';
import {createServer} from 'vite';
import {JSDOM} from 'jsdom';

const server=await createServer({root:fileURLToPath(new URL('..',import.meta.url)),server:{middlewareMode:true,hmr:false,ws:false}});
after(()=>server.close());
const {bindHoldActions}=await server.ssrLoadModule('/src/app/holdActions.ts');

function fixture(t){
  t.mock.timers.enable({apis:['setTimeout']});
  const dom=new JSDOM('<button data-guide>Channel</button><button id="option">Options</button>');
  const previous={document:globalThis.document,addEventListener:globalThis.addEventListener,AbortController:globalThis.AbortController};
  globalThis.document=dom.window.document;globalThis.addEventListener=dom.window.addEventListener.bind(dom.window);globalThis.AbortController=dom.window.AbortController;
  const controller=new dom.window.AbortController(),plays=[],menus=[];
  bindHoldActions('[data-guide]',button=>plays.push(button.textContent),button=>{menus.push(button.textContent);document.querySelector('#option').focus();},controller.signal);
  const fire=(target,type,values={})=>{const Constructor=type.startsWith('key')?dom.window.KeyboardEvent:dom.window.MouseEvent;const event=new Constructor(type,{bubbles:true,cancelable:true,...values});target.dispatchEvent(event);return event;};
  t.after(()=>{controller.abort();dom.window.close();Object.assign(globalThis,previous);});
  return {dom,controller,plays,menus,fire,button:document.querySelector('[data-guide]')};
}

test('short clicks and Enter activate exactly once, while a hold consumes release on the focused option',t=>{
  const f=fixture(t);f.button.click();
  f.fire(f.button,'keydown',{key:'Enter'});t.mock.timers.tick(100);f.fire(f.button,'keyup',{key:'Enter'});
  assert.equal(f.plays.length,2);
  f.fire(f.button,'keydown',{key:'Enter'});t.mock.timers.tick(650);
  assert.equal(f.menus.length,1);
  assert.equal(f.fire(document.activeElement,'keydown',{key:'Enter',repeat:true}).defaultPrevented,true);
  assert.equal(f.fire(document.activeElement,'keyup',{key:'Enter'}).defaultPrevented,true);
  assert.equal(f.plays.length,2);
});

test('pointer holds suppress their click and later clicks still activate newly mounted rows',t=>{
  const f=fixture(t);f.fire(f.button,'pointerdown',{clientX:10,clientY:10});t.mock.timers.tick(650);
  f.fire(f.button,'pointerup');f.button.click();assert.equal(f.menus.length,1);assert.equal(f.plays.length,0);
  f.button.outerHTML='<button data-guide>Virtualized channel</button>';
  const next=document.querySelector('[data-guide]');f.fire(next,'pointerdown');f.fire(next,'pointerup');next.click();
  assert.deepEqual(f.plays,['Virtualized channel']);
  f.controller.abort();next.click();assert.equal(f.plays.length,1);
});

test('scroll gestures and focus loss cancel pending holds without starting playback',t=>{
  const f=fixture(t);f.fire(f.button,'pointerdown',{clientX:10,clientY:10});f.fire(f.button,'pointermove',{clientX:10,clientY:40});t.mock.timers.tick(650);f.button.click();
  f.button.focus();f.fire(f.button,'keydown',{key:'Enter'});document.querySelector('#option').focus();t.mock.timers.tick(650);f.fire(document.activeElement,'keyup',{key:'Enter'});
  assert.equal(f.plays.length,0);assert.equal(f.menus.length,0);
  f.fire(f.button,'contextmenu');assert.deepEqual(f.menus,['Channel']);
});

test('a guide refresh preserves a held key and uses the refreshed callbacks',t=>{
  const f=fixture(t);f.fire(f.button,'keydown',{key:'Enter'});t.mock.timers.tick(300);
  f.controller.abort();
  const next=new f.dom.window.AbortController();t.after(()=>next.abort());
  bindHoldActions('[data-guide]',()=>f.plays.push('refreshed'),()=>f.menus.push('refreshed'),next.signal);
  t.mock.timers.tick(350);assert.deepEqual(f.menus,['refreshed']);
  f.fire(f.button,'keyup',{key:'Enter'});assert.equal(f.plays.length,0);
});
