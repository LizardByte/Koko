/** Loads saved connection editors and binds source-specific channel pages and guide groups. */
import * as api from '../liveTvApi';
import { state } from './state';
import { confirmAction } from './confirmationDialog';

type Run = (operation:()=>Promise<unknown>,button?:HTMLButtonElement)=>Promise<void>;

function setField(form: HTMLFormElement, name: string, value: string): void {
  const control = form.elements.namedItem(name) as HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement | null;
  if (!control) return;
  if (control.tagName==='INPUT' && (control as HTMLInputElement).type==='checkbox') {(control as HTMLInputElement).checked=value==='true';return;}
  if (control.tagName === 'SELECT' && !Array.from((control as HTMLSelectElement).options).some(o=>o.value === value)) {
    const option = document.createElement('option'); option.value=value; option.textContent=value; control.append(option);
  }
  if (control.value !== value) control.value=value;
}

/** Reapply saved values and pending drafts after a Settings DOM patch. */
export function hydrateTvConnections(): void {
  document.querySelectorAll<HTMLFormElement>('[data-tv-replace-source]').forEach(form=>{
    const id = Number(form.dataset.tvReplaceSource), connection = state.liveTv?.connections?.[id];
    if (!connection) return;
    const source = connection.source, config = source.config;
    const fields:Record<string,string> = {name:source.name,capacity:String(source.capacity)};
    for (const [name,value] of Object.entries(config)) if (['string','number','boolean'].includes(typeof value)) fields[name]=String(value);
    fields.multiplex_urls = config.multiplex_urls?.join('\n') ?? '';
    if (config.zap2it?.lineup_id) {
      if (config.guide_kind === 'gracenote') fields.guide_kind = 'zap2it';
      fields.guide_country=config.zap2it.country;
      fields.guide_postal=config.zap2it.postal_code; fields.guide_days=String(config.zap2it.days ?? 14);
      fields.guide_provider=JSON.stringify(config.zap2it);
    }
    if (!fields.guide_kind) fields.guide_kind='automatic';
    if (!fields.iptv_mode) fields.iptv_mode='playlist';
    if (!Number(fields.bridge_capacity)) fields.bridge_capacity='1';
    if (!fields.artwork_provider) fields.artwork_provider='automatic';
    Object.assign(fields,state.liveTv?.connectionDrafts?.[id]);
    for (const [name,value] of Object.entries(fields)) setField(form,name,value);
    const provider = form.elements.namedItem('guide_provider') as HTMLSelectElement | null;
    if (provider?.selectedOptions[0] && config.zap2it) provider.selectedOptions[0].textContent=`Saved lineup · ${config.zap2it.lineup_id}`;
  });
}

/** Capture connection form values without discarding inactive guide fields or partial edits. */
function rememberDraft(form: HTMLFormElement): void {
  const data=state.liveTv; if (!data) return;
  const fields:Record<string,string>={};
  form.querySelectorAll<HTMLInputElement|HTMLSelectElement|HTMLTextAreaElement>('[name]').forEach(control=>{fields[control.name]=control.tagName==='INPUT'&&(control as HTMLInputElement).type==='checkbox'?String((control as HTMLInputElement).checked):control.value;});
  data.connectionDrafts ??= {}; data.connectionDrafts[Number(form.dataset.tvReplaceSource)]=fields;
}

function providerKey(form:HTMLFormElement):string {return form.dataset.tvProviderSource+':'+form.dataset.tvProviderForm;}
function providerDraft(form:HTMLFormElement):{name:string;enabled:boolean;icon_channel:string} {
  const input=new FormData(form);return {name:String(input.get('name')??''),enabled:input.has('enabled'),icon_channel:String(input.get('icon_channel')??'default')};
}

/** Server-selected icons and unsaved provider edits survive DOM patches and image uploads. */
export function hydrateTvProviders():void {
  document.querySelectorAll<HTMLFormElement>('[data-tv-provider-form]').forEach(form=>{
    const data=state.liveTv,provider=data?.sources.find(s=>s.id===Number(form.dataset.tvProviderSource))?.providers?.find(p=>p.id===form.dataset.tvProviderForm);
    if(!provider){return;}
    let choice=provider.has_custom_icon?'upload':'default';if(provider.icon_channel_id) choice='channel:'+provider.icon_channel_id;
    const fields={name:provider.name,enabled:provider.enabled,icon_channel:choice,...data?.providerDrafts?.[providerKey(form)]};
    for(const [name,value] of Object.entries(fields)) setField(form,name,String(value));
  });
}

/** Bind durable grouping and bounded source editors without exposing a stored PIN. */
export function bindTvSourceSettings(signal: AbortSignal, context:{render:()=>void}, run:Run): void {
  hydrateTvProviders();
  document.querySelectorAll<HTMLDetailsElement>('details[data-tv-source-card]').forEach(details=>details.addEventListener('toggle',()=>{
    const data=state.liveTv;if(!data) return;
    const id=Number(details.dataset.tvSourceCard),open=new Set(data.sourceCardsOpen ?? []);
    if(open.has(id)===details.open) return;
    if(details.open) open.add(id);else open.delete(id);
    data.sourceCardsOpen=[...open];
  },{signal}));
  document.querySelectorAll<HTMLButtonElement>('[data-tv-source-remove]').forEach(button=>button.addEventListener('click',async()=>{
    const id=Number(button.dataset.tvSourceRemove),source=state.liveTv?.sources.find(s=>s.id===id);
    if(!source) return;
    const owner=state.liveTv?.owner;
    const accepted=await confirmAction({title:`Remove ${source.name}?`,message:'This removes its channels, guide, future recording schedules, and series rules. Saved recordings and their files will be kept.',confirmLabel:'Remove source'});
    if(!accepted||state.liveTv?.owner!==owner||state.route.page!=='settings'||state.route.section!=='live-tv') return;
    void run(async()=>{
      await api.removeTvSource(id);
      const data=state.liveTv;if(!data) return;
      delete data.connections?.[id];delete data.connectionDrafts?.[id];
      data.sourceCardsOpen=data.sourceCardsOpen?.filter(source=>source!==id);
      if(data.connectionOpen===id) data.connectionOpen=undefined;
      if(data.mappingSource===id) {data.mappingSource=undefined;data.mappingProvider='';}
      data.providerEditor=undefined;
      for(const key of Object.keys(data.providerDrafts ?? {})) if(key.startsWith(id+':')) delete data.providerDrafts![key];
      data.message=`${source.name} was removed. Saved recordings were kept.`;
    },button);
  },{signal}));
  document.querySelectorAll<HTMLSelectElement>('select[name="iptv_mode"]').forEach(select=>{
    const sync=()=>{
      const form=select.form;if(!form) {return;}
      const fast=select.value==='fastchannels',label=form.querySelector('[data-tv-iptv-address-label]'),input=form.elements.namedItem('url') as HTMLInputElement;
      if(label) label.textContent=fast?'FastChannels server address':'IPTV playlist URL';
      input.placeholder=fast?'http://fastchannels.example:5523':'https://provider.example/channels.m3u';
      form.querySelector<HTMLElement>('[data-tv-iptv-server-help]')?.toggleAttribute('hidden',!fast);
      const bridge=form.querySelector<HTMLFieldSetElement>('[data-tv-iptv-bridge]');if(bridge){bridge.hidden=!fast;bridge.disabled=!fast;}
    };
    sync();select.addEventListener('change',()=>{const group=select.form?.elements.namedItem('provider_grouping') as HTMLInputElement|null;if(select.value==='fastchannels'&&group) {group.checked=true;}sync();if(select.form?.dataset.tvReplaceSource) rememberDraft(select.form);},{signal});
  });
  document.querySelectorAll<HTMLDetailsElement>('[data-tv-provider-editor]').forEach(details=>details.addEventListener('toggle',()=>{
    const data=state.liveTv;if(!data) {return;}const key=details.dataset.tvProviderEditor;
    if(details.open&&data.providerEditor!==key){data.providerEditor=key;context.render();}
    else if(!details.open&&data.providerEditor===key){data.providerEditor=undefined;context.render();}
  },{signal}));
  document.querySelectorAll<HTMLFormElement>('[data-tv-provider-form]').forEach(form=>{
    form.addEventListener('input',()=>{if(state.liveTv){state.liveTv.providerDrafts??={};state.liveTv.providerDrafts[providerKey(form)]=providerDraft(form);}},{signal});
    form.addEventListener('submit',event=>{
    event.preventDefault();const input=new FormData(form),choice=String(input.get('icon_channel') ?? 'default');
    void run(async()=>{
      await api.setTvProvider(Number(form.dataset.tvProviderSource),form.dataset.tvProviderForm!,{name:String(input.get('name')??''),enabled:input.has('enabled'),icon_channel_id:choice.startsWith('channel:')?Number(choice.slice(8)):null,remove_icon:choice==='default'});
      if(state.liveTv?.providerDrafts) delete state.liveTv.providerDrafts[providerKey(form)];
    });
  },{signal});});
  document.querySelectorAll<HTMLInputElement>('[data-tv-icon-source]').forEach(input=>input.addEventListener('change',()=>{
    const file=input.files?.[0];if(file) void run(async()=>{
      await api.setTvSourceImage(Number(input.dataset.tvIconSource),input.dataset.tvIconProvider,file);
      if(input.form?.dataset.tvProviderForm&&state.liveTv){
        state.liveTv.providerDrafts??={};state.liveTv.providerDrafts[providerKey(input.form)]={...providerDraft(input.form),icon_channel:'upload'};
      }
    });
  },{signal}));
  document.querySelectorAll<HTMLButtonElement>('[data-tv-icon-reset]').forEach(button=>button.addEventListener('click',()=>{void run(()=>api.setTvSourceImage(Number(button.dataset.tvIconReset),undefined));},{signal}));
  document.querySelectorAll<HTMLButtonElement>('[data-tv-provider-channels]').forEach(button=>button.addEventListener('click',()=>{
    if(!state.liveTv){return;}state.liveTv.mappingSource=Number(button.dataset.tvProviderSource);state.liveTv.mappingProvider=button.dataset.tvProviderChannels;state.liveTv.mappingQuery='';state.liveTv.mappingPage=0;context.render();
    document.querySelector(`[data-tv-mapping-source="${state.liveTv.mappingSource}"]`)?.scrollIntoView({block:'start'});
  },{signal}));
  document.querySelectorAll<HTMLDetailsElement>('[data-tv-connection-source]').forEach(details=>details.addEventListener('toggle',()=>{
    const data=state.liveTv; if (!data) return;
    const id=Number(details.dataset.tvConnectionSource);
    if (!details.open) {
      if (data.connectionOpen === id) {data.connectionOpen=undefined;}
      return;
    }
    if (data.connectionOpen === id && (data.connections?.[id] || data.connectionLoading === id)) return;
    data.connectionOpen=id;
    if (data.connections?.[id]) {context.render();return;}
    data.connectionLoading=id; context.render();
    const owner=data.owner;
    void api.getTvSourceConnection(id).then(connection=>{
      if (state.liveTv?.owner !== owner || state.route.page !== 'settings' || state.route.section !== 'live-tv') return;
      state.liveTv.connections ??= {}; state.liveTv.connections[id]=connection; state.liveTv.connectionLoading=undefined; context.render();
    }).catch((error:unknown)=>{
      if (state.liveTv?.owner === owner) {state.liveTv.connectionLoading=undefined;state.liveTv.error=error instanceof Error ? error.message : 'Unable to load source settings';context.render();}
    });
  },{signal}));
  document.querySelectorAll<HTMLFormElement>('[data-tv-replace-source]').forEach(form=>{
    form.addEventListener('input',()=>rememberDraft(form),{signal});
    form.addEventListener('change',()=>rememberDraft(form),{signal});
  });
  document.querySelectorAll<HTMLDetailsElement>('[data-tv-mapping-source]').forEach(details=>details.addEventListener('toggle',()=>{
    const data=state.liveTv; if (!data) return;
    const id=Number(details.dataset.tvMappingSource);
    if (details.open && data.mappingSource !== id) {data.mappingSource=id;data.mappingProvider='';data.mappingQuery='';data.mappingPage=0;context.render();}
    else if (!details.open && data.mappingSource === id) {data.mappingSource=undefined;context.render();}
  },{signal}));
  document.querySelectorAll<HTMLFormElement>('[data-tv-mapping-search]').forEach(form=>form.addEventListener('submit',event=>{
    event.preventDefault();if (!state.liveTv) return;
    const input=new FormData(form);state.liveTv.mappingQuery=String(input.get('query') ?? '').trim();state.liveTv.mappingProvider=String(input.get('provider') ?? '');state.liveTv.mappingPage=0;context.render();
  },{signal}));
  document.querySelectorAll<HTMLElement>('[data-tv-mapping-page]').forEach(button=>button.addEventListener('click',()=>{
    if (state.liveTv) {state.liveTv.mappingPage=Number(button.dataset.tvMappingPage);context.render();}
  },{signal}));
  document.querySelectorAll<HTMLFormElement>('[data-tv-group-form]').forEach(form=>form.addEventListener('submit',event=>{
    event.preventDefault();
    void run(async()=>{
      const input=new FormData(form), id=form.dataset.tvGroupForm || crypto.randomUUID();
      const group={id,name:String(input.get('name') ?? '').trim(),source_ids:input.getAll('source_id').map(Number)};
      if (new Set(group.source_ids).size < 2) throw new Error('Choose at least two sources for this group.');
      const groups=[...(state.liveTv?.sourceGroups ?? [])], index=groups.findIndex(g=>g.id === id);
      if (index < 0) groups.push(group); else groups[index]=group;
      await api.setTvSourceGroups(groups);
    });
  },{signal}));
  document.querySelectorAll<HTMLElement>('[data-tv-group-delete]').forEach(button=>button.addEventListener('click',()=>{
    void run(()=>api.setTvSourceGroups((state.liveTv?.sourceGroups ?? []).filter(g=>g.id !== button.dataset.tvGroupDelete)));
  },{signal}));
}
