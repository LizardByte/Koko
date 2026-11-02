/** Delegates click, keyboard, and hold actions without retaining virtualized guide controls. */
import { RESUME_HOLD_MS } from './resumeHold';

type Action=(button:HTMLButtonElement)=>void;
interface Binding {scope:AbortSignal;controller:AbortController;generation:number;activate:Action;options:Action}
const bindings=new Map<string,Binding>();

/** Keep a pending hold through DOM patches, removing listeners when its route stops rebinding. */
export function bindHoldActions(selector:string, activate:Action, options:Action, signal:AbortSignal):void {
  if(signal.aborted)return;
  let binding=bindings.get(selector);
  if(!binding){
    binding={scope:signal,controller:new AbortController(),generation:0,activate,options};bindings.set(selector,binding);
    const current=binding;
    listenForHolds(selector,button=>{if(!current.scope.aborted)current.activate(button);},button=>{if(!current.scope.aborted)current.options(button);},current.controller.signal);
  }
  binding.scope=signal;binding.activate=activate;binding.options=options;
  const current=binding,generation=++binding.generation;
  signal.addEventListener('abort',()=>queueMicrotask(()=>{
    if(current.generation!==generation)return;
    current.controller.abort();bindings.delete(selector);
  }),{once:true});
}

/** Short presses activate; holds or context-menu gestures open options and consume release. */
function listenForHolds(selector:string, activate:Action, options:Action, signal:AbortSignal):void {
  type Press={button:HTMLButtonElement;source:'pointer'|'keyboard';opened:boolean;x:number;y:number};
  let press:Press|undefined,timer:ReturnType<typeof setTimeout>|undefined;
  let suppressClick=false,releaseKey:string|undefined;
  const target=(event:Event)=> (event.target as Element|null)?.closest?.<HTMLButtonElement>(selector);
  const cancel=()=>{
    if(timer!==undefined) clearTimeout(timer);
    if(press?.source==='pointer') suppressClick=true;
    press?.button.classList.remove('is-holding');
    press=undefined;timer=undefined;
  };
  const begin=(button:HTMLButtonElement,source:Press['source'],x=0,y=0)=>{
    if(button.disabled||signal.aborted) return;
    cancel();suppressClick=false;press={button,source,opened:false,x,y};button.classList.add('is-holding');
    timer=setTimeout(()=>{
      timer=undefined;if(!press||!button.isConnected) {cancel();return;}
      press.opened=true;button.classList.remove('is-holding');suppressClick=source==='pointer';options(button);
    },RESUME_HOLD_MS);
  };
  document.addEventListener('click',event=>{
    if(suppressClick){event.preventDefault();event.stopImmediatePropagation();suppressClick=false;return;}
    const button=target(event);if(button&&!button.disabled&&!event.defaultPrevented) activate(button);
  },{signal,capture:true});
  document.addEventListener('pointerdown',event=>{
    suppressClick=false;const button=target(event);
    if(button&&event.button===0) {begin(button,'pointer',event.clientX,event.clientY);button.setPointerCapture?.(event.pointerId);}
  },{signal,capture:true});
  document.addEventListener('pointermove',event=>{
    if(press?.source==='pointer'&&!press.opened&&Math.hypot(event.clientX-press.x,event.clientY-press.y)>12) cancel();
  },{signal,capture:true});
  document.addEventListener('pointerup',()=>{if(press?.source==='pointer') {if(timer!==undefined){clearTimeout(timer);}press.button.classList.remove('is-holding');press=undefined;timer=undefined;}},{signal,capture:true});
  document.addEventListener('pointercancel',cancel,{signal,capture:true});
  document.addEventListener('scroll',()=>{if(press?.source==='pointer'&&!press.opened) cancel();},{signal,capture:true,passive:true});
  document.addEventListener('focusout',event=>{if(press&&!press.opened&&event.target===press.button) cancel();},{signal,capture:true});
  document.addEventListener('contextmenu',event=>{
    const button=target(event);if(!button||button.disabled)return;
    event.preventDefault();cancel();options(button);
  },{signal});
  document.addEventListener('keydown',event=>{
    if(event.repeat&&event.key===releaseKey){event.preventDefault();event.stopImmediatePropagation();return;}
    if(!event.repeat) suppressClick=false;
    const button=target(event);
    if(!button||button.disabled||event.altKey||event.ctrlKey||event.metaKey||!['Enter',' '].includes(event.key))return;
    event.preventDefault();if(!event.repeat){releaseKey=event.key;begin(button,'keyboard');}
  },{signal,capture:true});
  document.addEventListener('keyup',event=>{
    if(event.key!==releaseKey)return;
    event.preventDefault();event.stopImmediatePropagation();releaseKey=undefined;
    const held=press;cancel();
    if(held?.source==='keyboard'&&!held.opened&&held.button.isConnected)activate(held.button);
  },{signal,capture:true});
  globalThis.addEventListener('blur',()=>{cancel();releaseKey=undefined;},{signal});
  signal.addEventListener('abort',()=>{cancel();releaseKey=undefined;},{once:true});
}
