/** Exercises native browser decoding, server conversion, recovery, progress, and session ownership. */
import { test, expect } from '@playwright/test';
import { rename } from 'node:fs/promises';
import { dirname, basename, resolve } from 'node:path';
import { tmpdir } from 'node:os';

const media = (page) => page.locator('#media-player');
const error = (page) => page.locator('.media-player-shell.has-media-error .player-error-indicator');

test.beforeEach(async ({ page }) => {
  await page.goto('/');
  await page.getByLabel('Username', { exact: true }).fill('playback-admin');
  await page.getByLabel('Password', { exact: true }).fill(process.env.KOKO_E2E_PASSWORD);
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await expect(page.locator('#login-form')).toHaveCount(0);
});

async function headers(page) {
  const token = await page.evaluate(() => localStorage.getItem('koko-client-web-auth-token'));
  return { Authorization: `Bearer ${token}` };
}

async function openItem(page, title = 'Playback Sample') {
  const response = await page.request.get('/api/v1/items', { headers: await headers(page) });
  expect(response.ok()).toBeTruthy();
  const item = (await response.json()).find((item) => item.display_title === title);
  expect(item).toBeTruthy();
  await page.goto(`/items/${item.id}`);
  await expect(page.locator('[data-play-selected-item-start-ms]').first()).toBeVisible();
  return item;
}

async function sessionAfter(page, action) {
  const pending = page.waitForResponse((response) => response.url().endsWith('/api/v1/sessions') && response.request().method() === 'POST');
  await action();
  const response = await pending;
  expect(response.ok()).toBeTruthy();
  return response.json();
}

async function start(page) {
  return sessionAfter(page, () => page.locator('[data-play-selected-item-start-ms]').first().click());
}

async function playing(page) {
  await expect.poll(() => media(page).evaluate((player) => player.readyState >= 2 && !player.paused && player.currentTime > 0.1)).toBeTruthy();
  await expect(error(page)).toHaveCount(0);
}

async function seek(page, fraction) {
  await page.locator('.media-player-shell').hover();
  const box = await page.locator('#player-progress').boundingBox();
  expect(box).toBeTruthy();
  await page.mouse.click(box.x + box.width * fraction, box.y + box.height / 2);
}

async function sessionGone(page, session) {
  await expect.poll(async () => (await page.request.get(`/api/v1/sessions/${session.session_id}/status`, { headers: await headers(page) })).status()).toBe(404);
}

test('direct play seeks, saves on close, resumes after reload, and deletes its session', async ({ page }) => {
  const item = await openItem(page);
  const session = await start(page);
  expect(session.decision.can_direct_play).toBeTruthy();
  await playing(page);
  // Koko offers video resume only when at least 30 seconds remain.
  await seek(page, 0.5);
  await expect.poll(() => media(page).evaluate((player) => player.currentTime)).toBeGreaterThan(32);
  await page.locator('#close-player').click();
  await sessionGone(page, session);
  const progress = await page.request.get(`/api/v1/items/${item.id}`, { headers: await headers(page) });
  expect((await progress.json()).playback_position_ms).toBeGreaterThan(32000);
  await page.reload();
  const resumed = await start(page);
  await playing(page);
  await expect.poll(() => media(page).evaluate((player) => player.currentTime)).toBeGreaterThan(32);
  await page.locator('#close-player').click();
  await sessionGone(page, resumed);
});

test('quality changes preserve position and pause, resize real video, and return to Original', async ({ page }, testInfo) => {
  await openItem(page);
  const original = await start(page);
  await playing(page);
  await seek(page, 0.5);
  await expect.poll(() => media(page).evaluate((player) => player.currentTime)).toBeGreaterThan(30);
  await page.locator('#player-play-toggle-small').click();
  await expect.poll(() => media(page).evaluate((player) => player.paused)).toBeTruthy();
  await page.getByLabel('Subtitles', { exact: true }).selectOption('0');
  await page.locator('#player-volume').evaluate((input) => { input.value = '0.35'; input.dispatchEvent(new Event('input', { bubbles: true })); });
  const limited = await sessionAfter(page, () => page.getByLabel('Playback quality').selectOption('480p'));
  expect(limited.client_profile).toMatchObject({ max_video_width: 854, max_video_height: 480, max_bitrate_kbps: 1500 });
  expect(limited.decision.video_transcode_required).toBeTruthy();
  await sessionGone(page, original);
  await expect.poll(() => media(page).evaluate((player) => player.videoHeight)).toBe(480);
  expect(await media(page).evaluate((player) => player.videoWidth)).toBeLessThanOrEqual(854);
  expect(await media(page).evaluate((player) => player.paused)).toBeTruthy();
  expect(await media(page).evaluate((player) => player.volume)).toBeCloseTo(0.35);
  await expect(page.getByLabel('Subtitles', { exact: true })).toHaveValue('0');
  expect(await media(page).evaluate((player) => player.textTracks[0].mode)).toBe('showing');
  const screenshot = testInfo.outputPath('quality-controls.png');
  await page.screenshot({ path: screenshot });
  await testInfo.attach('Quality controls', { path: screenshot, contentType: 'image/png' });
  await expect(page.locator('#player-current-time')).toContainText('0:3');
  await page.locator('#player-play-toggle-small').click();
  await playing(page);
  // This seek restarts FFmpeg with a source offset instead of seeking inside its short output timeline.
  await seek(page, 0.7);
  await expect(page.locator('#player-current-time')).toContainText('0:4');
  await playing(page);
  const restored = await sessionAfter(page, () => page.getByLabel('Playback quality').selectOption('original'));
  expect(restored.decision.can_direct_play).toBeTruthy();
  await sessionGone(page, limited);
  await playing(page);
  await expect.poll(() => media(page).evaluate((player) => player.currentTime)).toBeGreaterThan(44);
  await page.locator('#close-player').click();
  await sessionGone(page, restored);
});

test('alternate audio streams and sidecar subtitles work in the actual media element', async ({ page }) => {
  await openItem(page);
  const session = await start(page);
  await playing(page);
  await page.locator('#player-audio-track-toggle').click();
  const changed = page.waitForRequest((request) => request.url().includes('/stream?') && request.url().includes('audio_stream_index=1'));
  await page.locator('[data-player-audio-track-index="1"]').click();
  await changed;
  await playing(page);
  await page.getByLabel('Subtitles', { exact: true }).selectOption('0');
  await expect.poll(() => media(page).evaluate((player) => player.textTracks[0].cues?.length ?? 0)).toBeGreaterThan(0);
  await expect.poll(() => media(page).evaluate((player) => player.textTracks[0].activeCues?.[0]?.text)).toBe('Playback subtitle fixture');
  await page.locator('#close-player').click();
  await sessionGone(page, session);
});

test('decoder recovery tries conversion once and reports a failed conversion', async ({ page }) => {
  const profiles = [];
  page.on('request', request => {
    if (request.url().endsWith('/api/v1/sessions') && request.method() === 'POST') profiles.push(request.postDataJSON().client_profile);
  });
  await openItem(page, 'Broken Sample');
  const original = await start(page);
  await expect.poll(() => profiles.length).toBe(2);
  expect(profiles[0].force_transcode).toBeFalsy();
  expect(profiles[1].force_transcode).toBeTruthy();
  await sessionGone(page, original);
  await expect(error(page)).toContainText(/source file|FFmpeg/);
  await expect(page.getByRole('button', { name: 'Try compatible playback' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Retry playback' })).toBeVisible();
  await page.locator('#close-player').click();
  expect(profiles).toHaveLength(2);
});

test('a rejected original stream recovers at the saved position and stays working after quality changes', async ({ page }) => {
  const sessions = [];
  const rejected = new Set();
  const decoderRejections = new Set();
  await page.route('**/api/v1/sessions', async route => {
    const response = await route.fetch();
    if (response.ok()) {
      const session = await response.json();
      sessions.push(session);
      if (!session.decision.video_transcode_required) rejected.add(session.session_id);
    }
    await route.fulfill({ response });
  });
  // Inject a rejection at the native decoder boundary; conversion and all server APIs stay real.
  await page.route('**/api/v1/sessions/*/stream*', route => {
    const sessionId = /\/sessions\/([^/]+)\/stream/.exec(route.request().url())?.[1];
    if (rejected.has(sessionId)) {
      decoderRejections.add(sessionId);
      return route.fulfill({ status: 200, contentType: 'video/mp4', body: 'Rejected media fixture' });
    }
    return route.continue();
  });
  const item = await openItem(page);
  expect((await page.request.post(`/api/v1/items/${item.id}/progress`, { headers: await headers(page),
    data: { position_ms: 35000, duration_ms: 70000, completed: false } })).ok()).toBeTruthy();
  await page.reload();
  const original = await start(page);
  await playing(page);
  expect(sessions).toHaveLength(2);
  expect(sessions[1].client_profile.force_transcode).toBeTruthy();
  await expect(page.locator('#player-current-time')).toContainText('0:3');
  await expect(page.locator('.player-badge')).toHaveText('Transcoding');
  await expect(page.locator('.player-badge')).toHaveAttribute('title', /^Video conversion · Audio conversion\n/);
  await sessionGone(page, original);
  await sessionAfter(page, () => page.getByLabel('Playback quality').selectOption('480p'));
  await playing(page);
  const restored = await sessionAfter(page, () => page.getByLabel('Playback quality').selectOption('original'));
  expect(restored.client_profile).toMatchObject({ force_transcode: true, max_video_height: 0, max_bitrate_kbps: 0 });
  await playing(page);
  await expect.poll(() => media(page).evaluate(player => player.videoHeight)).toBe(540);
  expect(sessions).toHaveLength(4);
  expect(decoderRejections.size).toBe(1);
  await page.locator('#close-player').click();
  await sessionGone(page, restored);
});

test('an unavailable FFmpeg reports the cause and retry recovers after settings are corrected', async ({ page }) => {
  await openItem(page);
  const auth = await headers(page);
  const settings = (await (await page.request.get('/api/v1/settings', { headers: auth })).json()).settings;
  const changed = structuredClone(settings);
  changed.ffmpeg.ffmpeg_path = 'koko-e2e-ffmpeg-does-not-exist';
  try {
    expect((await page.request.put('/api/v1/settings', { headers: auth, data: changed })).ok()).toBeTruthy();
    await start(page);
    await playing(page);
    const failed = await sessionAfter(page, () => page.getByLabel('Playback quality').selectOption('480p'));
    await expect(error(page)).toContainText('could not start FFmpeg');
    expect((await (await page.request.get(`/api/v1/sessions/${failed.session_id}/status`, { headers: auth })).json()).error.code).toBe('transcode_unavailable');
    expect((await page.request.put('/api/v1/settings', { headers: auth, data: settings })).ok()).toBeTruthy();
    const retried = await sessionAfter(page, () => page.getByRole('button', { name: 'Retry playback' }).click());
    await playing(page);
    await sessionGone(page, failed);
    await page.locator('#close-player').click();
    await sessionGone(page, retried);
  } finally {
    await page.request.put('/api/v1/settings', { headers: auth, data: settings });
  }
});

test('expired authentication gives a sign-in action during session creation', async ({ page }) => {
  await openItem(page);
  await page.route('**/api/v1/sessions', (route) => route.continue({ headers: { ...route.request().headers(), authorization: 'Bearer expired-e2e-token' } }));
  await page.locator('[data-play-selected-item-start-ms]').first().click();
  await expect(error(page)).toContainText('session has expired');
  await page.getByRole('button', { name: 'Sign in again' }).click();
  await expect(page.locator('#login-form')).toBeVisible();
});

test('closing while session creation is delayed deletes the late server session', async ({ page }) => {
  await openItem(page);
  let release;
  let created;
  const held = new Promise((done) => { release = done; });
  await page.route('**/api/v1/sessions', async (route) => {
    const response = await route.fetch();
    created = await response.json();
    await held;
    await route.fulfill({ response });
  });
  await page.locator('[data-play-selected-item-start-ms]').first().click();
  await expect.poll(() => Boolean(created)).toBeTruthy();
  await page.locator('#close-player').click();
  release();
  await sessionGone(page, created);
  await expect(page.locator('.media-player-overlay')).toHaveCount(0);
});

test('document navigation saves progress and releases an active conversion session', async ({ page }) => {
  await openItem(page);
  await start(page);
  const converted = await sessionAfter(page, () => page.getByLabel('Playback quality').selectOption('480p'));
  await playing(page);
  await page.goto('/');
  await sessionGone(page, converted);
});

test('a file removed after scanning gives an availability error instead of decoder advice', async ({ page }) => {
  const item = await openItem(page, 'Missing Sample');
  const sources = await page.request.get(`/api/v1/items/${item.id}/files`, { headers: await headers(page) });
  const path = (await sources.json()).files[0].path;
  // Fault injection is limited to the generated media inside this suite's private temporary root.
  const root = dirname(dirname(path));
  expect(dirname(root)).toBe(resolve(tmpdir()));
  expect(basename(root)).toMatch(/^koko-playback-e2e-/);
  await rename(path, `${path}.unavailable`);
  try {
    const session = await start(page);
    await expect(error(page)).toContainText('unavailable');
    await expect(page.getByRole('button', { name: 'Try compatible playback' })).toHaveCount(0);
    await page.locator('#close-player').click();
    await sessionGone(page, session);
  } finally {
    await rename(`${path}.unavailable`, path);
  }
});

test('a stalled stream times out with retry and lower-quality guidance', async ({ page }) => {
  await openItem(page);
  await page.clock.install();
  let release;
  const held = new Promise((done) => { release = done; });
  const routePattern = '**/api/v1/sessions/*/stream*';
  await page.route(routePattern, async (route) => { await held; await route.abort('connectionreset'); });
  await start(page);
  await expect.poll(() => media(page).evaluate((player) => !player.paused)).toBeTruthy();
  await page.clock.fastForward(21000);
  await expect(error(page)).toContainText('choose a lower quality');
  release();
  await page.unrouteAll({ behavior: 'wait' });
  await sessionAfter(page, () => page.getByRole('button', { name: 'Retry playback' }).click());
  await playing(page);
  await page.locator('#close-player').click();
});
