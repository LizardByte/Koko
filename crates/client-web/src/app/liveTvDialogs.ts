/** Accessible Live TV action and recording dialogs that preserve drafts across guide refreshes. */
import { createIcons, icons } from 'lucide';
import { scheduleTvRecording, setTvFavorite, type TvChannel, type TvProgram } from '../liveTvApi';
import { escapeHtml as escape } from './format';
import { channelPrograms, tvTime } from './liveTvGuideModel';
import { state } from './state';
import { renderButtonContent, renderIcon } from './ui';

interface Actions {play:(id:number)=>void;changed:()=>void}
interface OpenDialog {dialog:HTMLDialogElement;owner:number;close:()=>void}
let active:OpenDialog|undefined;

/** Close account-scoped options when navigation or sign-in changes, without losing refresh drafts. */
export function syncTvDialogs():void {
  if(active&&(state.route.page!=='live-tv'||state.bootstrap?.current_user?.id!==active.owner))active.close();
}

function selection(channelId:number,programId?:number):{channel:TvChannel;program?:TvProgram}|undefined {
  const data=state.liveTv;
  if(!data||data.owner!==state.bootstrap?.current_user?.id||state.route.page!=='live-tv')return;
  const channel=data.channels.find(c=>c.id===channelId);if(!channel)return;
  const now=Date.now()/1000,program=programId?data.programs.find(p=>p.id===programId&&p.source_id===channel.source_id):channelPrograms(channel,data.programs).find(p=>p.starts_at<=now&&p.ends_at>now);
  return {channel,program};
}

function openDialog(title:string,channel:TvChannel,program:TvProgram|undefined,content:string):OpenDialog|undefined {
  const owner=state.bootstrap?.current_user?.id;if(owner==null)return;
  active?.close();
  const previous=document.activeElement as HTMLElement|null;
  const dialog=document.createElement('dialog');dialog.className='tv-program-dialog';dialog.id='tv-program-dialog';
  dialog.setAttribute('aria-labelledby','tv-dialog-title');
  const details=program?`<h3>${escape(program.title)}</h3><p>${escape(tvTime(program.starts_at,true))} – ${escape(tvTime(program.ends_at))}</p>`:'';
  dialog.innerHTML=`<header><div><p class="eyebrow">${escape(channel.number)} · ${escape(channel.name)}</p><h2 id="tv-dialog-title">${escape(title)}</h2></div><button type="button" class="secondary-button" data-tv-dialog-close aria-label="Close ${escape(title.toLowerCase())}">${renderIcon('x')}</button></header><div class="tv-dialog-program">${details}</div><p class="live-tv-error" role="alert" hidden></p>${content}`;
  const events=new AbortController();let closed=false;
  const close=()=>{
    if(closed){return;}closed=true;events.abort();dialog.close();dialog.remove();
    if(active?.dialog===dialog)active=undefined;
    if(state.route.page!=='live-tv'||state.bootstrap?.current_user?.id!==owner)return;
    const replacement=previous?.id?document.getElementById(previous.id):null;
    const restore=previous?.isConnected?previous:replacement;
    (restore??document.querySelector<HTMLElement>('#tv-guide-scroll'))?.focus({preventScroll:true});
  };
  dialog.addEventListener('cancel',event=>{event.preventDefault();close();},{signal:events.signal});
  dialog.addEventListener('close',close,{signal:events.signal});
  dialog.addEventListener('click',event=>{
    if((event.target as Element).closest('[data-tv-dialog-close]')){close();return;}
    const box=dialog.getBoundingClientRect();
    if(event.target===dialog&&(event.clientX<box.left||event.clientX>box.right||event.clientY<box.top||event.clientY>box.bottom))close();
  },{signal:events.signal});
  globalThis.addEventListener('popstate',close,{signal:events.signal});
  active={dialog,owner,close};document.body.append(dialog);createIcons({icons,root:dialog});dialog.showModal();
  dialog.querySelector<HTMLButtonElement>('[data-tv-dialog-close]')?.focus();
  return active;
}

async function perform(open:OpenDialog,operation:()=>Promise<unknown>,changed:()=>void):Promise<void> {
  if(open.dialog.dataset.busy||state.bootstrap?.current_user?.id!==open.owner)return;
  open.dialog.dataset.busy='true';
  const buttons=open.dialog.querySelectorAll<HTMLButtonElement>('button:not([data-tv-dialog-close])');buttons.forEach(b=>{b.disabled=true;});
  try{await operation();open.close();changed();}
  catch(error){const notice=open.dialog.querySelector<HTMLElement>('[role="alert"]')!;notice.textContent=error instanceof Error?error.message:'The TV operation failed.';notice.hidden=false;}
  finally{delete open.dialog.dataset.busy;buttons.forEach(b=>{b.disabled=false;});}
}

/** Offer padding, series rules, and shared/private ownership on an opaque modal surface. */
export function openTvRecordingOptions(channelId:number,programId:number,changed:()=>void):void {
  const selected=selection(channelId,programId),program=selected?.program;
  if(!selected||!program||program.ends_at<=Date.now()/1000||state.liveTv?.canRecord===false)return;
  const open=openDialog('Record program or series',selected.channel,program,`<form class="tv-record-form"><div class="tv-record-padding"><label>Before (minutes)<input name="before" type="number" min="0" max="60" value="1" required/></label><label>After (minutes)<input name="after" type="number" min="0" max="60" value="2" required/></label></div><label class="tv-dialog-checkbox"><input type="checkbox" name="new-only"/> New episodes only (series)</label><label class="tv-dialog-checkbox"><input type="checkbox" name="private"/> Private (only my account)</label><p class="muted">Recordings are shared with all accounts unless you choose Private.</p><footer><button type="button" class="secondary-button" data-tv-dialog-close>Cancel</button><button type="submit" name="mode" value="single">Record program</button><button type="submit" name="mode" value="series" class="secondary-button">Record series</button></footer></form>`);
  if(!open)return;
  const form=open.dialog.querySelector<HTMLFormElement>('form')!;
  form.addEventListener('submit',event=>{
    event.preventDefault();if(!form.reportValidity())return;
    const input=new FormData(form),series=(event.submitter as HTMLButtonElement|null)?.value==='series';
    void perform(open,()=>scheduleTvRecording({channel_id:channelId,title:program.title,starts_at:program.starts_at,ends_at:program.ends_at,series,new_only:input.has('new-only'),private:input.has('private'),padding_before:Number(input.get('before'))*60,padding_after:Number(input.get('after'))*60}),changed);
  });
}

/** Hold or right-click a guide cell to access recording and favorite actions without tuning. */
export function openTvGuideOptions(channelId:number,programId:number|undefined,actions:Actions):void {
  const selected=selection(channelId,programId);if(!selected)return;
  const {channel,program}=selected,favorite=state.liveTv?.preferences?.favorites.includes(channelId)===true;
  const record=program&&program.ends_at>Date.now()/1000&&state.liveTv?.canRecord!==false?`<button type="button" class="secondary-button" data-tv-option-record>${renderButtonContent('Record program or series','circle')}</button>`:'';
  const open=openDialog('Channel options',channel,program,`<div class="tv-dialog-actions"><button type="button" data-tv-option-watch>${renderButtonContent('Watch channel live','play')}</button>${record}<button type="button" class="secondary-button" data-tv-option-favorite>${renderButtonContent(favorite?'Remove from favorites':'Add to favorites','star')}</button></div>`);
  if(!open)return;
  open.dialog.querySelector('[data-tv-option-watch]')?.addEventListener('click',()=>{open.close();actions.play(channelId);});
  open.dialog.querySelector('[data-tv-option-record]')?.addEventListener('click',()=>{open.close();if(program)openTvRecordingOptions(channelId,program.id,actions.changed);});
  open.dialog.querySelector('[data-tv-option-favorite]')?.addEventListener('click',()=>{void perform(open,()=>setTvFavorite(channelId,!favorite),actions.changed);});
}
