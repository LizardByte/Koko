/** Runs actual media playback against an isolated Koko server using a codec-enabled browser. */
import { defineConfig } from '@playwright/test';
import { randomUUID } from 'node:crypto';

process.env.KOKO_E2E_PASSWORD ??= randomUUID();
process.env.KOKO_E2E_PORT ??= '19192';
process.env.KOKO_E2E_FFMPEG ??= 'ffmpeg';
process.env.KOKO_E2E_FFPROBE ??= 'ffprobe';
process.env.KOKO_E2E_GENERATOR_FFMPEG ??= process.env.KOKO_E2E_FFMPEG;
const baseURL = `http://127.0.0.1:${process.env.KOKO_E2E_PORT}`;
const browserName = process.env.KOKO_E2E_BROWSER ?? 'chromium';

export default defineConfig({
  testDir: './tests/e2e',
  testMatch: '*.spec.mjs',
  fullyParallel: false,
  workers: 1,
  timeout: 60000,
  expect: { timeout: 15000 },
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL,
    browserName,
    channel: browserName === 'chromium' ? process.env.KOKO_E2E_CHANNEL ?? 'chrome' : undefined,
    launchOptions: { args: browserName === 'chromium' ? ['--autoplay-policy=no-user-gesture-required'] : [] },
    headless: true,
    viewport: { width: 1440, height: 900 },
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  webServer: {
    command: 'node tests/e2e/server.mjs',
    url: `http://127.0.0.1:${Number(process.env.KOKO_E2E_PORT) + 1}/ready`,
    reuseExistingServer: false,
    timeout: 600000,
    stdout: 'pipe',
    stderr: 'pipe',
  },
});
