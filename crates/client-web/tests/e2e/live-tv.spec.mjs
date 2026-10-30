/** Exercises real IPTV import, browser decoding, tuner release, and timed DVR capture. */
import { test, expect } from '@playwright/test';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { stat } from 'node:fs/promises';

test('IPTV imports its guide, plays live video, releases the tuner, and saves a timed recording', async ({ page }, testInfo) => {
  test.setTimeout(180000);
  const children = new Set();
  let activeStreams = 0;
  let sourceId;
  let headers;
  const stamp = seconds => new Date(seconds * 1000).toISOString().slice(0, 19).replaceAll(/[-T:]/g, '') + ' +0000';
  const fixture = createServer((request, response) => {
    const base = `http://127.0.0.1:${fixture.address().port}`;
    if (request.url === '/tv.m3u') {
      response.end(`#EXTM3U\n#EXTINF:-1 tvg-id="test-tv" tvg-chno="7.1" group-title="News",Live Test TV\n${base}/stream.ts\n#EXTINF:-1 tvg-id="hls-tv" tvg-chno="7.2",HLS Test TV\n${base}/master.m3u8\n`);
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
        '-c:v', 'mpeg4', '-q:v', '5', '-g', '25', '-c:a', 'aac', '-f', 'mpegts', ...(request.url === '/stream.ts' ? [] : ['-t', '2']), 'pipe:1'],
      { windowsHide: true, stdio: ['ignore', 'pipe', 'ignore'] });
      children.add(ffmpeg);
      response.setHeader('Content-Type', 'video/mp2t');
      ffmpeg.stdout.pipe(response);
      response.once('close', () => { activeStreams -= 1; ffmpeg.kill(); children.delete(ffmpeg); });
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
    await expect(page.getByText('Live Test News', { exact: true })).toBeVisible();
    await page.screenshot({ path: testInfo.outputPath('live-tv-guide.png') });
    const channels = await (await page.request.get('/api/v1/live-tv/channels', { headers })).json();
    const hls = channels.find(c => c.source_id === sourceId && c.name === 'HLS Test TV');
    await page.locator(`[data-tv-play="${hls.id}"]`).click();
    await expect.poll(() => page.locator('.live-tv-player video').evaluate(video => video.readyState >= 2 && video.currentTime > 0.1)).toBeTruthy();
    await page.getByRole('button', { name: 'Close TV player' }).click();
    await expect.poll(() => activeStreams).toBe(0);
    await page.locator(`[data-tv-play="${channel.id}"]`).click();
    await expect.poll(() => page.locator('.live-tv-player video').evaluate(video => video.readyState >= 2 && video.currentTime > 0.2)).toBeTruthy();
    expect(activeStreams).toBe(1);
    await page.getByRole('button', { name: 'Close TV player' }).click();
    await expect.poll(() => activeStreams).toBe(0);

    const recordingDirectory = testInfo.outputPath('dvr-storage');
    const storage = await page.request.put('/api/v1/live-tv/dvr/settings', { headers, data: { recording_dir: recordingDirectory } });
    expect(storage.ok()).toBeTruthy();
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
    await expect.poll(() => page.locator('.live-tv-player video').evaluate(video => video.readyState >= 2 && video.currentTime > 0.1)).toBeTruthy();
    await page.getByRole('button', { name: 'Close TV player' }).click();
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
