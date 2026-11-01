/** Verifies four independent Koko capture buffers and real browser decoders against a live IPTV fixture. */
import {test,expect} from '@playwright/test';
import {createServer} from 'node:http';
import {spawn} from 'node:child_process';

test('four live buffers play concurrently and release every source lease on exit',async({page},testInfo)=>{
  test.setTimeout(240000);
  const clients=new Set(),children=new Set();let sourceId,headers,encoder,generator;
  const start=()=>{
    if(encoder)return;
    generator=spawn(process.env.KOKO_E2E_GENERATOR_FFMPEG,['-hide_banner','-loglevel','error','-re','-f','lavfi','-i','testsrc2=size=320x180:rate=25','-f','lavfi','-i','sine=frequency=440:sample_rate=48000','-c:v','mpeg4','-q:v','5','-g','25','-c:a','aac','-f','mpegts','pipe:1'],{windowsHide:true,stdio:['ignore','pipe','ignore']});
    encoder=spawn(process.env.KOKO_E2E_FFMPEG,['-hide_banner','-loglevel','error','-i','pipe:0','-c:v','libx264','-preset','ultrafast','-tune','zerolatency','-g','25','-pix_fmt','yuv420p','-c:a','copy','-f','mpegts','pipe:1'],{windowsHide:true,stdio:['pipe','pipe','ignore']});
    children.add(generator);children.add(encoder);encoder.stdin.on('error',()=>undefined);generator.stdout.pipe(encoder.stdin);
    encoder.stdout.on('data',chunk=>{for(const response of clients)response.write(chunk);});
  };
  const fixture=createServer((request,response)=>{
    if(request.url==='/tv.m3u'){
      const base=`http://127.0.0.1:${fixture.address().port}`;
      response.end('#EXTM3U\n'+Array.from({length:4},(_,i)=>`#EXTINF:-1 tvg-id="multi-${i}" tvg-chno="${i+1}",Live Sport ${i+1}\n${base}/stream-${i}.ts\n`).join(''));return;
    }
    if(request.url?.startsWith('/stream-')){response.setHeader('Content-Type','video/mp2t');clients.add(response);response.on('close',()=>clients.delete(response));start();return;}
    response.statusCode=404;response.end();
  });
  await new Promise(done=>fixture.listen(0,'127.0.0.1',done));
  try{
    await page.goto('/');await page.getByLabel('Username',{exact:true}).fill('playback-admin');await page.getByLabel('Password',{exact:true}).fill(process.env.KOKO_E2E_PASSWORD);await page.getByRole('button',{name:'Sign in',exact:true}).click();await expect(page.locator('#login-form')).toHaveCount(0);
    headers={Authorization:`Bearer ${await page.evaluate(()=>localStorage.getItem('koko-client-web-auth-token'))}`};
    expect((await page.request.put('/api/v1/live-tv/dvr/settings',{headers,data:{recording_dir:testInfo.outputPath('buffers')}})).ok()).toBeTruthy();
    const created=await page.request.post('/api/v1/live-tv/sources',{headers,data:{name:'Four tuners',kind:'m3u',capacity:4,config:{url:`http://127.0.0.1:${fixture.address().port}/tv.m3u`,guide_kind:'automatic'}}});
    expect(created.ok()).toBeTruthy();const added=await created.json();sourceId=added.id;expect(added.refresh_error??null).toBeNull();
    let channels=[];await expect.poll(async()=>{channels=(await(await page.request.get('/api/v1/live-tv/channels',{headers})).json()).filter(c=>c.source_id===sourceId);return channels.length;},{timeout:45000}).toBe(4);
    await page.goto('/live-tv');await page.locator(`#tv-channel-select-${channels[0].id}`).click();
    await expect.poll(()=>page.locator('#media-player').evaluate(video=>video.readyState>=2&&video.currentTime>0),{timeout:60000}).toBeTruthy();
    for(const channel of channels.slice(1)){
      await page.mouse.move(400,400);await page.locator('#player-multiview').click();await page.locator(`[data-multi-picker-channel="${channel.id}"]`).click();
    }
    await expect(page.locator('#tv-player-stage video')).toHaveCount(4);
    await expect.poll(()=>page.locator('#tv-player-stage video').evaluateAll(videos=>videos.every(video=>video.readyState>=2&&!video.paused&&video.currentTime>0)),{timeout:60000}).toBeTruthy();
    expect(clients.size).toBe(4);
    await page.mouse.move(400,400);await page.getByRole('button',{name:'Main and sidebar',exact:true}).click();
    await page.locator(`[data-tv-multi-main="${channels[3].id}"]`).click();
    await expect.poll(()=>page.locator('#tv-player-stage video').evaluateAll(videos=>videos.filter(video=>!video.muted).length)).toBe(1);
    const frames=await page.locator('#tv-player-stage video').evaluateAll(videos=>videos.map(video=>({video,frames:video.getVideoPlaybackQuality().totalVideoFrames})).map(({frames})=>frames));
    await expect.poll(()=>page.locator('#tv-player-stage video').evaluateAll((videos,before)=>videos.every((video,i)=>video.getVideoPlaybackQuality().totalVideoFrames>before[i]),frames)).toBeTruthy();
    await page.screenshot({path:testInfo.outputPath('four-live-streams.png')});
    await page.goto('/');
    await expect.poll(()=>clients.size,{timeout:20000}).toBe(0);
  }finally{
    for(const response of clients){response.destroy();}for(const child of children){child.kill();}fixture.close();
    if(sourceId&&headers)await page.request.delete(`/api/v1/live-tv/sources/${sourceId}`,{headers}).catch(()=>undefined);
  }
});
