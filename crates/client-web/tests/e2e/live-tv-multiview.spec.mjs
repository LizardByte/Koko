/** Exercises multiple real browser decoders, persistent tile swaps, bounded selection, and session cleanup. */
import {test,expect} from '@playwright/test';

async function fixture(page,holdFirst=false){
  const now=Math.floor(Date.now()/1000),channels=Array.from({length:120},(_,i)=>({id:i+1,source_id:i<4?1:2,source_key:String(i+1),guide_key:String(i+1),name:`Sports ${i+1}`,number:String(i+1),group_name:'Sports',provider_id:'espn',enabled:true,available:true}));
  const programs=channels.map(c=>({id:c.id,source_id:c.source_id,guide_key:c.guide_key,title:`Game ${c.id}`,starts_at:now-3600,ends_at:now+3600,metadata:{categories:['Sports']}}));
  const starts=[],closes=[],streams=[],errors=[],sessions=new Map();let serial=0,fail,hold,release;if(holdFirst)hold=1;
  page.on('pageerror',error=>errors.push(error.message));
  await page.goto('/');await page.getByLabel('Username',{exact:true}).fill('playback-admin');await page.getByLabel('Password',{exact:true}).fill(process.env.KOKO_E2E_PASSWORD);await page.getByRole('button',{name:'Sign in',exact:true}).click();await expect(page.locator('#login-form')).toHaveCount(0);
  const token=await page.evaluate(()=>localStorage.getItem('koko-client-web-auth-token')),headers={Authorization:`Bearer ${token}`};
  const items=await(await page.request.get('/api/v1/items',{headers})).json(),sample=items.find(i=>i.display_title==='Playback Sample');
  const video=await(await page.request.get(`/api/v1/items/${sample.id}/stream`,{headers})).body();
  await page.route('**/api/v1/live-tv/channels',route=>route.fulfill({json:channels}));
  await page.route('**/api/v1/live-tv/guide?**',route=>route.fulfill({json:{programs,stations:[],preferences:{favorites:[2],recent:[1]},sources:[{id:1,name:'Sports source',kind:'m3u'},{id:2,name:'Other source',kind:'m3u'}],providers:[{source_id:1,id:'espn',name:'ESPN',enabled:true,channel_count:4},{source_id:2,id:'espn',name:'ESPN',enabled:true,channel_count:116}]}}));
  await page.route('**/api/v1/live-tv/recordings',route=>route.fulfill({json:{recordings:[],conflicts:[],can_record:true}}));
  await page.route('**/api/v1/live-tv/rules',route=>route.fulfill({json:[]}));
  await page.context().route(/\/api\/v1\/live-tv\/sessions(?:\/|$)/,async route=>{
    const request=route.request(),url=new URL(request.url());
    if(url.pathname.endsWith('/sessions')){
      const channel=request.postDataJSON().channel_id;starts.push(channel);
      if(hold===channel)await new Promise(done=>{release=done;});
      if(fail===channel)return route.fulfill({status:400,json:{status:400,message:'No tuner is available for this source'}});
      const buffer={session_id:`multi-${++serial}`,channel_id:channel,title:`Sports ${channel}`,start_ms:0,end_ms:60000,finished:false,can_keep:true};sessions.set(buffer.session_id,buffer);return route.fulfill({json:buffer});
    }
    const id=/\/sessions\/([^/]+)/.exec(url.pathname)?.[1];
    if(request.method()==='DELETE'){closes.push(id);return route.fulfill({status:204});}
    if(url.pathname.endsWith('/stream')){streams.push(id);return route.fulfill({body:video,contentType:'video/mp4'});}
    return route.fulfill({json:sessions.get(id)});
  });
  await page.goto('/live-tv');await page.locator('#tv-channel-select-1').click();
  if(!holdFirst)await expect.poll(()=>page.locator('#media-player').evaluate(video=>video.readyState>=2&&!video.paused)).toBeTruthy();
  return {starts,closes,streams,errors,sessions,fail:id=>{fail=id;},hold:id=>{hold=id;},release:()=>{release?.();},add:async id=>{
    await page.locator('#player-multiview').click();
    const picker=page.getByRole('dialog',{name:'Add a stream'});await expect(picker).toBeVisible();await picker.getByLabel('Find a channel',{exact:true}).fill(`Sports ${id}`);await picker.locator(`[data-multi-picker-channel="${id}"]`).click();
  }};
}

test('four streams retain their decoders across layouts and main swaps, with only main audio',async({page},testInfo)=>{
  test.setTimeout(120000);const f=await fixture(page);
  await f.add(2);await f.add(3);await f.add(4);
  await expect(page.locator('#tv-player-stage video')).toHaveCount(4);
  await expect(page.getByRole('button',{name:'Main and sidebar',exact:true})).toHaveAttribute('aria-pressed','true');
  await expect(page.getByRole('button',{name:'Main and sidebar',exact:true}).locator('svg')).toHaveClass(/lucide-layout-panel-left/);
  await expect.poll(()=>page.locator('#tv-player-stage video').evaluateAll(videos=>videos.every(v=>v.readyState>=2&&!v.paused))).toBeTruthy();
  await page.evaluate(()=>{globalThis.tvVideos=Array.from(document.querySelectorAll('#tv-player-stage video')).map(video=>({video,id:video.closest('section').id}));});
  expect(f.starts).toEqual([1,2,3,4]);
  await expect(page.locator('#player-multiview')).toBeDisabled();
  await expect.poll(()=>page.locator('#tv-player-stage video').evaluateAll(videos=>videos.filter(v=>!v.muted).length)).toBe(1);
  await page.getByRole('button',{name:'Main and sidebar',exact:true}).click();
  const main=await page.locator('#tv-stream-1').boundingBox(),side=await page.locator('#tv-stream-2').boundingBox();expect(main.width).toBeGreaterThan(side.width*2);
  await page.screenshot({path:testInfo.outputPath('multiview-focus.png')});
  await page.locator('#tv-stream-3 video').click();await expect(page.locator('#tv-stream-3 #media-player')).toHaveCount(1);
  expect(await page.evaluate(()=>globalThis.tvVideos.every(({video})=>video.isConnected))).toBeTruthy();
  expect(f.starts).toEqual([1,2,3,4]);await expect.poll(()=>page.locator('#tv-player-stage video').evaluateAll(videos=>videos.filter(v=>!v.muted).map(v=>v.closest('section').id))).toEqual(['tv-stream-3']);
  await page.getByRole('button',{name:'Equal grid',exact:true}).click();await page.screenshot({path:testInfo.outputPath('multiview-grid.png')});
  await page.locator('#player-volume').fill('0.3');await page.locator('#player-volume').dispatchEvent('input');
  await page.locator('#player-quality').selectOption('480p');await expect.poll(()=>page.locator('#media-player').evaluate(video=>video.volume)).toBeCloseTo(0.3,1);
  expect(await page.evaluate(()=>globalThis.tvVideos.filter(({id})=>id!=='tv-stream-3').every(({video})=>video.isConnected))).toBeTruthy();
  await page.getByRole('button',{name:'Remove Sports 2',exact:true}).click();await expect(page.locator('#tv-stream-2')).toHaveCount(0);await expect.poll(()=>f.closes.length).toBe(1);
  await page.locator('#close-player').click();await expect(page.locator('.tv-preview-player #tv-player-stage video')).toHaveCount(3);
  expect(f.closes.length).toBe(1);await page.locator('#expand-tv-player').click();await expect(page.locator('.tv-preview-player')).toHaveCount(0);
  await page.getByRole('button',{name:'Watch main only',exact:true}).click();await expect(page.locator('#tv-player-stage video')).toHaveCount(1);await expect(page.locator('#tv-stream-3 #media-player')).toHaveCount(1);
  await expect.poll(()=>page.locator('#media-player').evaluate(video=>!video.paused&&video.readyState>=2)).toBeTruthy();
  await expect.poll(()=>f.closes.length).toBe(3);expect(f.starts).toEqual([1,2,3,4]);
  await page.locator('#close-player').click();await page.getByRole('button',{name:'Stop playback',exact:true}).click();await expect.poll(()=>f.closes.length).toBe(4);expect(new Set(f.closes).size).toBe(4);expect(f.errors).toEqual([]);
});

test('failed and canceled tunes release only their tile, and the picker stays bounded for large lineups',async({page},testInfo)=>{
  const f=await fixture(page);f.fail(2);await f.add(2);await expect(page.locator('#tv-stream-2 .tv-stream-problem')).toContainText('No tuner is available');
  await page.evaluate(()=>{globalThis.tvMain=document.querySelector('#media-player');});
  f.fail(undefined);await page.getByRole('button',{name:'Retry stream',exact:true}).click();await expect.poll(()=>page.locator('#tv-stream-2 video').evaluate(video=>video.readyState>=2&&!video.paused)).toBeTruthy();
  expect(await page.evaluate(()=>globalThis.tvMain===document.querySelector('#media-player'))).toBeTruthy();
  await page.locator('#player-multiview').click();const picker=page.getByRole('dialog',{name:'Add a stream'});
  await expect(picker.locator('[data-multi-picker-channel]')).toHaveCount(50);await expect(picker.locator('[data-multi-picker-channel]').first()).toHaveAttribute('data-multi-picker-channel','3');await picker.getByLabel('Source',{exact:true}).selectOption('1');await expect(picker.locator('[data-multi-picker-channel]')).toHaveCount(4);
  await picker.getByLabel('Favorites',{exact:true}).check();await expect(picker.locator('[data-multi-picker-channel]')).toHaveCount(1);await expect(picker.locator('[data-multi-picker-channel="2"]')).toBeDisabled();await page.keyboard.press('Escape');
  f.hold(3);await f.add(3);await expect(page.locator('#tv-stream-3')).toContainText('Starting stream');await expect.poll(()=>f.starts.includes(3)).toBeTruthy();await page.getByRole('button',{name:'Remove Sports 3',exact:true}).click();f.release();await expect.poll(()=>f.closes.length).toBe(1);
  await page.setViewportSize({width:390,height:844});await page.locator('#player-multiview').click();await page.screenshot({path:testInfo.outputPath('multiview-picker-mobile.png')});await page.keyboard.press('Escape');
  await page.locator('#close-player').click();await page.locator('#mobile-library-picker').selectOption('/');await expect.poll(()=>f.closes.length).toBe(3);expect(f.errors).toEqual([]);
});

test('a late first tune survives promotion, removing main preserves its replacement, and leaving Live TV closes every buffer',async({page})=>{
  const f=await fixture(page,true);await expect.poll(()=>f.starts.includes(1)).toBeTruthy();
  await f.add(2);await expect.poll(()=>page.locator('#tv-stream-2 video').evaluate(video=>video.readyState>=2&&!video.paused)).toBeTruthy();
  await page.locator('[data-tv-multi-main="2"]').click();f.release();
  await expect.poll(()=>page.locator('#tv-stream-1 video').evaluate(video=>video.readyState>=2&&!video.paused)).toBeTruthy();expect(f.closes).toEqual([]);
  await page.evaluate(()=>{globalThis.tvReplacement=document.querySelector('#tv-stream-1 video');});
  await page.getByRole('button',{name:'Remove Sports 2',exact:true}).click();await expect(page.locator('#tv-stream-1 #media-player')).toHaveCount(1);
  expect(await page.evaluate(()=>globalThis.tvReplacement===document.querySelector('#media-player'))).toBeTruthy();await expect.poll(()=>f.closes.length).toBe(1);
  await f.add(3);await expect.poll(()=>page.locator('#tv-stream-3 video').evaluate(video=>video.readyState>=2&&!video.paused)).toBeTruthy();
  await page.locator('#close-player').click();await page.getByRole('button',{name:'Home',exact:true}).click();await expect.poll(()=>f.closes.length).toBe(3);expect(new Set(f.closes).size).toBe(3);expect(f.errors).toEqual([]);
});

test('retrying a stopped single-channel capture creates a new buffer instead of seeking the failed one',async({page})=>{
  const f=await fixture(page),buffer=f.sessions.values().next().value;
  buffer.error='The upstream capture stopped';buffer.finished=true;
  await expect(page.locator('.player-error-indicator')).toContainText('The upstream capture stopped');
  await page.getByRole('button',{name:'Retry playback',exact:true}).click();
  await expect.poll(()=>f.starts.length).toBe(2);await expect.poll(()=>f.closes.length).toBe(1);
  expect(f.starts).toEqual([1,1]);expect(f.closes).toEqual([buffer.session_id]);
  await expect.poll(()=>page.locator('#media-player').evaluate(video=>video.readyState>=2&&!video.paused)).toBeTruthy();
  await expect(page.locator('.player-error-indicator')).not.toBeVisible();expect(f.errors).toEqual([]);
});
