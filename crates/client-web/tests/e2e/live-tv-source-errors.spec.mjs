/** Upstream HTTP failures must reach the player as source errors, with safe actionable messages. */
import {test,expect} from '@playwright/test';
import {createServer} from 'node:http';

test('a missing IPTV stream reports its HTTP status instead of blaming DVR storage',async({page})=>{
  let sourceId,headers;
  const fixture=createServer((request,response)=>{
    if(request.url==='/channels.m3u') {
      response.end(`#EXTM3U\n#EXTINF:-1 tvg-id="missing" tvg-chno="410",Unavailable FOX\nhttp://127.0.0.1:${fixture.address().port}/missing.m3u8\n`);
    }else {response.writeHead(404,{'Content-Type':'text/plain'});response.end('404 not found');}
  });
  await new Promise(done=>fixture.listen(0,'127.0.0.1',done));
  try {
    await page.goto('/');await page.getByLabel('Username',{exact:true}).fill('playback-admin');await page.getByLabel('Password',{exact:true}).fill(process.env.KOKO_E2E_PASSWORD);await page.getByRole('button',{name:'Sign in',exact:true}).click();await expect(page.locator('#login-form')).toHaveCount(0);
    const token=await page.evaluate(()=>localStorage.getItem('koko-client-web-auth-token'));headers={Authorization:`Bearer ${token}`};
    const response=await page.request.post('/api/v1/live-tv/sources',{headers,data:{name:'Unavailable IPTV',kind:'m3u',capacity:1,config:{url:`http://127.0.0.1:${fixture.address().port}/channels.m3u`,guide_kind:'automatic'}}});expect(response.ok()).toBeTruthy();sourceId=(await response.json()).id;
    let channel;
    await expect.poll(async()=>{const channels=await(await page.request.get('/api/v1/live-tv/channels',{headers})).json();channel=channels.find(c=>c.source_id===sourceId);return !!channel;},{timeout:45000}).toBeTruthy();
    const playback=await page.request.post('/api/v1/live-tv/sessions',{headers,data:{channel_id:channel.id},timeout:45000});
    expect(playback.status()).toBe(400);
    const error=await playback.json();expect(error.message).toContain('HTTP 404');expect(error.message).toContain('source app');expect(error.message).not.toMatch(/disk|127\.0\.0\.1|token/i);
  }finally{
    if(sourceId&&headers)await page.request.delete(`/api/v1/live-tv/sources/${sourceId}`,{headers});
    await new Promise(done=>fixture.close(done));
  }
});

test('the player explains an upstream stream limit instead of a generic HTTP 5xx failure',async({page})=>{
  let sourceId,sessionId,headers,attempts=0;
  const fixture=createServer((request,response)=>{
    if(request.url==='/channels.m3u'){
      response.end(`#EXTM3U\n#EXTINF:-1 tvg-id="busy" tvg-chno="1",Busy Bridge TV\nhttp://127.0.0.1:${fixture.address().port}/busy.m3u8\n`);return;
    }
    attempts++;response.writeHead(503,{'Content-Type':'text/plain','Retry-After':'120'});
    response.end('Maximum concurrent streams (4) reached. Private diagnostic: http://private.example/?token=secret');
  });
  await new Promise(done=>fixture.listen(0,'127.0.0.1',done));
  try{
    await page.goto('/');await page.getByLabel('Username',{exact:true}).fill('playback-admin');await page.getByLabel('Password',{exact:true}).fill(process.env.KOKO_E2E_PASSWORD);await page.getByRole('button',{name:'Sign in',exact:true}).click();await expect(page.locator('#login-form')).toHaveCount(0);
    headers={Authorization:`Bearer ${await page.evaluate(()=>localStorage.getItem('koko-client-web-auth-token'))}`};
    const added=await page.request.post('/api/v1/live-tv/sources',{headers,data:{name:'Busy Bridge',kind:'m3u',capacity:1,config:{url:`http://127.0.0.1:${fixture.address().port}/channels.m3u`,guide_kind:'upload'}}});expect(added.ok()).toBeTruthy();sourceId=(await added.json()).id;
    let channel;await expect.poll(async()=>{channel=(await(await page.request.get('/api/v1/live-tv/channels',{headers})).json()).find(c=>c.source_id===sourceId);return !!channel;},{timeout:45000}).toBeTruthy();
    await page.goto('/live-tv');const tuning=page.waitForResponse(r=>r.url().endsWith('/api/v1/live-tv/sessions')&&r.request().method()==='POST');
    await page.locator(`#tv-channel-select-${channel.id}`).click();sessionId=(await(await tuning).json()).session_id;
    const notice=page.locator('.player-error-indicator');
    await expect(notice).toContainText('The TV source has reached its concurrent stream limit (4).');
    await expect(notice).toContainText('Stop another stream');
    await expect(notice).not.toContainText('5xx');await expect(notice).not.toContainText('private.example');await expect(notice).not.toContainText('secret');
    expect(attempts).toBe(1);
  }finally{
    if(sessionId&&headers)await page.request.delete(`/api/v1/live-tv/sessions/${sessionId}`,{headers});
    if(sourceId&&headers)await page.request.delete(`/api/v1/live-tv/sources/${sourceId}`,{headers});
    fixture.closeAllConnections();await new Promise(done=>fixture.close(done));
  }
});
