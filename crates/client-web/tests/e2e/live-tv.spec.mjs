/** Exercises real IPTV import, browser decoding, tuner release, and timed DVR capture. */
import { test, expect } from '@playwright/test';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { stat } from 'node:fs/promises';

test('IPTV imports its guide, plays live video, releases the tuner, and saves a timed recording', async ({ page }, testInfo) => {
  test.setTimeout(300000);
  page.setDefaultTimeout(15000);
  const children = new Set();
  let activeStreams = 0;
  let sourceId;
  let headers;
  const stamp = seconds => new Date(seconds * 1000).toISOString().slice(0, 19).replaceAll(/[-T:]/g, '') + ' +0000';
  const fixture = createServer((request, response) => {
    const base = `http://127.0.0.1:${fixture.address().port}`;
    if (request.url === '/tv.m3u') {
      response.end(`#EXTM3U\n#EXTINF:-1 tvg-id="test-tv" tvg-chno="7.1" tvg-logo="${base}/logo.svg" group-title="News",Live Test TV\n${base}/stream.ts\n#EXTINF:-1 tvg-id="hls-tv" tvg-chno="7.2",HLS Test TV\n${base}/master.m3u8\n#EXTINF:-1 tvg-id="alternate-tv" tvg-chno="7.3",Alternate TV\n${base}/stream.ts\n`);
    } else if (request.url === '/logo.svg') {
      response.setHeader('Content-Type','image/svg+xml');
      response.end('<svg xmlns="http://www.w3.org/2000/svg" width="100" height="60"><rect width="100" height="60" fill="#3a8"/><text y="40" x="15" fill="white" font-size="30">TV</text></svg>');
    } else if (request.url === '/master.m3u8') {
      response.setHeader('Content-Type', 'application/octet-stream');
      response.write('#EXT');
      setImmediate(() => response.end('M3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,RESOLUTION=640x360\nhls.m3u8\n'));
    } else if (request.url === '/hls.m3u8') {
      response.setHeader('Content-Type', 'application/vnd.apple.mpegurl');
      response.end('#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n#EXT-X-MEDIA-SEQUENCE:0\n#EXTINF:2,\nsegment0.ts\n#EXT-X-DISCONTINUITY\n#EXTINF:2,\nsegment1.ts\n#EXT-X-ENDLIST\n');
    } else if (request.url === '/guide.xml') {
      const now = Math.floor(Date.now() / 1000);
      response.setHeader('Content-Type', 'application/xml');
      response.end(`<tv><channel id="test-tv"><display-name>Live Test TV</display-name></channel><programme channel="test-tv" start="${stamp(now - 600)}" stop="${stamp(now + 3600)}"><title>Live Test News</title><desc>Local IPTV guide fixture.</desc><category>News</category><new/></programme></tv>`);
    } else if (request.url === '/stream.ts' || /^\/segment\d+\.ts$/.test(request.url)) {
      activeStreams += 1;
      const ffmpeg = spawn(process.env.KOKO_E2E_GENERATOR_FFMPEG ?? process.env.KOKO_E2E_FFMPEG ?? 'ffmpeg', ['-hide_banner', '-loglevel', 'error', '-re',
        '-f', 'lavfi', '-i', 'testsrc2=size=640x360:rate=25', '-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=48000',
        '-c:v','mpeg4','-q:v','5', '-g', '25', '-c:a', 'aac', '-f', 'mpegts', ...(request.url === '/stream.ts' ? [] : ['-t', '2']), 'pipe:1'],
      { windowsHide: true, stdio: ['ignore', 'pipe', 'ignore'] });
      children.add(ffmpeg);
      let encoder;
      if (request.url === '/stream.ts') {
        // The fixture generator has lavfi; Koko's bundled FFmpeg supplies libx264.
        encoder=spawn(process.env.KOKO_E2E_FFMPEG,['-hide_banner','-loglevel','error','-i','pipe:0','-c:v','libx264','-preset','ultrafast','-tune','zerolatency','-g','25','-pix_fmt','yuv420p','-c:a','copy','-f','mpegts','pipe:1'],{windowsHide:true,stdio:['pipe','pipe','ignore']});
        children.add(encoder); encoder.stdin.on('error',()=>undefined);
        ffmpeg.stdout.pipe(encoder.stdin); encoder.once('error',()=>response.destroy());
      }
      response.setHeader('Content-Type', 'video/mp2t');
      (encoder ?? ffmpeg).stdout.pipe(response);
      response.once('close', () => { activeStreams -= 1; ffmpeg.kill(); children.delete(ffmpeg); encoder?.kill(); children.delete(encoder); });
      ffmpeg.once('error', () => response.destroy());
    } else { response.statusCode = 404; response.end(); }
  });
  await new Promise(done => fixture.listen(0, '127.0.0.1', done));
  try {
    await page.goto('/');
    await page.getByLabel('Username', { exact: true }).fill('playback-admin');
    await page.getByLabel('Password', { exact: true }).fill(process.env.KOKO_E2E_PASSWORD);
    await page.getByRole('button', { name: 'Sign in', exact: true }).click();
    await expect(page.locator('#login-form')).toHaveCount(0);
    const token = await page.evaluate(() => localStorage.getItem('koko-client-web-auth-token'));
    headers = { Authorization: `Bearer ${token}` };
    const initialStorage = await page.request.put('/api/v1/live-tv/dvr/settings',{headers,data:{recording_dir:testInfo.outputPath('initial-capture')}});
    expect(initialStorage.ok()).toBeTruthy();
    const base = `http://127.0.0.1:${fixture.address().port}`;
    const created = await page.request.post('/api/v1/live-tv/sources', { headers, data: {
      name: 'Local IPTV test', kind: 'm3u', capacity: 1,
      config: { url: `${base}/tv.m3u`, guide_kind: 'xmltv', xmltv_url: `${base}/guide.xml` },
    } });
    expect(created.ok()).toBeTruthy(); sourceId = (await created.json()).id;
    let channel;
    await expect.poll(async () => {
      const response = await page.request.get('/api/v1/live-tv/channels', { headers });
      channel = (await response.json()).find(c => c.source_id === sourceId && c.name === 'Live Test TV');
      return channel?.name;
    }, { timeout: 45000 }).toBe('Live Test TV');
    expect(channel).not.toHaveProperty('stream_url');
    await page.goto('/live-tv');
    await expect(page.getByRole('heading', { name:'Live Test News', exact:true })).toBeVisible();
    await expect(page.locator('#tv-guide-scroll')).toHaveCount(1);
    await expect(page.locator('.tv-guide-tick')).toHaveCount(12);
    await expect(page.locator('.live-tv-programs')).toHaveCount(0);
    await page.getByRole('button',{name:'Add Live Test TV to favorites',exact:true}).click();
    await expect(page.getByRole('button',{name:'Remove Live Test TV from favorites',exact:true})).toHaveAttribute('aria-pressed','true');
    await page.getByRole('button',{name:'Filters',exact:true}).click();
    await page.locator('[data-tv-channel-view="favorites"]').click();
    await expect(page.locator('.tv-guide-row')).toHaveCount(1);
    await page.locator('[data-tv-channel-view="all"]').click();
    await page.screenshot({ path: testInfo.outputPath('live-tv-guide.png') });
    const channels = await (await page.request.get('/api/v1/live-tv/channels', { headers })).json();
    const hls = channels.find(c => c.source_id === sourceId && c.name === 'HLS Test TV');
    await page.locator(`[data-tv-play="${hls.id}"]`).first().click();
    await expect.poll(() => page.locator('#media-player').evaluate(video => video.readyState >= 2 && video.currentTime > 0.1),{timeout:45000}).toBeTruthy();
    await page.getByRole('button', { name:'Close player',exact:true }).click();
    await expect(page.locator('.tv-preview-player #media-player')).toBeAttached();
    await page.getByRole('button', {name:'Stop playback',exact:true}).click();
    await expect.poll(() => activeStreams).toBe(0);
    // A delayed tune must be closed before a replacement asks the same single tuner.
    let releaseTune, tuneReady;
    const pendingTune = new Promise(resolve=>{releaseTune=resolve;});
    const captured = new Promise(resolve=>{tuneReady=resolve;});
    await page.route('**/api/v1/live-tv/sessions',async route=>{
      const response = await route.fetch();
      tuneReady(); await pendingTune; await route.fulfill({response});
    },{times:1});
    await page.locator(`[data-tv-play="${channel.id}"]`).first().click();
    await captured;
    await page.getByRole('button',{name:'Close player',exact:true}).click();
    const alternate = channels.find(c=>c.source_id===sourceId && c.name==='Alternate TV');
    await page.locator(`[data-tv-play="${alternate.id}"]`).first().click();
    releaseTune();
    await expect.poll(()=>page.locator('#media-player').evaluate(node=>node.readyState>=2),{timeout:45000}).toBeTruthy();
    expect(activeStreams).toBe(1);
    await page.getByRole('button',{name:'Close player',exact:true}).click();
    await page.getByRole('button',{name:'Stop playback',exact:true}).click();
    await expect.poll(()=>activeStreams).toBe(0);
    await page.locator(`[data-tv-play="${channel.id}"]`).first().click();
    await expect.poll(() => page.locator('#media-player').evaluate(video => video.readyState >= 2 && video.currentTime > 0.2),{timeout:45000}).toBeTruthy();
    const viewport = await page.locator('.media-player-overlay').boundingBox();
    expect(viewport.x).toBe(0); expect(viewport.y).toBe(0); expect(viewport.height).toBe(900);
    await page.getByRole('button',{name:'Pause',exact:true}).click();
    await page.getByRole('button',{name:'Play',exact:true}).click();
    await expect(page.locator('.player-channel-heading h2')).toHaveText('Live Test TV');
    await expect.poll(()=>page.locator('.player-channel-logo').evaluate(image=>image.naturalWidth)).toBeGreaterThan(0);
    await page.locator('#player-volume').evaluate(input=>{input.value='0.27';input.dispatchEvent(new Event('input',{bubbles:true}));});
    // Refresh the surrounding app much more frequently than the real guide timer.
    // These updates must not rebind controls, change volume, or replace SVG click targets.
    await page.evaluate(()=>{
      window.tvCloseIcon=document.querySelector('#close-player svg');
      window.tvQuality=document.querySelector('#player-quality');
      window.tvRefreshTimer=setInterval(()=>document.querySelector('[data-tv-refresh]')?.click(),750);
    });
    await page.getByLabel('Playback quality',{exact:true}).focus();
    await page.waitForTimeout(2200);
    expect(await page.evaluate(()=>window.tvQuality===document.activeElement && window.tvQuality===document.querySelector('#player-quality'))).toBe(true);
    await page.locator('.media-player-shell').focus();
    await page.mouse.move(5,5);
    await expect(page.locator('.media-player-shell')).toHaveClass(/is-controls-hidden/);
    const steady = await page.locator('#media-player').evaluate(async video=>{
      let waits=0; const onWait=()=>{waits++;}; video.addEventListener('waiting',onWait);
      const start=video.currentTime;
      await new Promise(resolve=>setTimeout(resolve,12000));
      video.removeEventListener('waiting',onWait);
      return {elapsed:video.currentTime-start,waits};
    });
    expect(steady.elapsed).toBeGreaterThan(10); expect(steady.waits).toBeLessThanOrEqual(1);
    await expect(page.locator('.media-player-shell')).toHaveClass(/is-controls-hidden/);
    expect(await page.locator('#media-player').evaluate(video=>video.volume)).toBe(0.27);
    expect(await page.evaluate(()=>window.tvCloseIcon===document.querySelector('#close-player svg'))).toBe(true);
    await page.evaluate(()=>clearInterval(window.tvRefreshTimer));
    await page.locator('.media-player-shell').hover();
    expect(activeStreams).toBe(1);
    const video = await page.locator('#media-player').elementHandle();
    const originalSource = await video.getAttribute('src');
    const bufferId = new URL(originalSource,'http://localhost').pathname.split('/').at(-2);
    // Freeze only the reported clock: readyState stays healthy, as in a decoder
    // that stops advancing without emitting a useful stalled/error event.
    await video.evaluate(node=>{
      const frozen=node.currentTime;
      Object.defineProperty(node,'currentTime',{configurable:true,get:()=>frozen});
    });
    await expect.poll(()=>page.locator('#media-player').getAttribute('src'),{timeout:25000}).not.toBe(originalSource);
    await expect(page.locator('#media-player')).toHaveAttribute('src',new RegExp(`sessions/${bufferId}/stream.*quality=original`));
    await expect.poll(()=>page.locator('#media-player').evaluate(node=>node.currentTime>0.2),{timeout:20000}).toBeTruthy();
    expect(await page.locator('#media-player').evaluate(node=>node.volume)).toBe(0.27);
    expect(activeStreams).toBe(1);
    const recoveredVideo = await page.locator('#media-player').elementHandle();
    const initialBuffer = await (await page.request.get(`/api/v1/live-tv/sessions/${bufferId}`,{headers})).json();
    await page.getByRole('button',{name:'Pause',exact:true}).click();
    await expect.poll(async()=> (await (await page.request.get(`/api/v1/live-tv/sessions/${bufferId}`,{headers})).json()).end_ms,{timeout:15000}).toBeGreaterThan(initialBuffer.end_ms);
    await page.getByRole('button',{name:'Play',exact:true}).click();
    await page.getByRole('button', { name:'Close player',exact:true }).click();
    expect(await recoveredVideo.evaluate(node=>node === document.querySelector('#media-player'))).toBe(true);
    await expect(page.locator('.tv-preview-player')).toBeVisible();
    await expect(page.locator('#tv-guide-hero #tv-hero-media .tv-preview-player #media-player')).toHaveCount(1);
    await expect(page.locator('.tv-preview-player .player-controls')).toHaveCount(0);
    await expect(page.locator('.tv-preview-player')).toHaveCSS('position','absolute');
    const watchButton = page.locator('#tv-guide-hero [data-tv-play]');
    await expect(watchButton).toContainText('Watch channel live');
    await expect(watchButton).not.toHaveCSS('color','rgba(0, 0, 0, 0)');
    await page.screenshot({path:testInfo.outputPath('live-tv-mini-player.png')});
    expect(activeStreams).toBe(1);
    await page.locator('[data-tv-channel-view="recent"]').click();
    await expect(page.locator('.tv-guide-row')).toHaveCount(3);
    expect(await recoveredVideo.evaluate(node=>node === document.querySelector('#media-player'))).toBe(true);
    await page.getByRole('button',{name:'Expand player',exact:true}).click();
    await page.getByLabel('Playback quality',{exact:true}).selectOption('720p');
    await expect(page.locator('#media-player')).toHaveAttribute('src',new RegExp(`sessions/${bufferId}/stream.*quality=720p`));
    await expect.poll(()=>page.locator('#media-player').evaluate(node=>node.readyState>=2 && node.currentTime>0.1),{timeout:20000}).toBeTruthy();
    expect(activeStreams).toBe(1);
    const beforeSeek = await page.locator('#media-player').evaluate(video=>Number(new URL(video.src).searchParams.get('start_ms'))+video.currentTime*1000);
    await page.getByRole('button',{name:'Back 10 seconds',exact:true}).click();
    const seekPosition = await page.locator('#media-player').evaluate(video=>Number(new URL(video.src).searchParams.get('start_ms')));
    expect(Math.abs(seekPosition-Math.max(0,beforeSeek-10000))).toBeLessThan(1500);
    await expect.poll(()=>page.locator('#media-player').evaluate(node=>node.readyState>=2),{timeout:20000}).toBeTruthy();
    await page.getByRole('button', {name:'Close player',exact:true}).click();
    await page.getByRole('button', {name:'Stop playback',exact:true}).click();
    await expect.poll(() => activeStreams).toBe(0);
    const closedBuffer = await page.request.get(`/api/v1/live-tv/sessions/${bufferId}`,{headers});
    expect(closedBuffer.ok()).toBe(false);

    const recordingDirectory = testInfo.outputPath('dvr-storage');
    const storage = await page.request.put('/api/v1/live-tv/dvr/settings', { headers, data: { recording_dir: recordingDirectory } });
    expect(storage.ok()).toBeTruthy();
    // Keeping a buffer reuses its tuner, then survives switching and leaving Live TV.
    const capacity = await page.request.put(`/api/v1/live-tv/sources/${sourceId}`, { headers, data: { enabled:true, capacity:2 } });
    expect(capacity.ok()).toBeTruthy();
    await page.locator('[data-tv-channel-view="all"]').click();
    await page.locator(`[data-tv-play="${channel.id}"]`).first().click();
    await expect.poll(()=>page.locator('#media-player').evaluate(node=>node.readyState>=2),{timeout:45000}).toBeTruthy();
    const keepResponse = page.waitForResponse(response=>response.url().endsWith('/keep') && response.request().method()==='POST');
    await page.getByRole('button',{name:'Keep recording',exact:true}).click();
    await expect(page.getByRole('menu',{name:'Keep recording',exact:true})).toBeVisible();
    const recordingButton=await page.locator('#player-keep-tv').boundingBox();
    const recordingMenu=await page.locator('#player-recording-menu').boundingBox();
    expect(recordingMenu.y+recordingMenu.height).toBeLessThan(recordingButton.y);
    await page.evaluate(()=>document.querySelector('[data-tv-refresh]').click());
    await page.waitForTimeout(1000);
    await expect(page.getByRole('menu',{name:'Keep recording',exact:true})).toBeVisible();
    await page.getByRole('menuitem',{name:/Shared recording/}).click();
    const keptResult = await keepResponse;
    expect(keptResult.ok()).toBeTruthy();
    expect(keptResult.request().postDataJSON()).toEqual({private:false});
    await expect(page.getByRole('button',{name:'Recording kept',exact:true})).toBeDisabled();
    const keptId = await keptResult.json();
    await expect.poll(async()=> (await stat(testInfo.outputPath('dvr-storage',`recording-${keptId}.ts`)).catch(()=>({size:0}))).size).toBeGreaterThan(0);
    expect(activeStreams).toBe(1);
    await page.getByRole('button',{name:'Close player',exact:true}).click();
    await page.locator(`[data-tv-play="${alternate.id}"]`).first().click();
    await expect.poll(()=>activeStreams).toBe(2);
    await expect.poll(()=>page.locator('#media-player').evaluate(node=>node.readyState>=2),{timeout:45000}).toBeTruthy();
    await page.getByRole('button',{name:'Close player',exact:true}).click();
    await page.locator('[data-nav-settings]').click();
    await expect(page.locator('#media-player')).toHaveCount(0);
    await expect.poll(()=>activeStreams).toBe(1);
    const keptList = await (await page.request.get('/api/v1/live-tv/recordings',{headers})).json();
    expect(keptList.recordings.find(recording=>recording.id===keptId)).toMatchObject({state:'recording',shared:true});
    const cancelled = await page.request.post(`/api/v1/live-tv/recordings/${keptId}/cancel`,{headers});
    expect(cancelled.ok()).toBeTruthy();
    await expect.poll(()=>activeStreams,{timeout:15000}).toBe(0);
    const removedKept = await page.request.delete(`/api/v1/live-tv/recordings/${keptId}`,{headers});
    expect(removedKept.ok()).toBeTruthy();
    await page.locator('[data-nav-live-tv]').click();
    const now = Math.floor(Date.now() / 1000);
    const scheduled = await page.request.post('/api/v1/live-tv/recordings', { headers, data: {
      channel_id: channel.id, title: 'Timed IPTV recording', starts_at: now - 1, ends_at: now + 12,
      series: false, new_only: false, padding_before: 0, padding_after: 0,
    } });
    expect(scheduled.ok()).toBeTruthy();
    let recording;
    await expect.poll(async () => {
      const response = await page.request.get('/api/v1/live-tv/recordings', { headers });
      recording = (await response.json()).recordings.find(r => r.title === 'Timed IPTV recording');
      return recording?.state;
    }, { timeout: 35000 }).toBe('completed');
    expect(recording.has_video).toBeTruthy();
    expect(recording.shared).toBeTruthy();
    const savedFile = testInfo.outputPath('dvr-storage', `recording-${recording.id}.ts`);
    expect((await stat(savedFile)).size).toBeGreaterThan(0);
    // Changing the folder for future captures must not move or break the completed video.
    const changedStorage = await page.request.put('/api/v1/live-tv/dvr/settings', { headers, data: { recording_dir: '' } });
    expect(changedStorage.ok()).toBeTruthy();
    await page.getByRole('button', { name: 'Recordings', exact: true }).click();
    await expect(page.getByText('Timed IPTV recording', { exact: true })).toBeVisible();
    await page.locator(`[data-tv-recording-play="${recording.id}"]`).click();
    await expect.poll(() => page.locator('#media-player').evaluate(video => video.readyState >= 2 && video.currentTime > 0.1)).toBeTruthy();
    await page.getByRole('button', {name:'Close player',exact:true}).click();
    await page.getByRole('button', {name:'Stop playback',exact:true}).click();
    const unauthenticated = await page.request.get('/api/v1/live-tv/recordings', { headers: { Authorization: 'Bearer invalid' } });
    expect(unauthenticated.status()).toBe(401);
    const deleted = await page.request.delete(`/api/v1/live-tv/recordings/${recording.id}`, { headers });
    expect(deleted.ok(), await deleted.text()).toBeTruthy();
    await expect(stat(savedFile)).rejects.toMatchObject({ code: 'ENOENT' });
  } finally {
    if (sourceId && headers) await page.request.put(`/api/v1/live-tv/sources/${sourceId}`, { headers, data: { enabled: false, capacity: 1 } });
    for (const child of children) child.kill();
    fixture.closeAllConnections();
    await new Promise(done => fixture.close(done));
  }
});

test('Live TV Settings separates setup flows and finds local tuner metadata without hardware', async ({ page }, testInfo) => {
  test.setTimeout(90000);
  await page.goto('/');
  await page.getByLabel('Username', { exact: true }).fill('playback-admin');
  await page.getByLabel('Password', { exact: true }).fill(process.env.KOKO_E2E_PASSWORD);
  await page.getByRole('button', { name:'Sign in', exact:true }).click();
  await expect(page.locator('#login-form')).toHaveCount(0);
  await page.goto('/settings/live-tv');
  await expect(page.locator('#tv-source-kind')).toBeAttached();
  const panel = page.locator('details').filter({ has: page.locator('#tv-source-form') }).last();
  if (!(await panel.evaluate(node=>node.open))) await panel.locator('summary').first().click();
  const form = page.locator('#tv-source-form');
  await page.route('**/api/v1/live-tv/discover/hdhomerun', route => route.fulfill({ json:[
    {name:'Living room HDHomeRun',url:'http://192.0.2.10/',capacity:4},
    {name:'Office HDHomeRun',url:'http://192.0.2.11/',capacity:2},
  ] }));
  await form.getByRole('button',{name:'Find HDHomeRun',exact:true}).click();
  const livingRoom = form.getByRole('radio', { name:/Living room HDHomeRun/ });
  const office = form.getByRole('radio', { name:/Office HDHomeRun/ });
  await expect(form.getByRole('radio')).toHaveCount(2);
  await livingRoom.check();
  await expect(form.locator('[name="url"]')).toHaveValue('http://192.0.2.10/');
  await expect(form.locator('[name="capacity"]')).toHaveValue('4');
  await expect(form.locator('[name="name"]')).toHaveValue('Living room HDHomeRun');
  await form.locator('.live-tv-source-discovery').scrollIntoViewIfNeeded();
  await form.locator('.live-tv-source-discovery').screenshot({ path:testInfo.outputPath('live-tv-discovery-cards.png') });
  await page.setViewportSize({ width:390, height:844 });
  await form.locator('.live-tv-discovery-results').scrollIntoViewIfNeeded();
  const cards = await form.locator('.live-tv-discovery-card').evaluateAll(nodes => nodes.map(node => {
    const rect = node.getBoundingClientRect(); return {left:rect.left, right:rect.right, top:rect.top, bottom:rect.bottom};
  }));
  expect(cards[0].left).toBeGreaterThanOrEqual(0);
  expect(cards[0].right).toBeLessThanOrEqual(390);
  expect(cards[1].top).toBeGreaterThanOrEqual(cards[0].bottom);
  await page.screenshot({ path:testInfo.outputPath('live-tv-discovery-cards-mobile.png'), fullPage:true });
  await page.setViewportSize({ width:1440, height:900 });
  await livingRoom.focus(); await livingRoom.press('ArrowRight');
  await expect(office).toBeChecked();
  await expect(form.locator('[name="url"]')).toHaveValue('http://192.0.2.11/');
  await expect(form.locator('[name="capacity"]')).toHaveValue('2');
  await expect(form.locator('[name="name"]')).toHaveValue('Office HDHomeRun');
  await form.locator('[name="name"]').fill('Antenna'); await livingRoom.check();
  await expect(form.locator('[name="name"]')).toHaveValue('Antenna');
  await form.locator('[name="url"]').fill('http://192.0.2.99/');
  await expect(form.locator('input[type="radio"]:checked')).toHaveCount(0);
  if (process.env.KOKO_E2E_NEXT_PVR_URL) {
    const expected = new URL(process.env.KOKO_E2E_NEXT_PVR_URL).href;
    await page.locator('#tv-source-kind').selectOption('nextpvr');
    await form.getByRole('button', { name:'Find NextPVR', exact:true }).click();
    const card = form.locator('.live-tv-discovery-card').filter({ hasText:expected });
    await expect(card).toHaveCount(1);
    await card.getByRole('radio').check();
    await expect(form.getByLabel('NextPVR address')).toHaveValue(expected);
    await form.locator('.live-tv-source-discovery').screenshot({ path:testInfo.outputPath('live-tv-nextpvr-cards.png') });
  }
  await page.locator('#tv-source-kind').selectOption('m3u');
  await expect(form.getByLabel('IPTV playlist URL')).toBeVisible();
  await expect(form.locator('[name="pin"], [name="native_device"]')).toHaveCount(0);
  await form.locator('[name="guide_kind"]').selectOption('zap2it');
  await form.getByLabel('ZIP / postal code').fill('14850');
  const token = await page.evaluate(()=>localStorage.getItem('koko-client-web-auth-token'));
  if (process.env.KOKO_E2E_LIVE_ZAP2IT !== '1') await page.route('**/api/v1/live-tv/guide/providers?**',route=>route.fulfill({json:[{type:'OTA',name:'Local Over the Air',lineupId:'USA-lineupId-DEFAULT',headendId:'lineupId',device:'',timezone:'',location:''}]}));
  await form.getByRole('button',{name:'Find providers',exact:true}).click();
  await expect.poll(()=>form.locator('[name="guide_provider"] option').count(),{timeout:30000}).toBeGreaterThan(0);
  await expect(form.locator('[data-tv-provider-status]')).toContainText('Select your provider');
  await page.locator('#tv-source-kind').selectOption('native');
  await expect.poll(()=>form.locator('[name="region"] option').count()).toBeGreaterThan(1);
  await expect(form.locator('input[name="region"]')).toHaveCount(0);
  await form.getByRole('button',{name:'Find local tuners',exact:true}).click();
  await expect(form.locator('[data-tv-tuner-status]')).toContainText(/No tuners found|tuners? found\./);
  await expect(form.locator('[data-tv-tuner-status]')).not.toContainText('missing');
  await page.route('**/api/v1/live-tv/tuners', route => route.fulfill({ json:[
    {id:'/dev/dvb/adapter0',name:'Hauppauge test tuner',info:'DVB-T2'},
    {id:'/dev/dvb/adapter1',name:'Second test tuner',info:'DVB-T2'},
  ] }));
  await form.getByRole('button',{name:'Find local tuners',exact:true}).click();
  await form.getByRole('radio', {name:/Hauppauge test tuner/}).check();
  await expect(form.getByLabel('Tuner location', {exact:true})).toHaveValue('/dev/dvb/adapter0');
  await expect(form.locator('select[name="native_device"]')).toHaveCount(0);
  await page.screenshot({path:testInfo.outputPath('live-tv-settings.png'),fullPage:true});
  await form.getByLabel('Tuner location', {exact:true}).fill('/dev/dvb/manual');
  await expect(form.locator('input[type="radio"]:checked')).toHaveCount(0);
  const denied = await page.request.get('/api/v1/live-tv/discover/hdhomerun',{headers:{Authorization:'Bearer invalid'}});
  expect(denied.status()).toBe(401);
  expect(token).toBeTruthy();
});

test('the guide aligns all channels to one timeline with a program hero, source rail, and categories', async ({page},testInfo) => {
  const start = Math.floor(Date.now()/1800000)*1800;
  const channel = (id,source_id,name,number) => ({id,source_id,name,number,source_key:String(id),guide_key:String(id),group_name:'',enabled:true,available:true});
  const channels = [channel(101,10,'Local News','9.1'),channel(102,10,'Sports Network','13.6'),channel(201,20,'Cable Movies','1'),...Array.from({length:20},(_,i)=>channel(300+i,10,`News ${i+1}`,String(20+i)))];
  channels[0].logo='/api/v1/live-tv/channels/101/artwork';
  channels[1].logo='/api/v1/live-tv/channels/102/artwork';
  const cover='https://zpmc.tmsimg.com/assets/p13159645_b_v13_aa.jpg?w=165';
  const poster='https://zpmc.tmsimg.com/assets/poster_b_v13_aa.jpg?w=165';
  const logo='https://zpmc.tmsimg.com/h3/NowShowing/45738/station.png?w=55';
  const programs = [
    {id:1,source_id:10,guide_key:'101',title:'Afternoon News',starts_at:start-600,ends_at:start+1800,metadata:{description:'The stories that matter in your community, with headlines, weather, and local reporting.',categories:['News'],image:cover,poster,is_new:true}},
    {id:2,source_id:10,guide_key:'101',title:'The Classic Film',starts_at:start+1800,ends_at:start+7200,metadata:{description:'A feature-length movie with interviews from the cast.',categories:['Movie / drama'],is_premiere:true}},
    {id:3,source_id:10,guide_key:'102',title:'World of Sports',starts_at:start-600,ends_at:start+7200,metadata:{description:'Live coverage and analysis.',categories:['Sports event'],is_live:true}},
    {id:4,source_id:20,guide_key:'201',title:'Cable Movie Premiere',starts_at:start-600,ends_at:start+7200,metadata:{description:'A film from your cable lineup.',categories:['Movies'],is_premiere:true}},
    ...Array.from({length:20},(_,i)=>({id:1000+i,source_id:10,guide_key:String(300+i),title:`News program ${i+1}`,starts_at:start,ends_at:start+21600,metadata:{categories:['News']}})),
  ];
  await page.route('**/api/v1/live-tv/channels',route=>route.fulfill({json:channels}));
  await page.route('**/api/v1/live-tv/guide?**',route=>route.fulfill({json:{programs,stations:[{source_id:10,guide_key:'101',name:'Local News',number:'9.1',aliases:[],logo}],sources:[{id:10,name:'Antenna',kind:'hdhomerun'},{id:20,name:'Cable',kind:'nextpvr'}],preferences:{favorites:[],recent:[]}}}));
  await page.route('**/api/v1/live-tv/channels/101/artwork',route=>route.fulfill({status:404}));
  await page.route('**/api/v1/live-tv/channels/102/artwork',route=>route.fulfill({status:404}));
  await page.route('https://zpmc.tmsimg.com/**',route=>route.fulfill({contentType:'image/svg+xml',body:'<svg xmlns="http://www.w3.org/2000/svg" width="960" height="540"><defs><linearGradient id="g"><stop stop-color="#15394f"/><stop offset="1" stop-color="#244b43"/></linearGradient></defs><rect width="960" height="540" fill="url(#g)"/><text x="60" y="290" fill="#c7f5e0" font-size="70" font-family="sans-serif">Koko Live TV</text></svg>'}));
  await page.goto('/');
  await page.getByLabel('Username',{exact:true}).fill('playback-admin');
  await page.getByLabel('Password',{exact:true}).fill(process.env.KOKO_E2E_PASSWORD);
  await page.getByRole('button',{name:'Sign in',exact:true}).click();
  await expect(page.locator('#login-form')).toHaveCount(0);
  await page.goto('/live-tv');
  await expect(page.getByRole('heading',{name:'Afternoon News',exact:true})).toBeVisible();
  await expect(page.locator('#tv-guide-hero')).toContainText('The stories that matter');
  await expect(page.locator('#tv-guide-hero.home-feature')).toHaveAttribute('style',/w=960/);
  await expect(page.locator('.home-pinned-header > .home-feature')).toHaveCount(1);
  await expect(page.locator('#tv-guide-hero .tv-badge-new')).toHaveText('NEW');
  await expect(page.locator('#tv-guide-hero .tv-badge-live')).toHaveCount(0);
  await expect(page.locator('.tv-guide-row')).toHaveCount(22);
  await expect(page.locator('#tv-filter-panel')).toBeHidden();
  await expect(page.locator('#toggle-tv-filters')).toHaveAttribute('aria-expanded','false');
  await expect(page.locator('#tv-guide-hero .home-feature-poster')).toHaveAttribute('src',/poster_b_v13_aa.jpg\?w=960/);
  await expect.poll(()=>page.locator('#tv-guide-hero .tv-hero-channel-logo').evaluate(image=>image.naturalWidth)).toBeGreaterThan(0);
  await expect(page.locator('#tv-guide-hero .tv-hero-channel-logo')).toHaveAttribute('src',/w=320/);
  await expect(page.locator('#tv-guide-scroll')).toHaveCount(1);
  await expect(page.locator('[data-tv-select-channel="102"] .tv-channel-logo')).toBeHidden();
  const textPositions=await page.locator('.tv-channel-select > span:last-child').evaluateAll(labels=>labels.map(label=>label.getBoundingClientRect().left));
  expect(new Set(textPositions).size).toBe(1);
  await page.setViewportSize({width:390,height:844});
  const mobilePositions=await page.locator('.tv-channel-select > span:last-child').evaluateAll(labels=>labels.map(label=>label.getBoundingClientRect().left));
  expect(new Set(mobilePositions).size).toBe(1);
  await page.setViewportSize({width:1440,height:900});
  const guideEdges = () => page.locator('#tv-guide-scroll').evaluate(scroll=>{
    const box=selector=>scroll.querySelector(selector).getBoundingClientRect();
    const corner=box('.tv-guide-corner'),tick=box('.tv-guide-tick'),header=box('.tv-guide-time-row');
    const channel=box('.tv-guide-channel'),track=box('.tv-guide-track'),first=box('#tv-program-101-1'),next=box('#tv-program-101-2');
    const style=getComputedStyle(scroll.querySelector('#tv-program-101-1'));
    return {cornerEdge:Math.abs(corner.right-channel.right),timeEdge:Math.abs(tick.left-track.left),
      headerBottom:Math.abs(corner.bottom-tick.bottom),overflow:Math.max(corner.bottom,tick.bottom)-header.bottom,
      topGap:Math.abs(first.top-track.top),programGap:Math.abs(first.right-next.left),
      borders:[style.borderTopWidth,style.borderRightWidth,style.borderBottomWidth,style.borderLeftWidth],radius:style.borderRadius};
  });
  for(const fontSize of ['16px','24px']) {
    await page.evaluate(size=>{document.documentElement.style.fontSize=size;},fontSize);
    const edges=await guideEdges();
    for(const gap of [edges.cornerEdge,edges.timeEdge,edges.headerBottom,edges.overflow,edges.topGap,edges.programGap]) expect(gap).toBeLessThanOrEqual(1);
    expect(edges.borders).toEqual(['0px','1px','0px','0px']);
    expect(edges.radius).toBe('0px');
  }
  await page.evaluate(()=>{document.documentElement.style.fontSize='';});
  const alignment = await page.locator('#tv-guide-scroll').evaluate(scroll => {
    const header=scroll.querySelector('.tv-guide-tick'), program=scroll.querySelector('.tv-guide-program'), channel=scroll.querySelector('.tv-guide-channel');
    const before={header:header.getBoundingClientRect().left,program:program.getBoundingClientRect().left,channel:channel.getBoundingClientRect().left};
    scroll.scrollLeft=240;
    return {header:before.header-header.getBoundingClientRect().left,program:before.program-program.getBoundingClientRect().left,channel:before.channel-channel.getBoundingClientRect().left};
  });
  expect(alignment.header).toBe(240); expect(alignment.program).toBe(240); expect(alignment.channel).toBe(0);
  await page.locator('#tv-guide-scroll').evaluate(scroll=>{scroll.scrollLeft=0;});
  await page.screenshot({path:testInfo.outputPath('live-tv-timeline.png'),fullPage:true});
  await page.locator('#tv-program-101-1').focus();
  for(let step=0;step<14;step++) {
    await page.keyboard.press('ArrowDown');
    await expect(page.locator('.tv-guide-scroll :focus')).toBeInViewport();
  }
  expect(await page.locator('#tv-guide-scroll').evaluate(scroll=>scroll.scrollTop)).toBeGreaterThan(400);
  expect(await page.locator('#tv-guide-scroll').evaluate(scroll=>scroll.scrollLeft)).toBe(0);
  const selectedBounds = await page.locator('.tv-guide-scroll :focus').evaluate(control=>{
    const guide=control.closest('#tv-guide-scroll'),row=control.getBoundingClientRect(),bounds=guide.getBoundingClientRect();
    return {top:row.top,bottom:row.bottom,header:bounds.top+48,visibleBottom:Math.min(bounds.bottom,innerHeight)};
  });
  expect(selectedBounds.top).toBeGreaterThanOrEqual(selectedBounds.header-2);
  expect(selectedBounds.bottom).toBeLessThanOrEqual(selectedBounds.visibleBottom+2);
  await page.getByRole('button',{name:'Filters',exact:true}).click();
  await expect(page.locator('#tv-filter-panel')).toBeVisible();
  await page.locator('[data-tv-category="Sports"]').click();
  await expect(page.locator('.tv-guide-row')).toHaveCount(1);
  await expect(page.getByRole('heading',{name:'World of Sports',exact:true})).toBeVisible();
  await expect(page.locator('#tv-guide-hero .tv-badge-live')).toHaveText('LIVE');
  await page.locator('[data-tv-category="Movies"]').click();
  await expect(page.locator('.tv-guide-program.is-dimmed')).toHaveCount(1);
  await expect(page.getByRole('heading',{name:'The Classic Film',exact:true})).toBeVisible();
  await page.getByRole('navigation',{name:'TV sources',exact:true}).getByRole('button',{name:/Cable/}).click();
  await expect(page.locator('.tv-guide-row')).toHaveCount(1);
  await expect(page.getByRole('heading',{name:'Cable Movie Premiere',exact:true})).toBeVisible();
  await expect(page.locator('#tv-guide-hero .tv-badge-premiere')).toHaveText('PREMIERE');
  await page.getByRole('button',{name:/Filters/}).click();
  await expect(page.locator('#tv-filter-panel')).toBeHidden();
  await page.setViewportSize({width:390,height:844});
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  await page.screenshot({path:testInfo.outputPath('live-tv-timeline-mobile.png'),fullPage:true});
});
