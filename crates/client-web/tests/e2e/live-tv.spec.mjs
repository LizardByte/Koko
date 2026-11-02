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
      response.end(`#EXTM3U\n#EXTINF:-1 tvg-id="test-tv" tvg-chno="7.1" tvg-logo="${base}/missing-logo.svg" group-title="News",Live Test TV\n${base}/stream.ts\n#EXTINF:-1 tvg-id="hls-tv" tvg-chno="7.2",HLS Test TV\n${base}/master.m3u8\n#EXTINF:-1 tvg-id="alternate-tv" tvg-chno="7.3",Alternate TV\n${base}/stream.ts\n`);
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
      response.end(`<tv><channel id="test-tv"><display-name>Live Test TV</display-name><icon src="${base}/logo.svg"/></channel><programme channel="test-tv" start="${stamp(now - 600)}" stop="${stamp(now + 3600)}"><title>Live Test News</title><desc>Local IPTV guide fixture.</desc><category>News</category><new/></programme></tv>`);
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
    await page.locator(`#tv-channel-select-${hls.id}`).click();
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
    await page.locator(`.tv-guide-program[data-tv-select-channel="${channel.id}"]`).first().click();
    await expect.poll(() => page.locator('#media-player').evaluate(video => video.readyState >= 2 && video.currentTime > 0.2),{timeout:45000}).toBeTruthy();
    const viewport = await page.locator('.media-player-overlay').boundingBox();
    expect(viewport.x).toBe(0); expect(viewport.y).toBe(0); expect(viewport.height).toBe(900);
    await page.getByRole('button',{name:'Pause',exact:true}).click();
    await page.getByRole('button',{name:'Play',exact:true}).click();
    await expect(page.locator('.player-channel-heading h2')).toHaveText('Live Test TV');
    await expect(page.locator('#player-tv-program')).toContainText('Live Test News');
    await expect(page.locator('#player-tv-program')).toContainText('Local IPTV guide fixture.');
    await expect.poll(()=>page.locator('.player-channel-logo').evaluate(image=>image.naturalWidth)).toBeGreaterThan(0);
    await expect(page.locator('.player-channel-logo')).toHaveAttribute('src',/\/logo.svg$/);
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
    await expect(page.getByRole('button',{name:'Go live',exact:true})).toHaveCount(0);
    const previousSeekSource=await page.locator('#media-player').getAttribute('src');
    await page.getByRole('button',{name:'Skip to end',exact:true}).click();
    await expect(page.locator('#media-player')).not.toHaveAttribute('src',previousSeekSource);
    const latestBuffer=await (await page.request.get(`/api/v1/live-tv/sessions/${bufferId}`,{headers})).json();
    const newestPosition=await page.locator('#media-player').evaluate(node=>Number(new URL(node.src).searchParams.get('start_ms')));
    expect(latestBuffer.end_ms-newestPosition).toBeGreaterThanOrEqual(5000);
    expect(latestBuffer.end_ms-newestPosition).toBeLessThan(11000);
    await expect.poll(()=>page.locator('#media-player').evaluate(node=>node.readyState>=2),{timeout:20000}).toBeTruthy();
    expect(await page.locator('#media-player').evaluate(node=>node.volume)).toBe(0.27);
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
    const browsingVideo=await page.locator('#media-player').elementHandle();
    const browsingSource=await browsingVideo.getAttribute('src');
    await page.getByRole('button',{name:'Channel guide',exact:true}).click();
    const playerGuide=page.getByRole('complementary',{name:'Channel guide',exact:true});
    await expect(playerGuide).toBeVisible();
    const overlappingControls=await page.locator('.player-bottom-controls button').evaluateAll(buttons=>{
      const bounds=buttons.map(button=>button.getBoundingClientRect()).filter(rect=>rect.width && rect.height);
      return bounds.some((a,i)=>bounds.slice(i+1).some(b=>Math.min(a.right,b.right)-Math.max(a.left,b.left)>1 && Math.min(a.bottom,b.bottom)-Math.max(a.top,b.top)>1));
    });
    expect(overlappingControls).toBe(false);
    const beforeBrowsing=await browsingVideo.evaluate(node=>node.currentTime);
    await playerGuide.getByRole('searchbox',{name:'Find a channel',exact:true}).fill('Live Test');
    await expect(playerGuide.locator('[data-player-tv-channel]:visible')).toHaveCount(1);
    await page.waitForTimeout(1500);
    expect(await browsingVideo.evaluate(node=>node===document.querySelector('#media-player'))).toBe(true);
    expect(await browsingVideo.evaluate(node=>node.currentTime)).toBeGreaterThan(beforeBrowsing+.7);
    await expect(page.locator('.tv-preview-player')).toHaveCount(0);
    await page.screenshot({path:testInfo.outputPath('live-tv-player-guide.png')});
    await playerGuide.getByRole('button',{name:'Watch Live Test TV live',exact:true}).click();
    await expect.poll(()=>page.locator('#media-player').getAttribute('src'),{timeout:45000}).not.toBe(browsingSource);
    await expect.poll(()=>page.locator('#media-player').evaluate(node=>node.readyState>=2),{timeout:45000}).toBeTruthy();
    await expect(playerGuide).toBeVisible();
    await expect(page.locator('#player-tv-program')).toContainText('Live Test News');
    await expect.poll(()=>page.locator('.player-channel-logo').evaluate(image=>image.naturalWidth)).toBeGreaterThan(0);
    await page.screenshot({path:testInfo.outputPath('live-tv-player-details.png')});
    await page.setViewportSize({width:390,height:844});
    await expect(playerGuide).toBeVisible();
    await expect.poll(()=>page.locator('#player-tv-guide-channels').evaluate(list=>list.clientHeight)).toBeGreaterThan(50);
    await expect.poll(async()=>{
      const guideBounds=await playerGuide.boundingBox(),progressBounds=await page.locator('#player-progress').boundingBox();
      return guideBounds.y+guideBounds.height<=progressBounds.y;
    }).toBeTruthy();
    await page.screenshot({path:testInfo.outputPath('live-tv-player-guide-mobile.png')});
    await page.setViewportSize({width:1440,height:900});
    await page.getByRole('button',{name:'Close channel guide',exact:true}).click();
    await expect(playerGuide).toBeHidden();
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

test('a 4402-channel guide stays bounded while scrolling, navigating, searching, and jumping to the end',async({page},testInfo)=>{
  test.setTimeout(120000);
  const start=Math.floor(Date.now()/1800000)*1800;
  const channels=Array.from({length:4402},(_,i)=>({id:i+1,source_id:1,source_key:String(i),guide_key:String(i),name:`Channel ${i+1}`,number:String(i+1),group_name:'',enabled:true,available:true}));
  const programs=channels.flatMap(c=>Array.from({length:4},(_,i)=>({id:c.id*10+i,source_id:1,guide_key:c.guide_key,title:`Channel ${c.id} program ${i}`,starts_at:start+i*5400,ends_at:start+(i+1)*5400,metadata:{description:`Program details for channel ${c.id}`,categories:['News']}})));
  await page.route('**/api/v1/live-tv/channels',route=>route.fulfill({json:channels}));
  await page.route('**/api/v1/live-tv/guide?**',route=>route.fulfill({json:{programs,stations:[],sources:[{id:1,name:'Large lineup',kind:'m3u'}]}}));
  await page.goto('/');await page.getByLabel('Username',{exact:true}).fill('playback-admin');await page.getByLabel('Password',{exact:true}).fill(process.env.KOKO_E2E_PASSWORD);await page.getByRole('button',{name:'Sign in',exact:true}).click();await expect(page.locator('#login-form')).toHaveCount(0);
  await page.goto('/live-tv');
  const scroll=page.locator('#tv-guide-scroll');
  await expect(scroll).toHaveAttribute('data-tv-channel-count','4402');
  expect(await page.locator('.tv-guide-row').count()).toBeLessThan(30);
  await page.locator('#tv-program-1-10').focus();
  const mounted=await page.locator('#tv-channel-row-1').elementHandle();
  await page.keyboard.press('ArrowDown');
  await expect(page.locator('#tv-program-2-20')).toBeFocused();
  expect(await mounted.evaluate(node=>node.isConnected)).toBe(true);
  await expect(page.locator('#tv-guide-hero h2')).toHaveText('Channel 2 program 0');
  await page.evaluate(()=>{
    const guide=document.querySelector('#tv-guide-scroll');window.tvGuideIdentity=guide;
    for(let i=0;i<50;i++) document.activeElement.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowDown',bubbles:true}));
  });
  await expect(page.locator('#tv-program-52-520')).toBeFocused();
  expect(await page.evaluate(()=>window.tvGuideIdentity===document.querySelector('#tv-guide-scroll'))).toBe(true);
  expect(await page.locator('.tv-guide-row').count()).toBeLessThan(30);
  await scroll.evaluate(node=>{node.scrollTop=2200*88;node.scrollLeft=240;});
  await expect(page.locator('#tv-program-2201-22010')).toBeAttached();
  await page.locator('#tv-program-2201-22010').focus();
  await page.keyboard.press('ArrowDown');await expect(page.locator('#tv-program-2202-22020')).toBeFocused();
  await page.keyboard.press('End');await expect(page.locator('#tv-program-4402-44020')).toBeFocused();
  expect(await page.locator('.tv-guide-row').count()).toBeLessThan(30);
  await page.keyboard.press('Home');await expect(page.locator('#tv-program-1-10')).toBeFocused();
  await page.getByRole('button',{name:'Filters',exact:true}).click();
  await page.getByRole('searchbox',{name:'Search channels or programs',exact:true}).fill('Channel 4402');
  await page.locator('#tv-filter-form').getByRole('button',{name:'Search',exact:true}).click();
  await expect(page.locator('.tv-guide-row')).toHaveCount(1);
  await expect(page.locator('#tv-guide-hero h2')).toHaveText('Channel 4402 program 0');
  await page.getByRole('searchbox',{name:'Search channels or programs',exact:true}).fill('');
  await page.locator('#tv-filter-form').getByRole('button',{name:'Search',exact:true}).click();
  expect(await page.locator('.tv-guide-row').count()).toBeLessThan(30);
  await scroll.screenshot({path:testInfo.outputPath('large-guide-virtual-rows.png')});
});

test('source settings prefill connections, report imports, isolate channel editors, and persist guide groups',async({page})=>{
  test.setTimeout(150000);
  const stamp=seconds=>new Date(seconds*1000).toISOString().slice(0,19).replaceAll(/[-T:]/g,'')+' +0000';
  const fixture=createServer((request,response)=>{
    const now=Math.floor(Date.now()/1000),bridge=request.url.startsWith('/bridge');
    if (request.url.endsWith('.m3u')) {
      response.end(`#EXTM3U\n#EXTINF:-1 tvg-id="station" tvg-chno="${bridge?'5167':'100'}",${bridge?'ESPN bridge':'Regular channel'}\nhttp://example.com/stream.ts\n`);
    } else {
      response.end(`<tv><channel id="station"><display-name>${bridge?'ESPN bridge':'Regular channel'}</display-name></channel><programme channel="station" start="${stamp(now-600)}" stop="${stamp(now+7200)}"><title>${bridge?'ESPN program':'Regular program'}</title></programme></tv>`);
    }
  });
  await new Promise(done=>fixture.listen(0,'127.0.0.1',done));
  let headers;const ids=[];
  try {
    await page.goto('/');await page.getByLabel('Username',{exact:true}).fill('playback-admin');await page.getByLabel('Password',{exact:true}).fill(process.env.KOKO_E2E_PASSWORD);await page.getByRole('button',{name:'Sign in',exact:true}).click();await expect(page.locator('#login-form')).toHaveCount(0);
    const token=await page.evaluate(()=>localStorage.getItem('koko-client-web-auth-token'));headers={Authorization:`Bearer ${token}`};
    const base=`http://127.0.0.1:${fixture.address().port}`;
    for(const [name,prefix,capacity] of [['Regular feed','regular',10],['PrismCast bridge','bridge',1]]) {
      const response=await page.request.post('/api/v1/live-tv/sources',{headers,data:{name,kind:'m3u',capacity,config:{url:`${base}/${prefix}.m3u`,guide_kind:'xmltv',xmltv_url:`${base}/${prefix}.xml`,artwork_provider:'guide',refresh_hours:12}}});
      expect(response.ok()).toBeTruthy();ids.push((await response.json()).id);
    }
    await expect.poll(async()=>{
      const sources=await(await page.request.get('/api/v1/live-tv/sources',{headers})).json();
      return sources.filter(s=>ids.includes(s.id)&&s.channel_count===1&&s.playlist_bytes>0&&s.guide_bytes>0).length;
    },{timeout:65000}).toBe(2);
    await page.goto('/settings/live-tv');
    const regular=page.locator(`[data-tv-source-card="${ids[0]}"]`),bridge=page.locator(`[data-tv-source-card="${ids[1]}"]`);
    await expect(regular).toContainText('1 channel');await expect(regular).toContainText('Last imported: M3U');await expect(regular).toContainText('XMLTV');
    await regular.locator(':scope > summary').click();await bridge.locator(':scope > summary').click();
    await regular.getByText('Edit connection settings',{exact:true}).click();
    const editor=regular.locator('[data-tv-replace-source]');
    await expect(editor.getByLabel('IPTV playlist URL',{exact:true})).toHaveValue(`${base}/regular.m3u`);
    await expect(editor.getByLabel('XMLTV URL',{exact:true})).toHaveValue(`${base}/regular.xml`);
    await expect(editor.getByRole('combobox',{name:'Program artwork',exact:true})).toHaveValue('guide');
    await expect(editor.getByLabel('Refresh interval (hours)',{exact:true})).toHaveValue('12');
    await editor.getByLabel('Refresh interval (hours)',{exact:true}).fill('18');
    await regular.locator(':scope > summary').click();await expect(regular).toHaveJSProperty('open',false);
    await regular.locator(':scope > summary').click();await expect(editor.getByLabel('Refresh interval (hours)',{exact:true})).toHaveValue('18');
    await regular.getByText('Channel visibility and guide mapping',{exact:true}).click();
    await expect(editor.getByLabel('XMLTV URL',{exact:true})).toHaveValue(`${base}/regular.xml`);
    await expect(editor.getByLabel('Refresh interval (hours)',{exact:true})).toHaveValue('18');
    await editor.getByRole('button',{name:'Save connection settings',exact:true}).click();
    await expect(editor).toHaveCount(0);
    if (!(await regular.locator('[data-tv-mapping-source]').evaluate(node=>node.open))) await regular.getByText('Channel visibility and guide mapping',{exact:true}).click();
    await expect(regular.locator('[data-tv-channel-update]')).toHaveCount(1);await expect(bridge.locator('[data-tv-channel-update]')).toHaveCount(0);
    await bridge.getByText('Channel visibility and guide mapping',{exact:true}).click();
    await expect(bridge.locator('[data-tv-channel-update]')).toHaveCount(1);await expect(regular.locator('[data-tv-channel-update]')).toHaveCount(0);
    await page.getByText('Add a source group',{exact:true}).click();
    const group=page.locator('[data-tv-group-form=""]');await group.getByLabel('Group name',{exact:true}).fill('Combined FastChannels');
    for(const id of ids) await group.locator(`input[name="source_id"][value="${id}"]`).check();
    await group.getByRole('button',{name:'Create group',exact:true}).click();
    await expect(page.getByText('Combined FastChannels',{exact:true})).toBeVisible();
    await page.goto('/live-tv');
    const tab=page.locator('[data-tv-source-group][aria-label^="Combined FastChannels,"]');
    if(await tab.count()) await tab.click();
    await expect(page.locator('#tv-guide-scroll')).toHaveAttribute('data-tv-channel-count','2');
    const bridgeChannel=(await(await page.request.get('/api/v1/live-tv/channels',{headers})).json()).find(c=>c.source_id===ids[1]);
    await page.locator(`[data-tv-select-channel="${bridgeChannel.id}"]`).first().focus();
    await expect(page.locator('#tv-guide-hero h2')).toHaveText('ESPN program');
    await page.reload();await expect(page.locator('#tv-guide-scroll')).toHaveAttribute('data-tv-channel-count','2');
    const saved=await(await page.request.get(`/api/v1/live-tv/sources/${ids[0]}/connection`,{headers})).json();
    expect(saved.source.config.refresh_hours).toBe(18);expect(saved.source.config.url).toBe(`${base}/regular.m3u`);
    const configured=await(await page.request.get('/api/v1/live-tv/sources',{headers})).json();
    expect(configured.find(s=>s.id===ids[0]).capacity).toBe(10);expect(configured.find(s=>s.id===ids[1]).capacity).toBe(1);
    await page.goto('/settings/live-tv');await page.getByText('Combined FastChannels',{exact:true}).click();
    const savedGroup=page.locator('[data-tv-group-form]').filter({has:page.locator('[data-tv-group-delete]')}).filter({has:page.locator('input[value="Combined FastChannels"]')});
    await savedGroup.getByLabel('Group name',{exact:true}).fill('FastChannels and bridge');
    await savedGroup.getByRole('button',{name:'Save group',exact:true}).click();
    await expect(page.getByText('FastChannels and bridge',{exact:true})).toBeVisible();
    const renamed=page.locator('[data-tv-group-form]').filter({has:page.locator('input[value="FastChannels and bridge"]')});
    await renamed.getByRole('button',{name:'Delete group',exact:true}).click();
    await expect(page.getByText('FastChannels and bridge',{exact:true})).toHaveCount(0);
    expect((await(await page.request.get('/api/v1/live-tv/sources',{headers})).json()).filter(s=>ids.includes(s.id))).toHaveLength(2);
  } finally {
    for(const id of ids) if(headers) await page.request.put(`/api/v1/live-tv/sources/${id}`,{headers,data:{enabled:false,capacity:1}}).catch(()=>undefined);
    await new Promise(done=>fixture.close(done));
  }
});

test('large Live TV Settings loads mapping choices on demand and saves the selected station', async ({page},testInfo) => {
  const stations=Array.from({length:5200},(_,i)=>({source_id:1,guide_key:`station-${i}`,name:`Station ${i}`,number:String(i),aliases:[]}));
  const channels=[1,2].map(id=>({id,source_id:1,source_key:String(id),name:`Channel ${id}`,number:String(id),group_name:'',guide_key:`station-${id}`,guide_override:id===1?'station-5':null,enabled:true,available:true}));
  const updates=[];
  await page.route('**/api/v1/live-tv/setup/channels',route=>route.fulfill({json:channels}));
  await page.route('**/api/v1/live-tv/guide?**',route=>route.fulfill({json:{programs:[],stations}}));
  await page.route('**/api/v1/live-tv/sources',route=>route.fulfill({json:[{id:1,name:'Large lineup',kind:'m3u',enabled:true,capacity:1,next_refresh:0}]}));
  await page.route('**/api/v1/live-tv/channels/1',route=>{
    const update=route.request().postDataJSON(); updates.push(update); channels[0].guide_override=update.guide_override;
    return route.fulfill({status:204});
  });
  await page.goto('/');
  await page.getByLabel('Username',{exact:true}).fill('playback-admin');
  await page.getByLabel('Password',{exact:true}).fill(process.env.KOKO_E2E_PASSWORD);
  await page.getByRole('button',{name:'Sign in',exact:true}).click();
  await expect(page.locator('#login-form')).toHaveCount(0);
  await page.goto('/settings/live-tv');
  const mapping=page.locator('[data-tv-channel-update="1"]');
  await page.locator('[data-tv-source-card="1"] > summary').click();
  await page.getByText('Channel visibility and guide mapping',{exact:true}).click();
  const select=mapping.getByRole('combobox',{name:'Guide station',exact:true});
  await expect(page.locator('[data-tv-guide-source] option')).toHaveCount(3);
  await select.click();
  await expect(select.locator('option')).toHaveCount(5201);
  await select.press('Escape');
  await select.selectOption('station-5199');
  await mapping.getByRole('button',{name:'Save',exact:true}).click();
  await expect.poll(()=>updates).toEqual([{enabled:true,guide_override:'station-5199'}]);
  await expect(select).toHaveValue('station-5199');
  await expect(page.locator('[data-tv-guide-source] option')).toHaveCount(3);
  await expect(mapping).toBeVisible();
  await mapping.screenshot({path:testInfo.outputPath('large-lineup-channel-mapping.png')});
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
  await expect(form.getByRole('combobox', {name:'Program artwork',exact:true})).toHaveValue('automatic');
  await form.getByRole('combobox', {name:'Program artwork',exact:true}).selectOption('guide');
  await expect(form.getByRole('combobox', {name:'Guide source',exact:true})).toHaveValue('automatic');
  await form.getByRole('combobox', {name:'Program artwork',exact:true}).selectOption('tvdb');
  await expect(form.getByRole('combobox', {name:'Guide source',exact:true})).toHaveValue('automatic');
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

test('the guide preserves its time column across channels and advances expired slots without stealing focus',async({page})=>{
  const start=Math.floor(Date.now()/1800000)*1800;
  await page.clock.install({time:start*1000+1790000});
  await page.clock.pauseAt(start*1000+1791000);
  const channels=Array.from({length:4},(_,i)=>({id:501+i,source_id:50,source_key:String(i),guide_key:String(i),name:`Navigation ${i+1}`,number:String(i+1),group_name:'',enabled:true,available:true}));
  const ranges=[[[0,1800],[1800,3600],[3600,21600]],[[0,21600]],[[0,900],[900,1800],[1800,2700],[2700,3600],[3600,21600]],[]];
  const programs=ranges.flatMap((row,index)=>row.map(([from,to],p)=>({id:100+index*10+p,source_id:50,guide_key:String(index),title:`Program ${index+1}.${p+1}`,starts_at:start+from,ends_at:start+to,metadata:{description:'Navigation fixture'}})));
  await page.route('**/api/v1/live-tv/channels',route=>route.fulfill({json:channels}));
  await page.route('**/api/v1/live-tv/guide?**',route=>route.fulfill({json:{programs,stations:[],sources:[{id:50,name:'Navigation source',kind:'m3u'}],preferences:{favorites:[],recent:[]}}}));
  await page.goto('/');
  await page.getByLabel('Username',{exact:true}).fill('playback-admin');
  await page.getByLabel('Password',{exact:true}).fill(process.env.KOKO_E2E_PASSWORD);
  await page.getByRole('button',{name:'Sign in',exact:true}).click();
  await expect(page.locator('#login-form')).toHaveCount(0);
  await page.goto('/live-tv');
  await page.clock.runFor(100);
  await page.locator('#tv-program-501-101').focus();
  for(let repeat=0;repeat<3;repeat++) {
    await page.keyboard.press('ArrowDown');await expect(page.locator('#tv-program-502-110')).toBeFocused();
    await page.keyboard.press('ArrowDown');await expect(page.locator('#tv-program-503-122')).toBeFocused();
    await page.keyboard.press('ArrowUp');await expect(page.locator('#tv-program-502-110')).toBeFocused();
    await page.keyboard.press('ArrowUp');await expect(page.locator('#tv-program-501-101')).toBeFocused();
  }
  const focusedStyle=await page.locator(':focus').evaluate(control=>{const style=getComputedStyle(control);return{offset:style.outlineOffset,width:style.outlineWidth};});
  expect(focusedStyle).toEqual({offset:'-2px',width:'2px'});
  await page.evaluate(()=>{document.body.dataset.inputMode='gamepad';});
  await expect(page.locator(':focus')).toHaveCSS('outline-offset','-2px');
  await page.keyboard.press('ArrowRight');await expect(page.locator('#tv-program-501-102')).toBeFocused();
  await page.locator('#tv-guide-scroll').evaluate(scroll=>{scroll.scrollLeft=240;});
  await page.keyboard.press('ArrowDown');await expect(page.locator('#tv-program-502-110')).toBeFocused();
  expect(await page.locator('#tv-guide-scroll').evaluate(scroll=>scroll.scrollLeft)).toBe(240);
  await page.keyboard.press('ArrowDown');await expect(page.locator('#tv-program-503-124')).toBeFocused();
  await page.keyboard.press('ArrowUp');await page.keyboard.press('ArrowUp');
  await expect(page.locator('#tv-program-501-102')).toBeFocused();
  await page.clock.fastForward(11000);
  await expect(page.locator('#tv-guide-scroll')).toHaveAttribute('data-tv-window-start',String(start+1800));
  await expect(page.locator('#tv-program-501-100')).toHaveCount(0);
  await expect(page.locator('#tv-program-501-102')).toBeFocused();
  expect(await page.locator('#tv-guide-scroll').evaluate(scroll=>scroll.scrollLeft)).toBe(120);
  await page.locator('#tv-program-501-101').focus();
  await page.clock.fastForward(1800000);
  await expect(page.locator('#tv-guide-scroll')).toHaveAttribute('data-tv-window-start',String(start+3600));
  await expect(page.locator('#tv-program-501-101')).toHaveCount(0);
  await expect(page.locator('#tv-program-501-102')).toBeFocused();
  await expect(page.getByRole('button',{name:'Earlier programs',exact:true})).toBeDisabled();
  await page.getByRole('button',{name:'Later programs',exact:true}).click();
  await expect(page.locator('#tv-guide-scroll')).toHaveAttribute('data-tv-window-start',String(start+3600+21600));
  const future=await page.locator('#tv-guide-scroll').getAttribute('data-tv-window-start');
  await page.clock.fastForward(1800000);
  await expect(page.locator('#tv-guide-scroll')).toHaveAttribute('data-tv-window-start',future);
  await expect(page.getByRole('button',{name:'Earlier programs',exact:true})).toBeEnabled();
  await page.getByRole('button',{name:'Now',exact:true}).click();
  await expect(page.locator('#tv-guide-scroll')).toHaveAttribute('data-tv-window-start',String(start+5400));
});

test('the guide in the player centers playback controls and reveals the playing channel',async({page},testInfo)=>{
  const channels=Array.from({length:40},(_,i)=>({id:1001+i,source_id:90,source_key:String(i),guide_key:String(i),name:`Fixture ${i+1}`,number:String(i+1),group_name:'',enabled:true,available:true}));
  await page.route('**/api/v1/live-tv/channels',route=>route.fulfill({json:channels}));
  await page.route('**/api/v1/live-tv/guide?**',route=>route.fulfill({json:{programs:[],stations:[],sources:[{id:90,name:'Guide layout fixture',kind:'m3u'}],preferences:{favorites:[],recent:[]}}}));
  await page.goto('/');
  await page.getByLabel('Username',{exact:true}).fill('playback-admin');
  await page.getByLabel('Password',{exact:true}).fill(process.env.KOKO_E2E_PASSWORD);
  await page.getByRole('button',{name:'Sign in',exact:true}).click();
  await expect(page.locator('#login-form')).toHaveCount(0);
  const token=await page.evaluate(()=>localStorage.getItem('koko-client-web-auth-token'));
  const headers={Authorization:`Bearer ${token}`};
  const items=await (await page.request.get('/api/v1/items',{headers})).json();
  const sample=items.find(item=>item.display_title==='Playback Sample');
  const sampleResponse=await page.request.get(`/api/v1/items/${sample.id}/stream`,{headers});
  expect(sampleResponse.ok()).toBeTruthy();
  const sampleVideo=await sampleResponse.body();
  let playing=1040;
  const buffer=()=>({session_id:'layout-buffer',channel_id:playing,title:`Fixture ${playing-1000}`,start_ms:0,end_ms:6000,finished:true,can_keep:true});
  await page.route('**/api/v1/live-tv/sessions',route=>{
    playing=route.request().postDataJSON().channel_id;
    return route.fulfill({json:buffer()});
  });
  await page.route('**/api/v1/live-tv/sessions/layout-buffer**',route=>{
    if(new URL(route.request().url()).pathname.endsWith('/stream')) return route.fulfill({contentType:'video/mp4',body:sampleVideo});
    return route.request().method()==='DELETE' ? route.fulfill({status:204}) : route.fulfill({json:buffer()});
  });
  await page.goto('/live-tv');
  await page.locator('[data-tv-play="1040"]').click();
  await expect.poll(()=>page.locator('#media-player').evaluate(video=>video.readyState>=2 && video.currentTime>.1)).toBeTruthy();
  const video=await page.locator('#media-player').elementHandle();
  const mainScroll=await page.locator('.main-shell').evaluate(main=>main.scrollTop);
  const guide=page.getByRole('complementary',{name:'Channel guide',exact:true});
  const revealCurrent=async id=>{
    await expect(guide.locator(`[data-player-tv-channel="${id}"]`)).toBeFocused();
    const visible=await guide.locator(`[data-player-tv-channel="${id}"]`).evaluate(channel=>{
      const row=channel.getBoundingClientRect(),list=channel.closest('#player-tv-guide-channels').getBoundingClientRect();
      return row.top>=list.top-1 && row.bottom<=list.bottom+1;
    });
    expect(visible).toBe(true);
  };
  const centered=async()=>{
    const videoBounds=await page.locator('#media-player').boundingBox(),playBounds=await page.locator('#player-play-toggle-small').boundingBox();
    expect(Math.abs(playBounds.x+playBounds.width/2-videoBounds.x-videoBounds.width/2)).toBeLessThan(1);
  };
  await page.getByRole('button',{name:'Channel guide',exact:true}).click();
  await revealCurrent(1040);await centered();
  expect(await video.evaluate(node=>node===document.querySelector('#media-player'))).toBe(true);
  expect(await page.locator('.main-shell').evaluate(main=>main.scrollTop)).toBe(mainScroll);
  await page.screenshot({path:testInfo.outputPath('centered-player-guide.png')});
  await page.getByRole('button',{name:'Close channel guide',exact:true}).click();
  await page.locator('#player-tv-guide-channels').evaluate(list=>{list.scrollTop=0;});
  await page.getByRole('button',{name:'Channel guide',exact:true}).click();
  await revealCurrent(1040);
  await guide.getByRole('searchbox',{name:'Find a channel',exact:true}).fill('Fixture 1');
  await page.getByRole('button',{name:'Close channel guide',exact:true}).click();
  await page.getByRole('button',{name:'Channel guide',exact:true}).click();
  await revealCurrent(1040);
  await expect(guide.getByRole('searchbox',{name:'Find a channel',exact:true})).toHaveValue('');
  await guide.getByRole('button',{name:'Watch Fixture 39 live',exact:true}).click();
  await expect(guide.locator('[data-player-tv-channel="1039"]')).toHaveAttribute('aria-pressed','true');
  await expect.poll(()=>guide.locator('[data-player-tv-channel="1039"]').evaluate(channel=>{
    const row=channel.getBoundingClientRect(),list=channel.closest('#player-tv-guide-channels').getBoundingClientRect();
    return row.top>=list.top-1 && row.bottom<=list.bottom+1;
  })).toBeTruthy();
  await centered();
  await page.setViewportSize({width:390,height:844});
  await page.getByRole('button',{name:'Close channel guide',exact:true}).click();
  await page.getByRole('button',{name:'Channel guide',exact:true}).click();
  await expect(guide.locator('[data-player-tv-channel="1039"]')).toBeInViewport();
  await centered();
});
