/** Starves a real browser decoder briefly and verifies automatic headroom without replacing it. */
import {test,expect} from '@playwright/test';
import {createServer} from 'node:http';
import {spawn} from 'node:child_process';

test('repeated live-edge starvation adds headroom while preserving the connection and player settings',async({page},testInfo)=>{
  test.setTimeout(100000);
  const children=new Set();let connections=0,tunes=0,streams=0,capturedAt,holding=false,release=()=>undefined;
  const fixture=createServer((request,response)=>{
    connections++;response.writeHead(200,{'Content-Type':'video/mp4','Access-Control-Allow-Origin':'*'});
    const generator=spawn(process.env.KOKO_E2E_GENERATOR_FFMPEG,['-hide_banner','-loglevel','error','-re','-f','lavfi','-i','testsrc2=size=320x180:rate=25','-f','lavfi','-i','sine=sample_rate=48000','-c:v','mpeg4','-g','25','-c:a','aac','-f','mpegts','pipe:1'],{windowsHide:true,stdio:['ignore','pipe','ignore']});
    const encoder=spawn(process.env.KOKO_E2E_FFMPEG,['-hide_banner','-loglevel','error','-i','pipe:0','-c:v','libx264','-preset','ultrafast','-tune','zerolatency','-g','25','-pix_fmt','yuv420p','-c:a','aac','-movflags','frag_keyframe+empty_moov+default_base_moof','-frag_duration','1000000','-f','mp4','pipe:1'],{windowsHide:true,stdio:['pipe','pipe','ignore']});
    children.add(generator);children.add(encoder);encoder.stdin.on('error',()=>undefined);generator.stdout.pipe(encoder.stdin);
    const queued=[];
    const flush=()=>{for(const held of queued.splice(0)){response.write(held);}};
    release=()=>{holding=false;flush();};
    encoder.stdout.on('data',chunk=>{
      capturedAt??=Date.now();
      if(holding){queued.push(chunk);return;}
      flush();response.write(chunk);
    });
    response.once('close',()=>{generator.kill();encoder.kill();});
  });
  await new Promise(done=>fixture.listen(0,'127.0.0.1',done));
  try{
    await page.addInitScript(()=>{
      globalThis.tvStartupReserves=[];
      document.addEventListener('ratechange',event=>{
        const video=event.target;
        if(video.id==='media-player'&&video.playbackRate>0&&video.currentTime<1&&video.buffered.length){
          globalThis.tvStartupReserves.push(video.buffered.end(0)-video.currentTime);
        }
      },true);
    });
    await page.goto('/');await page.getByLabel('Username',{exact:true}).fill('playback-admin');await page.getByLabel('Password',{exact:true}).fill(process.env.KOKO_E2E_PASSWORD);await page.getByRole('button',{name:'Sign in',exact:true}).click();await expect(page.locator('#login-form')).toHaveCount(0);
    const channel={id:1,source_id:1,source_key:'jitter',guide_key:'jitter',name:'Uneven delivery',number:'1',group_name:'Sports',enabled:true,available:true};
    await page.route('**/api/v1/live-tv/channels',route=>route.fulfill({json:[channel]}));
    await page.route('**/api/v1/live-tv/guide?**',route=>route.fulfill({json:{programs:[],stations:[],preferences:{favorites:[],recent:[]},sources:[{id:1,name:'Jitter fixture',kind:'m3u'}]}}));
    await page.route('**/api/v1/live-tv/recordings',route=>route.fulfill({json:{recordings:[],conflicts:[],can_record:true}}));
    await page.route('**/api/v1/live-tv/rules',route=>route.fulfill({json:[]}));
    await page.context().route(/\/api\/v1\/live-tv\/sessions(?:\/|$)/,route=>{
      const request=route.request(),path=new URL(request.url()).pathname;
      if(request.method()==='DELETE')return route.fulfill({status:204});
      if(path.endsWith('/sessions')){tunes++;}
      if(path.endsWith('/stream')){streams++;return route.fulfill({status:302,headers:{Location:`http://127.0.0.1:${fixture.address().port}/live.mp4`}});}
      return route.fulfill({json:{session_id:'jitter',channel_id:1,title:channel.name,start_ms:0,end_ms:capturedAt?Date.now()-capturedAt:0,finished:false,can_keep:true}});
    });
    await page.goto('/live-tv');await page.locator('#tv-channel-select-1').click();
    const player=page.locator('#media-player');
    await expect.poll(()=>player.evaluate(video=>video.readyState>=2&&video.currentTime>0.1&&video.playbackRate>0),{timeout:25000}).toBeTruthy();
    expect(await page.evaluate(()=>globalThis.tvStartupReserves.some(seconds=>seconds>=1.9))).toBeTruthy();
    const initialStreams=streams,initialConnections=connections;
    await player.evaluate(video=>{
      globalThis.originalTvVideo=video;globalThis.liveWaits=0;globalThis.liveHolds=0;globalThis.liveSeeks=0;
      video.volume=0.3;video.addEventListener('waiting',()=>globalThis.liveWaits++);
      video.addEventListener('ratechange',()=>{if(video.playbackRate===0)globalThis.liveHolds++;});
      video.addEventListener('seeking',()=>globalThis.liveSeeks++);
    });
    holding=true;
    await expect.poll(()=>page.evaluate(()=>globalThis.liveWaits),{timeout:15000}).toBeGreaterThan(0);
    const firstWait=await player.evaluate(video=>video.currentTime);release();
    await expect.poll(()=>player.evaluate(video=>video.currentTime)).toBeGreaterThan(firstWait+2);
    holding=true;
    await expect.poll(()=>page.evaluate(()=>globalThis.liveWaits),{timeout:15000}).toBeGreaterThanOrEqual(2);
    await expect.poll(()=>page.evaluate(()=>globalThis.liveHolds)).toBeGreaterThan(0);
    if(await player.evaluate(video=>video.playbackRate===0))await expect(page.locator('.media-player-shell')).toHaveClass(/is-media-loading/);
    const result=await player.evaluate(video=>({same:video===globalThis.originalTvVideo,volume:video.volume,waits:globalThis.liveWaits,seeks:globalThis.liveSeeks,position:video.currentTime}));
    expect(result.same).toBe(true);expect(result.volume).toBeCloseTo(0.3);expect(result.waits).toBeGreaterThanOrEqual(2);
    expect(result.seeks).toBe(0);
    release();
    await expect.poll(()=>player.evaluate(video=>video.playbackRate)).toBe(1);
    await expect.poll(()=>player.evaluate(video=>video.currentTime),{timeout:15000}).toBeGreaterThan(result.position+3);
    expect(tunes).toBe(1);expect(streams).toBe(initialStreams);expect(connections).toBe(initialConnections);
    await expect(page.locator('.player-problem')).not.toBeVisible();
  }finally{
    await testInfo.attach('rebuffering',{body:JSON.stringify(await page.evaluate(()=>({waits:globalThis.liveWaits,holds:globalThis.liveHolds,seeks:globalThis.liveSeeks,position:document.querySelector('video')?.currentTime}))),contentType:'application/json'});
    await page.goto('/');for(const child of children){child.kill();}fixture.closeAllConnections();await new Promise(done=>fixture.close(done));
  }
});
