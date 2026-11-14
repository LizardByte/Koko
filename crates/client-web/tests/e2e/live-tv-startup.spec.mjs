/** Reproduces transient source errors and a cold IPTV connection that exceeds the former buffer deadline. */
import {test,expect} from '@playwright/test';
import {createServer} from 'node:http';
import {spawn} from 'node:child_process';

async function selectChannel(page,channel,first){
  if(first){await page.locator(`#tv-channel-select-${channel.id}`).click();return;}
  if(!await page.locator('#player-tv-guide').isVisible()){await page.locator('#player-tv-guide-toggle').click();}
  await page.locator(`[data-player-tv-channel="${channel.id}"]`).click();
}

test('a cold channel survives a transient HTTP error and more than thirty seconds before its first video bytes',async({page})=>{
  test.setTimeout(150000);
  const transport=await new Promise((done,reject)=>{
    const generator=spawn(process.env.KOKO_E2E_GENERATOR_FFMPEG??process.env.KOKO_E2E_FFMPEG??'ffmpeg',['-hide_banner','-loglevel','error','-f','lavfi','-i','testsrc2=size=640x360:rate=25','-f','lavfi','-i','sine=frequency=440:sample_rate=48000','-t','14','-c:v','mpeg2video','-q:v','5','-g','25','-c:a','aac','-f','mpegts','pipe:1'],{windowsHide:true,stdio:['ignore','pipe','ignore']});
    const chunks=[];generator.stdout.on('data',chunk=>chunks.push(chunk));generator.once('error',reject);generator.once('exit',code=>code===0?done(Buffer.concat(chunks)):reject(new Error('Unable to generate startup fixture')));
  });
  let attempts=0,sourceId,sessionId,headers;
  const fixture=createServer((request,response)=>{
    if(request.url==='/tv.m3u'){response.end(`#EXTM3U\n#EXTINF:-1 tvg-id="cold" tvg-chno="1",Cold Start TV\nhttp://127.0.0.1:${fixture.address().port}/stream.ts\n`);return;}
    if(request.url!=='/stream.ts'){response.writeHead(404);response.end();return;}
    attempts++;if(attempts===1){response.writeHead(503);response.end();return;}
    const timer=setTimeout(()=>{response.writeHead(200,{'Content-Type':'video/mp2t'});response.write(transport);response.end();},32000);
    response.once('close',()=>clearTimeout(timer));
  });
  await new Promise(done=>fixture.listen(0,'127.0.0.1',done));
  try{
    await page.goto('/');await page.getByLabel('Username',{exact:true}).fill('playback-admin');await page.getByLabel('Password',{exact:true}).fill(process.env.KOKO_E2E_PASSWORD);await page.getByRole('button',{name:'Sign in',exact:true}).click();await expect(page.locator('#login-form')).toHaveCount(0);
    headers={Authorization:`Bearer ${await page.evaluate(()=>localStorage.getItem('koko-client-web-auth-token'))}`};
    const added=await page.request.post('/api/v1/live-tv/sources',{headers,data:{name:'Cold Start',kind:'m3u',capacity:1,config:{url:`http://127.0.0.1:${fixture.address().port}/tv.m3u`,guide_kind:'upload'}}});expect(added.ok()).toBeTruthy();sourceId=(await added.json()).id;
    let channel;await expect.poll(async()=>{channel=(await(await page.request.get('/api/v1/live-tv/channels',{headers})).json()).find(c=>c.source_id===sourceId);return !!channel;},{timeout:45000}).toBeTruthy();
    await page.goto('/live-tv');const started=Date.now();
    const tuning=page.waitForResponse(response=>response.url().endsWith('/api/v1/live-tv/sessions')&&response.request().method()==='POST',{timeout:90000});
    await page.locator(`#tv-channel-select-${channel.id}`).click();
    const tuned=await tuning,session=await tuned.json();expect(tuned.ok(),JSON.stringify(session)).toBeTruthy();sessionId=session.session_id;
    expect(Date.now()-started).toBeLessThan(5000);expect(session.end_ms).toBe(0);
    await page.waitForTimeout(3500);
    await expect(page.locator('.media-player-shell')).not.toHaveClass(/is-controls-hidden/);
    await expect.poll(()=>page.locator('#media-player').evaluate(video=>video.readyState>=2&&video.currentTime>0.2&&video.videoWidth===640),{timeout:90000}).toBeTruthy();
    expect(Date.now()-started).toBeGreaterThan(30000);expect(attempts).toBe(2);
  }finally{
    if(sessionId&&headers)await page.request.delete(`/api/v1/live-tv/sessions/${sessionId}`,{headers});
    if(sourceId&&headers)await page.request.delete(`/api/v1/live-tv/sources/${sourceId}`,{headers});
    fixture.closeAllConnections();await new Promise(done=>fixture.close(done));
  }
});

test('channel surfing cancels cold captures and plays while the temporary buffer is still growing',async({page})=>{
  test.setTimeout(120000);
  let sourceId,sessionId,headers,slowConnections=0,closedSlow=0;
  const generators=new Set();
  const fixture=createServer((request,response)=>{
    if(request.url==='/tv.m3u'){
      response.end('#EXTM3U\n'+[1,2,3,4].map(id=>{const path=id===4?'fast':'slow-'+id;return `#EXTINF:-1 tvg-id="surf-${id}" tvg-chno="${id}",Surf ${id}\nhttp://127.0.0.1:${fixture.address().port}/${path}.ts`;}).join('\n')+'\n');return;
    }
    response.writeHead(200,{'Content-Type':'video/mp2t'});response.flushHeaders();
    if(request.url.startsWith('/slow-')){
      slowConnections++;response.once('close',()=>{slowConnections--;closedSlow++;});return;
    }
    const generator=spawn(process.env.KOKO_E2E_GENERATOR_FFMPEG??process.env.KOKO_E2E_FFMPEG??'ffmpeg',[
      '-hide_banner','-loglevel','error','-re','-f','lavfi','-i','testsrc2=size=640x360:rate=25',
      '-f','lavfi','-i','sine=frequency=440:sample_rate=48000','-t','40',
      '-c:v','mpeg2video','-q:v','5','-g','25','-c:a','aac','-f','mpegts','pipe:1',
    ],{windowsHide:true,stdio:['ignore','pipe','ignore']});
    generators.add(generator);generator.stdout.pipe(response);
    response.once('close',()=>generator.kill());generator.once('exit',()=>generators.delete(generator));
  });
  await new Promise(done=>fixture.listen(0,'127.0.0.1',done));
  try{
    await page.goto('/');await page.getByLabel('Username',{exact:true}).fill('playback-admin');await page.getByLabel('Password',{exact:true}).fill(process.env.KOKO_E2E_PASSWORD);await page.getByRole('button',{name:'Sign in',exact:true}).click();await expect(page.locator('#login-form')).toHaveCount(0);
    headers={Authorization:`Bearer ${await page.evaluate(()=>localStorage.getItem('koko-client-web-auth-token'))}`};
    const added=await page.request.post('/api/v1/live-tv/sources',{headers,data:{name:'Channel Surfing',kind:'m3u',capacity:1,config:{url:`http://127.0.0.1:${fixture.address().port}/tv.m3u`,guide_kind:'upload'}}});expect(added.ok()).toBeTruthy();sourceId=(await added.json()).id;
    let channels;await expect.poll(async()=>{channels=(await(await page.request.get('/api/v1/live-tv/channels',{headers})).json()).filter(c=>c.source_id===sourceId).sort((a,b)=>Number(a.number)-Number(b.number));return channels.length;},{timeout:45000}).toBe(4);
    await page.goto('/live-tv');
    for(let index=0;index<channels.length;index++){
      const started=Date.now(),tuning=page.waitForResponse(r=>r.url().endsWith('/api/v1/live-tv/sessions')&&r.request().method()==='POST');
      const opening=index===3?page.waitForRequest(r=>new URL(r.url()).pathname.endsWith('/stream')&&r.url().includes('/live-tv/sessions/')):undefined;
      await selectChannel(page,channels[index],index===0);
      const tuned=await tuning,session=await tuned.json();expect(tuned.ok(),JSON.stringify(session)).toBeTruthy();sessionId=session.session_id;
      expect(Date.now()-started).toBeLessThan(5000);
      if(index<3)await expect.poll(()=>slowConnections).toBe(1);
      else{
        expect(session.end_ms).toBe(0);await opening;
        const initial=await(await page.request.get(`/api/v1/live-tv/sessions/${sessionId}`,{headers})).json();expect(initial.end_ms).toBe(0);
        await expect.poll(()=>page.locator('#media-player').evaluate(video=>video.readyState>=2&&video.currentTime>0.2),{timeout:20000}).toBeTruthy();
        const status=await(await page.request.get(`/api/v1/live-tv/sessions/${sessionId}`,{headers})).json();
        expect(status.error).toBeNull();expect(status.finished).toBe(false);expect(status.end_ms).toBeGreaterThan(0);
        await expect.poll(()=>page.locator('#media-player').evaluate(video=>video.currentTime),{timeout:20000}).toBeGreaterThan(10);
      }
    }
    expect(closedSlow).toBe(3);expect(slowConnections).toBe(0);
    await page.request.delete(`/api/v1/live-tv/sessions/${sessionId}`,{headers});sessionId=undefined;
    await expect.poll(()=>generators.size).toBe(0);
  }finally{
    if(sessionId&&headers)await page.request.delete(`/api/v1/live-tv/sessions/${sessionId}`,{headers});
    if(sourceId&&headers)await page.request.delete(`/api/v1/live-tv/sources/${sourceId}`,{headers});
    for(const generator of generators){generator.kill();}fixture.closeAllConnections();await new Promise(done=>fixture.close(done));
  }
});
