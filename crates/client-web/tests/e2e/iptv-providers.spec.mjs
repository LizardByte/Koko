/** Exercises one-address FastChannels setup, reusable provider grouping, and persistent icon choices. */
import {test,expect} from '@playwright/test';
import {createServer} from 'node:http';

async function openSettingsSource(card) {
  if(!(await card.evaluate(element=>element.open))) await card.locator(':scope > summary').click();
  await expect(card).toHaveJSProperty('open',true);
}

test('IPTV groups FastChannels and E+TV providers and keeps channel-linked icons current',async({page},testInfo)=>{
  test.setTimeout(180000);
  let revision=1,sourceId,headers;
  const requested=new Set();
  const icon=Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=','base64');
  const stamp=seconds=>new Date(seconds*1000).toISOString().slice(0,19).replaceAll(/[-T:]/g,'')+' +0000';
  const fixture=createServer((request,response)=>{
    const base=`http://127.0.0.1:${fixture.address().port}`;requested.add(request.url);
    response.setHeader('Content-Type','application/x-mpegurl');
    if(request.url==='/feeds/default/native/m3u') response.end(`#EXTM3U\n#EXTINF:-1 channel-id="pluto.news" tvg-id="pluto.news" tvg-chno="100" tvg-logo="${base}/pluto.png" group-title="News",Pluto News\n${base}/native.ts\n`);
    else if(request.url==='/feeds/default/m3u/prismcast') response.end(`#EXTM3U\n#EXTINF:-1 channel-id="espn.espn1" tvg-id="espn.espn1" tvg-chno="200" tvg-logo="${base}/espn-${revision}.png" group-title="Sports",ESPN\n${base}/bridge.m3u8\n`);
    else if(request.url==='/feeds/default/m3u/prismcast/gracenote') response.end(`#EXTM3U\n#EXTINF:-1 channel-id="sling.sports" tvc-guide-stationid="123" tvg-chno="300" group-title="Sports",Sling Sports\n${base}/other.m3u8\n#EXTINF:-1 channel-id="fox_one.fox_sports" tvg-id="fox_one.fox_sports" tvg-chno="400" group-title="Sports",FOX Sports\n${base}/fox.m3u8\n`);
    else if(request.url==='/feeds/default/native/epg.xml') {
      const now=Math.floor(Date.now()/1000);response.setHeader('Content-Type','application/xml');
      response.end('<tv>'+['pluto.news','espn.espn1','sling.sports'].map(id=>`<channel id="${id}"><display-name>${id}</display-name></channel><programme channel="${id}" start="${stamp(now-600)}" stop="${stamp(now+3600)}"><title>${id} program</title></programme>`).join('')+'</tv>');
    }else if(request.url==='/eplus.m3u') response.end(`#EXTM3U\n#EXTINF:0 channel-id="foxone.FOX" tvg-id="410.eplustv" tvg-chno="410",FOX\n${base}/fox.ts\n`);
    else if(request.url.endsWith('.png')) {response.setHeader('Content-Type','image/png');response.end(icon);}
    else {response.statusCode=404;response.end();}
  });
  await new Promise(done=>fixture.listen(0,'127.0.0.1',done));
  try {
    const base=`http://127.0.0.1:${fixture.address().port}`;
    await page.goto('/');await page.getByLabel('Username',{exact:true}).fill('playback-admin');await page.getByLabel('Password',{exact:true}).fill(process.env.KOKO_E2E_PASSWORD);await page.getByRole('button',{name:'Sign in',exact:true}).click();await expect(page.locator('#login-form')).toHaveCount(0);
    const token=await page.evaluate(()=>localStorage.getItem('koko-client-web-auth-token'));headers={Authorization:`Bearer ${token}`};
    await page.goto('/settings/live-tv');
    const addPanel=page.locator('#tv-add-source-panel');if(!(await addPanel.evaluate(element=>element.open))) await addPanel.locator('summary').click();
    await page.locator('#tv-source-kind').selectOption('m3u');
    const form=page.locator('#tv-source-form');await form.locator('[name="name"]').fill('FastChannels test');await form.locator('[name="iptv_mode"]').selectOption('fastchannels');
    await expect(form.locator('[name="provider_grouping"]')).toBeChecked();await expect(form.getByText('FastChannels server address',{exact:true})).toBeVisible();
    await form.locator('[name="url"]').fill(base);await form.locator('[name="capacity"]').fill('5');await form.getByRole('button',{name:'Add source',exact:true}).click();
    await expect.poll(async()=>{
      const sources=await(await page.request.get('/api/v1/live-tv/sources',{headers})).json();const source=sources.find(s=>s.name==='FastChannels test');sourceId=source?.id;return source?.channel_count;
    },{timeout:45000}).toBe(4);
    for(const path of ['/feeds/default/native/m3u','/feeds/default/m3u/prismcast','/feeds/default/m3u/prismcast/gracenote','/feeds/default/native/epg.xml'])expect(requested.has(path)).toBeTruthy();
    await page.reload();
    const card=page.locator(`[data-tv-source-card="${sourceId}"]`);
    await expect(card).toHaveJSProperty('open',false);
    await expect(card).toContainText('4 channels');
    await expect(card.locator('.tv-source-heading img')).toHaveAttribute('src',/\/assets\/providers\/fastchannels\.svg$/);
    await page.goto('/live-tv');
    const presetIcon=page.locator(`[data-tv-source="${sourceId}"] img`);
    await expect(presetIcon).toHaveAttribute('src',/\/assets\/providers\/fastchannels\.svg$/);
    await expect.poll(()=>presetIcon.evaluate(image=>image.complete&&image.naturalWidth>0)).toBeTruthy();
    const fastFox=page.locator(`[data-tv-provider="${sourceId}:fox_one"]`);
    await expect(fastFox).toContainText('FOX One');
    await expect(fastFox.locator('img')).toHaveAttribute('src',/\/assets\/providers\/tv-foxone\.ico$/);
    await fastFox.scrollIntoViewIfNeeded();
    await expect.poll(()=>fastFox.locator('img').evaluate(image=>image.complete&&image.naturalWidth>0)).toBeTruthy();
    await page.goto('/settings/live-tv');
    await openSettingsSource(card);
    const details=card.locator(`[data-tv-provider-editor="${sourceId}:espn"]`);await details.locator('summary').click();
    await expect(details.locator('summary img')).toHaveAttribute('src',/\/assets\/providers\/tv-espn\.png$/);
    const providerForm=details.locator('form');
    const channels=await(await page.request.get('/api/v1/live-tv/setup/channels',{headers})).json();const espn=channels.find(c=>c.source_id===sourceId&&c.name==='ESPN');
    await providerForm.locator('[name="name"]').fill('My ESPN');await providerForm.locator('[name="icon_channel"]').selectOption(`channel:${espn.id}`);await providerForm.getByRole('button',{name:'Save provider',exact:true}).click();
    await expect(details.locator('summary')).toContainText('My ESPN');await expect(details.locator('summary img')).toHaveAttribute('src',`${base}/espn-1.png`);
    const sourceIcon=card.getByLabel('Upload icon for FastChannels test',{exact:true});await sourceIcon.setInputFiles({name:'source.png',mimeType:'image/png',buffer:icon});
    await expect(card.locator('.tv-source-heading img')).toHaveAttribute('src',new RegExp(`/api/v1/live-tv/sources/${sourceId}/image`));
    await expect.poll(()=>card.locator('.tv-source-heading img').evaluate(image=>image.complete&&image.naturalWidth>0)).toBeTruthy();
    await page.screenshot({path:testInfo.outputPath('iptv-provider-settings.png'),fullPage:true});
    await page.goto('/live-tv');await expect(page.locator('[data-tv-provider=""] [aria-label="All providers icon"]')).toBeVisible();await page.locator(`[data-tv-provider="${sourceId}:espn"]`).click();
    await expect(page.locator('.tv-guide-channel')).toHaveCount(1);await expect(page.locator('.tv-guide-channel')).toContainText('ESPN');
    await page.screenshot({path:testInfo.outputPath('iptv-provider-guide.png')});
    revision=2;await page.request.post(`/api/v1/live-tv/sources/${sourceId}/refresh`,{headers});
    await expect.poll(async()=>{
      const guide=await(await page.request.get('/api/v1/live-tv/guide',{headers})).json();return guide.providers.find(p=>p.source_id===sourceId&&p.id==='espn')?.image_url;
    },{timeout:45000}).toBe(`${base}/espn-2.png`);
    await page.reload();await expect(page.locator(`[data-tv-provider="${sourceId}:espn"] img`)).toHaveAttribute('src',`${base}/espn-2.png`);
    await page.goto('/settings/live-tv');await openSettingsSource(card);await details.locator('summary').click();
    await details.locator('[name="enabled"]').uncheck();await details.getByRole('button',{name:'Save provider',exact:true}).click();
    await page.goto('/live-tv');await expect(page.locator(`[data-tv-provider="${sourceId}:espn"]`)).toHaveCount(0);
    const visible=await(await page.request.get('/api/v1/live-tv/channels',{headers})).json();expect(visible.some(c=>c.id===espn.id)).toBeFalsy();
    const old=await(await page.request.get('/api/v1/live-tv/setup/channels',{headers})).json();expect(old.find(c=>c.id===espn.id).enabled).toBeTruthy();
    await page.goto('/settings/live-tv');await openSettingsSource(card);await details.locator('summary').click();await details.locator('[name="enabled"]').check();await details.getByRole('button',{name:'Save provider',exact:true}).click();
    await details.getByLabel('Upload icon for My ESPN',{exact:true}).setInputFiles({name:'provider.png',mimeType:'image/png',buffer:icon});
    await expect(details.locator('summary img')).toHaveAttribute('src',new RegExp(`/api/v1/live-tv/sources/${sourceId}/image.*provider=espn`));
    await expect(details.locator('[name="icon_channel"]')).toHaveValue('upload');
    await details.locator('[name="icon_channel"]').selectOption('default');await details.getByRole('button',{name:'Save provider',exact:true}).click();await expect(details.locator('summary img')).toHaveAttribute('src',/\/assets\/providers\/tv-espn\.png$/);
    await card.getByRole('button',{name:'Use default icon',exact:true}).click();await expect(card.locator('.tv-source-heading img')).toHaveAttribute('src',/\/assets\/providers\/fastchannels\.svg$/);
    await details.getByRole('button',{name:'Manage channels',exact:true}).click();await expect(card.locator('[data-tv-channel-update]')).toHaveCount(1);await expect(card.locator('[data-tv-channel-update]')).toContainText('ESPN');
    await card.locator('[data-tv-connection-source] summary').click();const connection=card.locator('[data-tv-replace-source]');await expect(connection.locator('[name="url"]')).toHaveValue(base);await expect(connection.locator('[name="iptv_mode"]')).toHaveValue('fastchannels');await expect(connection.locator('[name="provider_grouping"]')).toBeChecked();
    const eplus=await page.request.post('/api/v1/live-tv/sources',{headers,data:{name:'E+TV test',kind:'m3u',capacity:1,config:{url:`${base}/eplus.m3u`,guide_kind:'automatic',provider_grouping:true}}});expect(eplus.ok()).toBeTruthy();const otherId=(await eplus.json()).id;
    await expect.poll(async()=>{const sources=await(await page.request.get('/api/v1/live-tv/sources',{headers})).json();return sources.find(s=>s.id===otherId)?.providers[0]?.name;},{timeout:45000}).toBe('FOX One');
    await page.goto('/live-tv');await page.locator(`[data-tv-source="${otherId}"]`).click();
    await expect(page.locator(`[data-tv-provider="${otherId}:foxone"] img`)).toHaveAttribute('src',/\/assets\/providers\/tv-foxone\.ico$/);
    await page.request.put(`/api/v1/live-tv/sources/${otherId}`,{headers,data:{enabled:false,capacity:1}});
  }finally{
    if(sourceId&&headers)await page.request.put(`/api/v1/live-tv/sources/${sourceId}`,{headers,data:{enabled:false,capacity:1}});
    await new Promise(done=>fixture.close(done));
  }
});

test('Live TV source cards show application icons and support cancelling or confirming removal',async({page},testInfo)=>{
  test.setTimeout(120000);
  const ids=[];
  const fixture=createServer((request,response)=>{
    if(request.url==='/lineup.json') {response.setHeader('Content-Type','application/json');response.end('[]');}
    else if(request.url.startsWith('/service')) {response.setHeader('Content-Type','application/json');response.end('{"stat":"fail"}');}
    else response.end('#EXTM3U\n');
  });
  await new Promise(done=>fixture.listen(0,'127.0.0.1',done));
  let headers;
  try {
    await page.goto('/');await page.getByLabel('Username',{exact:true}).fill('playback-admin');await page.getByLabel('Password',{exact:true}).fill(process.env.KOKO_E2E_PASSWORD);await page.getByRole('button',{name:'Sign in',exact:true}).click();await expect(page.locator('#login-form')).toHaveCount(0);
    const token=await page.evaluate(()=>localStorage.getItem('koko-client-web-auth-token'));headers={Authorization:`Bearer ${token}`};
    for(const kind of ['hdhomerun','nextpvr','tvheadend']) {
      const response=await page.request.post('/api/v1/live-tv/sources',{headers,data:{name:`${kind} icon test`,kind,capacity:1,config:{url:`http://127.0.0.1:${fixture.address().port}`,pin:'0000',guide_kind:'upload'}}});
      expect(response.ok()).toBeTruthy();ids.push({kind,id:(await response.json()).id});
    }
    await page.goto('/settings/live-tv');
    for(const {kind,id} of ids) {
      await expect(page.locator(`[data-tv-source-card="${id}"]`)).toHaveJSProperty('open',false);
      const icon=page.locator(`[data-tv-source-card="${id}"] .tv-source-heading img`);
      await expect(icon).toHaveAttribute('src',new RegExp(`/assets/providers/${kind}\\.`));
      await icon.scrollIntoViewIfNeeded();
      await expect.poll(()=>icon.evaluate(image=>image.complete&&image.naturalWidth>0)).toBeTruthy();
      await expect.poll(()=>icon.locator('..').evaluate(element=>getComputedStyle(element).backgroundColor)).toBe('rgba(0, 0, 0, 0)');
      await page.locator(`[data-tv-source-card="${id}"]`).screenshot({path:testInfo.outputPath(`${kind}-source-card.png`)});
    }
    await page.screenshot({path:testInfo.outputPath('live-tv-application-icons.png'),fullPage:true});
    const first=page.locator(`[data-tv-source-card="${ids[0].id}"]`);
    await openSettingsSource(first);
    await first.getByRole('button',{name:'Remove source',exact:true}).click();
    const modal=page.getByRole('dialog');await expect(modal).toContainText('Saved recordings and their files will be kept');
    await expect(modal.getByRole('button',{name:'Cancel',exact:true})).toBeFocused();
    await page.screenshot({path:testInfo.outputPath('live-tv-remove-confirmation.png')});
    await page.keyboard.press('Escape');await expect(modal).toHaveCount(0);await expect(first).toBeVisible();
    await first.getByRole('button',{name:'Remove source',exact:true}).click();await modal.getByRole('button',{name:'Cancel',exact:true}).click();await expect(first).toBeVisible();
    for(const {id} of ids) {
      const card=page.locator(`[data-tv-source-card="${id}"]`);
      await openSettingsSource(card);
      await card.getByRole('button',{name:'Remove source',exact:true}).click();
      await modal.getByRole('button',{name:'Remove source',exact:true}).click();
      await expect(card).toHaveCount(0);
    }
    const sources=await(await page.request.get('/api/v1/live-tv/sources',{headers})).json();
    expect(sources.some(source=>ids.some(({id})=>source.id===id))).toBeFalsy();
  }finally{
    if(headers) for(const {id} of ids) await page.request.delete(`/api/v1/live-tv/sources/${id}`,{headers});
    await new Promise(done=>fixture.close(done));
  }
});
