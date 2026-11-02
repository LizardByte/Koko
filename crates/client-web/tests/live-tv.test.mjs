/** Tests account isolation, station mapping, safe metadata rendering, and guide request races. */
import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';
import { JSDOM } from 'jsdom';

globalThis.location = { pathname: '/live-tv', origin: 'https://koko.local' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { channelPrograms, renderLiveTvPage, tvImage } = await server.ssrLoadModule('/src/app/liveTv.ts');
const { loadLiveTv,sourceConfig } = await server.ssrLoadModule('/src/app/liveTvActions.ts');
const { renderLiveTvSettings } = await server.ssrLoadModule('/src/app/liveTvSetup.ts');
const { renderTvGuideMapping,bindTvGuideMappings } = await server.ssrLoadModule('/src/app/liveTvMapping.ts');
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const { parseRoute } = await server.ssrLoadModule('/src/app/routes.ts');
const { programPosition, programBadges, matchesTvCategory, visibleGuideChannels, guideSources } = await server.ssrLoadModule('/src/app/liveTvGuideModel.ts');
const { renderPlayerOverlay } = await server.ssrLoadModule('/src/app/playbackController.ts');
const { TvPlaybackWatchdog } = await server.ssrLoadModule('/src/app/tvPlaybackRecovery.ts');
const { currentGuideWindow,programAtTime } = await server.ssrLoadModule('/src/app/tvGuideTime.ts');
const {tvGuideRowRange,tvGuideViewKey} = await server.ssrLoadModule('/src/app/tvGuideViewport.ts');
const {guideViews,activeGuideView} = await server.ssrLoadModule('/src/app/liveTvGuideModel.ts');
const {hydrateTvConnections,hydrateTvProviders} = await server.ssrLoadModule('/src/app/liveTvSourceActions.ts');
const {renderTvSourceIcon,recoverTvSourceImage} = await server.ssrLoadModule('/src/app/liveTvIcons.ts');
const {renderTvPlayerProgram} = await server.ssrLoadModule('/src/app/tvPlayer.ts');
const now = Math.floor(Date.now() / 1000);
const channel = { id: 1, source_id: 1, source_key: 'source', name: 'Test TV', number: '7.1', group_name: 'News', guide_key: 'station', enabled: true, available: true };

test('source artwork falls back from an uploaded icon to its application logo and then Lucide',()=>{
  const dom=new JSDOM(renderTvSourceIcon({kind:'nextpvr',image_url:'https://example.com/upload.png'}));
  const image=dom.window.document.querySelector('img');
  assert.equal(image.src,'https://example.com/upload.png');
  recoverTvSourceImage(image);
  assert.match(image.src,/\/assets\/providers\/nextpvr\.ico$/);
  assert.equal(image.hidden,false);
  recoverTvSourceImage(image);
  assert.equal(image.hidden,true);
  assert.ok(dom.window.document.querySelector('.tv-icon-fallback [data-lucide="database-zap"]'));
  assert.doesNotMatch(renderTvSourceIcon({kind:'m3u'}),/<img/);
  assert.match(renderTvSourceIcon({kind:'m3u',iptv_mode:'fastchannels'}),/\/assets\/providers\/fastchannels\.svg/);
  dom.window.close();
});
const program = { id: 1, source_id: 1, guide_key: 'station', title: 'News', starts_at: now - 30, ends_at: now + 3600, metadata: { description: 'Evening news', categories: ['News'], credits: ['Presenter'] } };
function data(extra = {}) {
  return { owner: 1, channels: [channel], programs: [program], stations: [], recordings: [], rules: [], sources: [], conflicts: [], windowStart: now, tab: 'guide', loading: false, ...extra };
}
function app(extra = {}) { return { bootstrap: { current_user: { id: 1, admin: false } }, route: { page: 'live-tv' }, liveTv: data(), ...extra }; }
function api(extra = {}) {
  return { getTvChannels: async () => [channel], getTvGuide: async () => ({ programs: [program], stations: [] }), getTvRecordings: async () => ({ recordings: [], conflicts: [] }), getTvRules: async () => [], getTvSources: async () => [], ...extra };
}

test('live television has its own persistent route', () => { assert.deepEqual(parseRoute(), { page: 'live-tv' }); });

test('provider views keep categories and favorites scoped to the selected provider',()=>{
  const first={...channel,provider_id:'espn'},second={...channel,id:2,provider_id:'sling'};
  const providers=[{source_id:1,id:'espn',name:'ESPN',enabled:true,channel_count:1},{source_id:1,id:'sling',name:'Sling',enabled:true,channel_count:1}];
  const model=data({channels:[first,second],providers,preferences:{favorites:[1,2],recent:[2,1]}});
  assert.deepEqual(visibleGuideChannels(model).map(c=>c.id),[1,2]);
  const original=tvGuideViewKey(model);model.providerView='1:espn';assert.notEqual(tvGuideViewKey(model),original);
  assert.deepEqual(visibleGuideChannels(model).map(c=>c.id),[1]);
  model.providerView='1:sling';model.channelView='favorites';assert.deepEqual(visibleGuideChannels(model).map(c=>c.id),[2]);
  model.sourceId=2;model.channels.push({...second,id:3,source_id:2});assert.deepEqual(visibleGuideChannels(model).map(c=>c.id),[]);
});

test('IPTV provider editing offers channel icon references only from that provider',()=>{
  state.bootstrap={current_user:{id:1,admin:true}};
  const providers=[{source_id:1,id:'espn',name:'ESPN',enabled:true,channel_count:1,icon_channel_id:1,has_custom_icon:false}];
  state.liveTv=data({providerEditor:'1:espn',sources:[{id:1,kind:'m3u',name:'IPTV',capacity:3,enabled:true,providers}],channels:[{...channel,provider_id:'espn',logo:'http://logos.local/espn.png'},{...channel,id:2,provider_id:'sling',logo:'http://logos.local/sling.png'}]});
  const dom=new JSDOM(renderLiveTvSettings());const select=dom.window.document.querySelector('[name="icon_channel"]');
  assert.equal(select.value,'channel:1');assert.ok(select.querySelector('option[value="channel:1"]'));assert.equal(select.querySelector('option[value="channel:2"]'),null);
  assert.ok(dom.window.document.querySelector('[aria-label="Upload icon for ESPN"]'));dom.window.close();
});

test('IPTV connection edits retain their preset and can explicitly disable provider grouping',()=>{
  state.bootstrap={current_user:{id:1,admin:true}};
  const config={url:'http://fastchannels.local:5523',pin:'',xmltv_url:'',guide_kind:'automatic',iptv_mode:'fastchannels',provider_grouping:true,bridge_capacity:2};
  state.liveTv=data({sources:[{id:1,name:'FastChannels',kind:'m3u',enabled:true,capacity:10}],connectionOpen:1,connections:{1:{source:{name:'FastChannels',kind:'m3u',capacity:10,config},has_pin:false}}});
  const previous=globalThis.document,dom=new JSDOM(renderLiveTvSettings());globalThis.document=dom.window.document;
  try {
    hydrateTvConnections();const form=document.querySelector('[data-tv-replace-source="1"]');
    assert.equal(form.elements.namedItem('iptv_mode').value,'fastchannels');assert.equal(form.elements.namedItem('provider_grouping').checked,true);assert.equal(form.elements.namedItem('bridge_capacity').value,'2');
    const input=new FormData();input.set('url',config.url);input.set('iptv_mode','fastchannels');input.set('provider_grouping_present','true');
    assert.equal(sourceConfig(input,config).provider_grouping,false);
    state.liveTv.connectionDrafts={1:{provider_grouping:'false'}};hydrateTvConnections();assert.equal(form.elements.namedItem('provider_grouping').checked,false);
  }finally{globalThis.document=previous;dom.window.close();}
});

test('an uploaded provider icon stays selected across DOM patches without losing pending edits',()=>{
  state.bootstrap={current_user:{id:1,admin:true}};
  const provider={source_id:1,id:'espn',name:'ESPN',enabled:true,channel_count:1,has_custom_icon:true,image_url:'/api/v1/live-tv/sources/1/image?v=fixture&provider=espn'};
  state.liveTv=data({providerEditor:'1:espn',sources:[{id:1,name:'IPTV',kind:'m3u',enabled:true,capacity:1,providers:[provider]}]});
  const previous=globalThis.document,dom=new JSDOM(renderLiveTvSettings());globalThis.document=dom.window.document;
  try {
    const form=document.querySelector('[data-tv-provider-form]');form.elements.namedItem('icon_channel').value='default';
    hydrateTvProviders();assert.equal(form.elements.namedItem('icon_channel').value,'upload');
    state.liveTv.providerDrafts={'1:espn':{name:'My ESPN',enabled:false,icon_channel:'upload'}};hydrateTvProviders();
    assert.equal(form.elements.namedItem('name').value,'My ESPN');assert.equal(form.elements.namedItem('enabled').checked,false);
  }finally{globalThis.document=previous;dom.window.close();}
});

test('the guide clips broadcasts to a shared timeline without closing schedule gaps', () => {
  const clipped = programPosition({...program,starts_at:now-900,ends_at:now+900},now);
  assert.equal(clipped.left,0); assert.ok(Math.abs(clipped.width-900/21600*100)<0.001);
  const later = programPosition({...program,starts_at:now+7200,ends_at:now+10800},now);
  assert.ok(Math.abs(later.left-100/3)<0.001); assert.ok(Math.abs(later.width-100/6)<0.001);
  assert.equal(programPosition({...program,starts_at:now-900,ends_at:now},now),undefined);
});

test('live broadcasts, premieres, and new episodes retain separate flags and genre filters', () => {
  assert.deepEqual(programBadges({is_new:true,is_live:false}),['NEW']);
  assert.deepEqual(programBadges({is_live:true,is_premiere:true,is_finale:true}),['LIVE','PREMIERE','FINALE']);
  assert.equal(matchesTvCategory({...program,metadata:{categories:['Sports event']}},'Sports'),true);
  assert.equal(matchesTvCategory({...program,metadata:{categories:['Movie / drama']}},'Movies'),true);
  assert.equal(matchesTvCategory({...program,metadata:{categories:['News / Current affairs']}},'News'),true);
  assert.equal(matchesTvCategory({...program,metadata:{}},'Sports'),false);
});

test('source navigation and personal channel views retain source identity', () => {
  const other = {...channel,id:2,source_id:2,name:'Other source'};
  const model = data({channels:[channel,other],guideSources:[{id:1,name:'Antenna'},{id:2,name:'Cable'}],preferences:{favorites:[2],recent:[2,1]}});
  assert.deepEqual(guideSources(model).map(s=>s.name),['Antenna','Cable']);
  assert.deepEqual(visibleGuideChannels(model).map(c=>c.id),[1]);
  model.sourceId=2; model.channelView='favorites';
  assert.deepEqual(visibleGuideChannels(model).map(c=>c.id),[2]);
  model.sourceId=1; assert.deepEqual(visibleGuideChannels(model),[]);
});

test('named guide groups combine sources while retaining source-scoped listings and personal filters',()=>{
  const other={...channel,id:2,source_id:2,name:'Bridge channel'};
  const model=data({channels:[channel,other],programs:[program,{...program,id:2,source_id:2,title:'Bridge show'}],sourceGroups:[{id:'combined',name:'FastChannels',source_ids:[1,2]}],preferences:{favorites:[2],recent:[2,1]}});
  assert.deepEqual(guideViews(model).map(v=>({name:v.name,count:v.count})),[{name:'FastChannels',count:2}]);
  assert.equal(activeGuideView(model).groupId,'combined');
  assert.deepEqual(visibleGuideChannels(model).map(c=>c.id),[1,2]);
  assert.equal(channelPrograms(other,model.programs)[0].title,'Bridge show');
  model.channelView='favorites';assert.deepEqual(visibleGuideChannels(model).map(c=>c.id),[2]);
  model.channelView='recent';assert.deepEqual(visibleGuideChannels(model).map(c=>c.id),[2,1]);
});

test('a 4402-channel guide indexes schedules once and mounts only a nearby row window',()=>{
  const channels=Array.from({length:4402},(_,i)=>({...channel,id:i+1,guide_key:`station-${i}`}));
  const rows=channels.flatMap(c=>Array.from({length:12},(_,slot)=>({...program,id:c.id*20+slot,guide_key:c.guide_key,title:`Channel ${c.id} show ${slot}`,starts_at:now+slot*1800,ends_at:now+(slot+1)*1800})));
  let scans=0;
  const indexed=new Proxy(rows,{get(target,key,receiver){if(key===Symbol.iterator){scans++;}return Reflect.get(target,key,receiver);}});
  state.bootstrap={current_user:{id:1,admin:true}};
  state.liveTv=data({channels,programs:indexed});
  let html=renderLiveTvPage();
  assert.equal(scans,1);
  assert.ok((html.match(/class="tv-guide-row"/g) ?? []).length<30);
  assert.match(html,/aria-rowcount="4403"/);
  assert.ok(html.length<300_000);
  state.liveTv.guideViewport={key:tvGuideViewKey(state.liveTv),top:2200*88,height:580};
  state.liveTv.selectedChannelId=2201;state.liveTv.selectedProgramId=2201*20;
  html=renderLiveTvPage();
  assert.equal(scans,1);assert.match(html,/id="tv-channel-row-2201"/);
  assert.doesNotMatch(html,/id="tv-channel-row-1"/);
  const range=tvGuideRowRange(4402,4402*88,580);
  assert.equal(range.end,4402);assert.ok(range.end-range.start<30);
});

test('source cards show counts and sizes and render only the opened source channel page',()=>{
  state.bootstrap={current_user:{id:1,admin:true}};
  state.liveTv=data({sources:[{id:1,name:'Fast',kind:'m3u',enabled:true,capacity:10,channel_count:4402,playlist_bytes:2*1024*1024,guide_bytes:150*1024*1024},{id:2,name:'Bridge',kind:'m3u',enabled:true,capacity:1}],channels:Array.from({length:4402},(_,i)=>({...channel,id:i+1,name:`Channel ${i+1}`})).concat([{...channel,id:5000,source_id:2,name:'Bridge only'}])});
  let dom=new JSDOM(renderLiveTvSettings());
  assert.equal(dom.window.document.querySelectorAll('[data-tv-channel-update]').length,0);
  assert.equal(dom.window.document.querySelector('[data-tv-source-card="1"]').open,false);
  assert.match(dom.window.document.querySelector('[data-tv-source-card="1"]').textContent,/4402 channels/);
  assert.match(dom.window.document.querySelector('[data-tv-source-card="1"]').textContent,/M3U.*MB.*XMLTV.*MB/);
  dom.window.close();state.liveTv.mappingSource=1;state.liveTv.mappingPage=1;
  dom=new JSDOM(renderLiveTvSettings());
  assert.equal(dom.window.document.querySelectorAll('[data-tv-channel-update]').length,50);
  assert.ok(dom.window.document.querySelector('[data-tv-source-card="1"] [data-tv-channel-update="51"]'));
  assert.equal(dom.window.document.querySelector('[data-tv-channel-update="5000"]'),null);
  dom.window.close();
});

test('connection editors prefill saved values and preserve masked PINs and unfinished drafts',()=>{
  const config={url:'http://tv.local:8866/',pin:'',xmltv_url:'http://guide.local/listings.xml',guide_kind:'xmltv',artwork_provider:'guide',refresh_hours:12,zap2it:{country:'USA',postal_code:'14850',lineup_id:'saved-lineup',headend_id:'saved',device:'X',timezone:'',days:7}};
  state.bootstrap={current_user:{id:1,admin:true}};
  state.liveTv=data({sources:[{id:1,name:'NextPVR',kind:'nextpvr',enabled:true,capacity:3}],connectionOpen:1,connections:{1:{source:{name:'NextPVR',kind:'nextpvr',capacity:3,config},has_pin:true}}});
  const previous=globalThis.document;
  const dom=new JSDOM(renderLiveTvSettings());globalThis.document=dom.window.document;
  try {
    hydrateTvConnections();const form=document.querySelector('[data-tv-replace-source="1"]');
    assert.equal(form.elements.namedItem('url').value,config.url);
    assert.equal(form.elements.namedItem('xmltv_url').value,config.xmltv_url);
    assert.equal(form.elements.namedItem('guide_kind').value,'xmltv');
    assert.equal(form.elements.namedItem('artwork_provider').value,'guide');
    assert.equal(form.elements.namedItem('refresh_hours').value,'12');
    assert.equal(form.elements.namedItem('pin').required,false);
    assert.equal(form.elements.namedItem('preserve_pin').value,'true');
    state.liveTv.connectionDrafts={1:{url:'http://edited.local/'}};hydrateTvConnections();
    assert.equal(form.elements.namedItem('url').value,'http://edited.local/');
  } finally {globalThis.document=previous;dom.window.close();}
});

test('editing an automatic connection preserves its saved XMLTV URL and inactive lineup selection',()=>{
  const original={url:'http://tv.local/',pin:'',xmltv_url:'http://guide.local/listings.xml',guide_kind:'automatic',artwork_provider:'guide',zap2it:{country:'USA',postal_code:'14850',lineup_id:'saved-lineup',headend_id:'saved',device:'X',timezone:'',days:7}};
  const form=new FormData();for(const [name,value] of Object.entries({url:original.url,guide_kind:'automatic',refresh_hours:'18',preserve_pin:'true'}))form.set(name,value);
  const config=sourceConfig(form,original);
  assert.equal(config.xmltv_url,original.xmltv_url);assert.deepEqual(config.zap2it,original.zap2it);
  assert.equal(config.refresh_hours,18);assert.equal(config.preserve_pin,true);
});

test('Zap2It logos and cover art request larger sizes while other hosts remain unchanged', () => {
  assert.equal(tvImage('https://zpmc.tmsimg.com/h3/NowShowing/45738/logo.png?w=55'),'https://zpmc.tmsimg.com/h3/NowShowing/45738/logo.png?w=320');
  assert.equal(tvImage('https://zpmc.tmsimg.com/assets/cover.jpg?w=165'),'https://zpmc.tmsimg.com/assets/cover.jpg?w=960');
  assert.equal(tvImage('https://example.com/cover.jpg?w=165'),'https://example.com/cover.jpg?w=165');
});

test('TV uses the shared video player in both overlay and hero modes without exposing another account', () => {
  state.bootstrap={current_user:{id:1,admin:true}};
  state.activeTvPlayback={owner:1,kind:'channels',id:5,title:'Live news',logo:'/channel-logo.png',logoFallback:'https://example.com/station.png',url:'/api/v1/live-tv/sessions/test/stream',buffer:{session_id:'test',start_ms:0,end_ms:12000,can_keep:true}};
  state.isPlayerOpen=true;
  let dom=new JSDOM(renderPlayerOverlay());
  assert.ok(dom.window.document.querySelector('.media-player-shell video#media-player'));
  assert.ok(dom.window.document.querySelector('#player-quality'));
  assert.ok(dom.window.document.querySelector('#player-keep-tv'));
  assert.ok(dom.window.document.querySelector('.player-bottom-controls #player-keep-tv'));
  assert.equal(dom.window.document.querySelector('.player-top-controls #player-keep-tv'),null);
  assert.equal(dom.window.document.querySelector('#player-recording-menu').hidden,true);
  assert.equal(dom.window.document.querySelectorAll('#player-recording-menu [role="menuitem"]').length,2);
  assert.equal(dom.window.document.querySelector('.player-channel-logo').getAttribute('src'),'/channel-logo.png');
  assert.equal(dom.window.document.querySelector('.player-channel-logo').dataset.fallbackSrc,'https://example.com/station.png');
  const controls=[...dom.window.document.querySelectorAll('.player-transport-cluster button[aria-label]')].filter(button=>!button.closest('[role="menu"]')).map(button=>button.getAttribute('aria-label'));
  assert.deepEqual(controls,['Keep recording','Back 10 seconds','Pause','Forward 10 seconds','Skip to end']);
  assert.equal(dom.window.document.querySelector('#player-go-live'),null);
  assert.equal(dom.window.document.querySelector('.player-channel-heading h2').textContent,'Live news');
  assert.equal(dom.window.document.querySelector('video').hasAttribute('controls'),false);
  const source=dom.window.document.querySelector('video').getAttribute('src'); dom.window.close();
  state.isPlayerOpen=false; dom=new JSDOM(renderPlayerOverlay());
  assert.ok(dom.window.document.querySelector('.tv-preview-player #media-player'));
  assert.equal(dom.window.document.querySelector('video').getAttribute('src'),source); dom.window.close();
  state.bootstrap.current_user.id=2;
  assert.equal(renderPlayerOverlay(),'');
  state.activeTvPlayback=undefined;
});

test('TV recovery detects frozen clocks, respects pauses, and caps retries across decoder replacements',()=>{
  const watchdog=new TvPlaybackWatchdog();
  const playing={position:1000,paused:false,interrupted:false,unavailable:false};
  watchdog.attach(1000,0);
  assert.equal(watchdog.observe(playing,11999),undefined);
  assert.equal(watchdog.observe(playing,12000),'recover');
  watchdog.attach(1000,12000);
  assert.equal(watchdog.observe({...playing,interrupted:true},13000),undefined);
  assert.equal(watchdog.observe(playing,27000),'recover');
  watchdog.attach(1000,27000);
  assert.equal(watchdog.observe(playing,42000),'recover');
  watchdog.attach(1000,42000);
  assert.equal(watchdog.observe(playing,57000),'exhausted');
  assert.equal(watchdog.observe({...playing,paused:true},90000),undefined);
  assert.equal(watchdog.observe(playing,91000),undefined);
  for(let second=92;second<=124;second+=2) watchdog.observe({...playing,position:1000+(second-91)*1000},second*1000);
  assert.equal(watchdog.attempts,0);
  assert.equal(watchdog.observe({...playing,position:34000,interrupted:true},125000),'recover');
  assert.equal(watchdog.observe({...playing,unavailable:true,interrupted:true},200000),undefined);
});
test('guide mappings require the correct source and honor explicit overrides', () => {
  const rows = [program, { ...program, id: 2, source_id: 2 }, { ...program, id: 3, guide_key: 'override' }];
  assert.deepEqual(channelPrograms(channel, rows).map(p => p.id), [1]);
  assert.deepEqual(channelPrograms({ ...channel, guide_override: 'override' }, rows).map(p => p.id), [3]);
});

test('large guide station lists load only for the focused channel and retain edits on blur', () => {
  const stations=Array.from({length:5200},(_,i)=>({source_id:1,guide_key:`station-${i}`,name:`Station ${i}`,number:String(i),aliases:[]}));
  stations.push({source_id:2,guide_key:'foreign',name:'Other source',number:'1',aliases:[]});
  const channels=Array.from({length:3600},(_,i)=>({...channel,id:i+1,guide_override:i===0?'station-5':undefined}));
  const html=channels.map(c=>renderTvGuideMapping(c,stations)).join('');
  assert.ok(html.length<1_000_000);
  const dom=new JSDOM(html); const doc=dom.window.document;
  const controller=new dom.window.AbortController();
  bindTvGuideMappings(doc,controller.signal,stations);
  assert.equal(doc.querySelectorAll('option').length,3601);
  const [first,second]=doc.querySelectorAll('select');
  assert.equal(first.value,'station-5'); first.focus();
  assert.equal(first.options.length,5201);
  assert.equal(first.querySelector('[value="foreign"]'),null);
  first.value='station-5199'; second.focus();
  assert.equal(first.options.length,2); assert.equal(first.value,'station-5199');
  assert.equal(second.options.length,5201); second.blur();
  assert.equal(second.options.length,1);
  first.focus(); assert.equal(first.value,'station-5199'); first.blur();
  controller.abort(); second.focus(); assert.equal(second.options.length,1);
  dom.window.close();
});
test('broadcast metadata is escaped and active artwork URLs never become image elements', () => {
  state.bootstrap = { current_user: { id: 1, admin: false } };
  state.liveTv = data({ programs: [{ ...program, title: '<script>bad()</script><ScRiPt>bad()</ScRiPt>', metadata: { description: '<img src=x onerror=bad()>', image: 'javascript:bad()', credits: ['<actor>'] } }] });
  const html = renderLiveTvPage();
  const dom = new JSDOM(html);
  assert.ok(dom.window.document.querySelector('.home-pinned-header > .home-feature > .home-feature-copy'));
  assert.ok(dom.window.document.querySelector('.home-navbar .browse-tabs'));
  assert.equal(dom.window.document.querySelector('.tv-guide-hero'),null);
  assert.equal(dom.window.document.querySelector('script, img[src="x"]'),null);
  dom.window.close();
  assert.match(html, /&lt;script&gt;/); assert.match(html, /&lt;ScRiPt&gt;/); assert.match(html, /&lt;img/); assert.doesNotMatch(html, /javascript:|<script>|<img src=x/i);
  assert.match(html, /Record program or series/); assert.match(html, /data-tv-record-options="1"/); assert.doesNotMatch(html, /data-tv-program-record|data-tv-tab="setup"/);
});
test('a previous account cannot reveal its cached DVR or setup data', () => {
  state.bootstrap = { current_user: { id: 2, admin: true } };
  state.liveTv = data({ tab: 'recordings', recordings: [{ title: 'Private recording' }] });
  assert.doesNotMatch(renderLiveTvPage(), /Private recording/);
});
test('setup lives in Settings and each source has its own connection form', () => {
  state.bootstrap = { current_user: { id: 1, admin: true } };
  state.liveTv = data({ setupKind: 'm3u' });
  assert.doesNotMatch(renderLiveTvPage(), /data-tv-tab="setup"|tv-source-form/);
  assert.match(renderLiveTvPage(), /settings\/live-tv/);
  for (const [kind,required,excluded] of [
    ['m3u','input[name="url"]','[name="native_device"], [name="pin"], [name="adapter"]'],
    ['nextpvr','input[name="pin"]','[name="native_device"], [name="adapter"]'],
    ['native','select[name="region"]','[name="url"], [name="pin"], [name="adapter"]'],
  ]) {
    state.liveTv.setupKind = kind;
    const dom = new JSDOM(renderLiveTvSettings()); const form = dom.window.document.querySelector('#tv-source-form');
    assert.ok(form.querySelector(required)); assert.equal(form.querySelector(excluded),null); dom.window.close();
  }
  globalThis.location.pathname = '/settings/live-tv'; assert.deepEqual(parseRoute(), { page:'settings', section:'live-tv' }); globalThis.location.pathname = '/live-tv';
});
test('only saved captures offer playback and conflicts stay visible', () => {
  state.bootstrap = { current_user: { id: 1, admin: false } };
  state.liveTv = data({ tab: 'recordings', recordings: [{ id: 7, title: 'Scheduled', starts_at: now, ends_at: now + 3600, state: 'scheduled', has_video: false }, { id: 8, title: 'Saved', starts_at: now, ends_at: now + 3600, state: 'completed', has_video: true }], conflicts: [7] });
  const html = renderLiveTvPage(); assert.match(html, /Tuner conflict/); assert.match(html, /data-tv-recording-play="8"/); assert.doesNotMatch(html, /data-tv-recording-play="7"/);
});
test('artwork rejects credentials and local or active URLs', () => {
  for (const value of ['file:///secret', 'data:image/png,abc', 'javascript:bad()', 'https://user:secret@example.com/image', 'invalid']) assert.equal(tvImage(value), '');
  assert.equal(tvImage('https://example.com/logo.png'), 'https://example.com/logo.png');
});

test('DVR controls honor scheduling permission and shared recording ownership', () => {
  state.bootstrap = { current_user: { id: 1, admin: false } };
  state.liveTv = data({ tab: 'recordings', canRecord: false, recordings: [{ id: 9, user_id: 2, shared: true, title: 'Shared show', state: 'completed', has_video: true, starts_at: now, ends_at: now+3600 }], rules: [{ id: 5, user_id: 2, shared: true, title: 'Shared series', channel_id: 1 }] });
  const html = renderLiveTvPage();
  assert.match(html, /Shared show/); assert.match(html, /data-tv-recording-play="9"/);
  assert.match(html, /Recording is disabled/); assert.doesNotMatch(html, /data-tv-delete="9"|tv-manual-record-form|data-tv-delete-rule="5"/);
});

test('setup includes Zap2It lineup discovery, DVR storage, and per-user permissions', () => {
  state.bootstrap = { current_user: { id: 1, admin: true } };
  state.liveTv = data({ dvrSettings: { settings: { recording_dir: 'D:\\TV' }, users: [{ user_id: 2, username: '<viewer>', can_record: false }] } });
  const html = renderLiveTvSettings();
  assert.match(html, /Zap2It/); assert.match(html, /name="guide_country"/); assert.match(html, /name="refresh_hours"/);
  assert.match(html, /DVR save folder/); assert.match(html, /data-tv-permission="2"/); assert.match(html, /&lt;viewer&gt;/);
  assert.doesNotMatch(html, /Install TSDuck/);
});

test('program artwork settings are separate from guide selection and retain provider attribution', () => {
  state.bootstrap = { current_user: { id: 1, admin: true } };
  state.liveTv = data({ setupKind: 'nextpvr' });
  const settings = new JSDOM(renderLiveTvSettings());
  const form = settings.window.document.querySelector('#tv-source-form');
  assert.equal(form.querySelector('[name="guide_kind"]').value, 'automatic');
  const artwork = form.querySelector('[name="artwork_provider"]');
  assert.deepEqual([...artwork.options].map(option => option.value), ['automatic', 'guide', 'tmdb', 'tvdb']);
  artwork.value = 'guide';
  assert.equal(form.querySelector('[name="guide_kind"]').value, 'automatic');
  settings.window.close();
  state.liveTv.programs = [{ ...program, metadata: { ...program.metadata, artwork_sources: ['tmdb', 'tvdb'] } }];
  const guide = new JSDOM(renderLiveTvPage());
  const attribution = guide.window.document.querySelector('.tv-artwork-attribution');
  assert.ok(attribution);
  assert.match(attribution.innerHTML, /themoviedb|tmdb/i);
  assert.match(attribution.innerHTML, /thetvdb|tvdb/i);
  guide.window.close();
});

test('native frequency regions come from supported options and manual tuner entry remains available', () => {
  state.bootstrap = { current_user: { id: 1, admin: true } };
  state.liveTv = data({ setupKind:'native', regions:['Europe','North-America'] });
  const dom = new JSDOM(renderLiveTvSettings());
  const select = dom.window.document.querySelector('select[name="region"]');
  assert.deepEqual([...select.options].map(o=>o.value),['Europe','North-America']);
  assert.equal(select.value,'North-America'); assert.ok(dom.window.document.querySelector('input[name="native_device"]')); dom.window.close();
});

test('the settings route requests administrator setup data without needing a Live TV setup tab', async () => {
  const target = app({ route:{page:'settings',section:'live-tv'}, bootstrap:{current_user:{id:1,admin:true}} });
  let requested = false;
  await loadLiveTv(target,()=>true,api({getTvChannels:async setup=>{assert.equal(setup,true);return[channel];},getTvSources:async()=>{requested=true;return[];},getTvDvrSettings:async()=>({settings:{recording_dir:''},users:[]})}));
  assert.ok(requested); assert.ok(target.liveTv.dvrSettings);
});
test('non-administrators never request setup endpoints', async () => {
  const target = app();
  await loadLiveTv(target, () => true, api({ getTvChannels: async setup => { assert.equal(setup, false); return [channel]; }, getTvSources: async () => { throw new Error('Setup must not be requested'); } }));
  assert.equal(target.liveTv.error, undefined); assert.equal(target.liveTv.programs.length, 1);
});
test('late account responses cannot publish private recording data', async () => {
  const target = app(); let release;
  const request = loadLiveTv(target, () => true, api({ getTvRecordings: () => new Promise(resolve => { release = resolve; }) }));
  target.bootstrap.current_user.id = 2;
  release({ recordings: [{ title: 'Private' }], conflicts: [] }); await request;
  assert.equal(target.liveTv.recordings.length, 0);
});
test('a slow previous guide window cannot overwrite a more recent navigation', async () => {
  const target = app(); let release;
  const first = loadLiveTv(target, () => true, api({ getTvGuide: () => new Promise(resolve => { release = resolve; }) }));
  target.liveTv.windowStart += 21600;
  await loadLiveTv(target, () => true, api({ getTvGuide: async () => ({ programs: [{ ...program, title: 'New window' }], stations: [] }) }));
  release({ programs: [{ ...program, title: 'Old window' }], stations: [] }); await first;
  assert.equal(target.liveTv.programs[0].title, 'New window');
});

test('expired guide columns are removed while future browsing windows remain selected',async()=>{
  const slot=currentGuideWindow();
  assert.equal(currentGuideWindow(slot,slot*1000+1799999),slot);
  assert.equal(currentGuideWindow(slot,slot*1000+1800000),slot+1800);
  assert.equal(currentGuideWindow(slot+21600,slot*1000),slot+21600);
  const target=app({liveTv:data({windowStart:slot-3600})});
  let from;
  await loadLiveTv(target,()=>true,api({getTvGuide:async start=>{from=start;return{programs:[],stations:[]};}}));
  assert.equal(from,slot); assert.equal(target.liveTv.windowStart,slot);
  const entries=[{starts_at:0,ends_at:1800},{starts_at:1800,ends_at:3600}];
  assert.equal(programAtTime(entries,1800),entries[1]);
});

test('the player describes its own broadcast, including buffered programs, without following guide browsing',()=>{
  state.bootstrap={current_user:{id:1,admin:true}};
  state.liveTv=data({programs:[{...program,metadata:{...program.metadata,subtitle:'Headlines',episode:'S1 E2',is_new:true,rating:'TV-PG'}},
    {...program,id:9,title:'Earlier broadcast',starts_at:now-3600,ends_at:now-30,metadata:{description:'Earlier details'}}],selectedProgramId:9});
  state.activeTvPlayback={owner:1,kind:'channels',id:1,title:channel.name,url:'/buffer',buffer:{end_ms:3600000}};
  const html=renderTvPlayerProgram(3600000);
  assert.match(html,/News/);assert.match(html,/Headlines/);assert.match(html,/Evening news/);assert.match(html,/NEW/);assert.match(html,/S1 E2/);assert.match(html,/TV-PG/);
  assert.doesNotMatch(html,/Earlier broadcast/);
  assert.match(renderTvPlayerProgram(1800000),/Earlier broadcast/);
  state.activeTvPlayback=undefined;
});
