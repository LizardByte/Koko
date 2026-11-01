/** Reproduces transient source errors and a cold IPTV connection that exceeds the former buffer deadline. */
import {test,expect} from '@playwright/test';
import {createServer} from 'node:http';
import {spawn} from 'node:child_process';

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
    expect(Date.now()-started).toBeGreaterThan(30000);expect(attempts).toBe(2);
    await expect.poll(()=>page.locator('#media-player').evaluate(video=>video.readyState>=2&&video.currentTime>0.2&&video.videoWidth===640),{timeout:45000}).toBeTruthy();
  }finally{
    if(sessionId&&headers)await page.request.delete(`/api/v1/live-tv/sessions/${sessionId}`,{headers});
    if(sourceId&&headers)await page.request.delete(`/api/v1/live-tv/sources/${sourceId}`,{headers});
    fixture.closeAllConnections();await new Promise(done=>fixture.close(done));
  }
});
