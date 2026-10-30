/** Generates local media, configures a private server through its API, and cleans up on exit. */
import { spawn } from 'node:child_process';
import { copyFile, mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { createServer } from 'node:http';
import { createServer as createTcpServer } from 'node:net';
import { tmpdir } from 'node:os';
import { dirname, basename, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const workspace = fileURLToPath(new URL('../../../../', import.meta.url));
const root = await mkdtemp(resolve(tmpdir(), 'koko-playback-e2e-'));
const media = resolve(root, 'media');
const baseURL = `http://127.0.0.1:${process.env.KOKO_E2E_PORT}`;
const ffmpeg = process.env.KOKO_E2E_FFMPEG ?? 'ffmpeg';
const ffprobe = process.env.KOKO_E2E_FFPROBE ?? 'ffprobe';
const generator = process.env.KOKO_E2E_GENERATOR_FFMPEG ?? ffmpeg;
let child;
let shuttingDown = false;
const readiness = createServer((_request, response) => { response.end('ready'); });

async function run(command, args, options = {}) {
  await new Promise((done, reject) => {
    const process = spawn(command, args, { cwd: workspace, stdio: 'inherit', windowsHide: true, ...options });
    process.once('error', reject);
    process.once('exit', (code) => code === 0 ? done() : reject(new Error(`${command} exited with ${code}`)));
  });
}

async function request(path, method = 'GET', body, token) {
  const headers = { 'Content-Type': 'application/json' };
  if (token) headers.Authorization = `Bearer ${token}`;
  const response = await fetch(`${baseURL}${path}`, { method, headers, body: body === undefined ? undefined : JSON.stringify(body), signal: AbortSignal.timeout(15000) });
  if (!response.ok) throw new Error(`${method} ${path}: ${response.status} ${await response.text()}`);
  return response.headers.get('content-type')?.includes('application/json') ? response.json() : undefined;
}

async function shutdown(code = 0) {
  if (shuttingDown) return;
  shuttingDown = true;
  readiness.close();
  if (child && child.exitCode === null) {
    const exited = new Promise((done) => child.once('exit', done));
    child.kill('SIGTERM');
    await Promise.race([exited, new Promise((done) => setTimeout(done, 5000))]);
    if (child.exitCode === null) child.kill('SIGKILL');
  }
  // Only this freshly-created temporary directory can be removed.
  if (dirname(root) === resolve(tmpdir()) && basename(root).startsWith('koko-playback-e2e-')) {
    await rm(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
  }
  process.exit(code);
}
process.once('SIGINT', () => { void shutdown(); });
process.once('SIGTERM', () => { void shutdown(); });

try {
  // Refuse a collision before making any setup requests to the application port.
  await new Promise((done, reject) => {
    const probe = createTcpServer();
    probe.once('error', reject);
    probe.listen(Number(process.env.KOKO_E2E_PORT), '127.0.0.1', () => probe.close(done));
  });
  await mkdir(media);
  await run(generator, ['-hide_banner', '-loglevel', 'error', '-f', 'lavfi', '-i', 'testsrc2=size=960x540:rate=24',
    '-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=48000', '-f', 'lavfi', '-i', 'sine=frequency=880:sample_rate=48000',
    '-t', '70', '-map', '0:v', '-map', '1:a', '-map', '2:a', '-c:v', 'mpeg4', '-q:v', '10',
    '-pix_fmt', 'yuv420p', '-g', '48', '-c:a', 'aac', '-b:a', '96k', '-metadata:s:a:0', 'language=eng',
    '-metadata:s:a:0', 'title=English', '-metadata:s:a:1', 'language=spa', '-metadata:s:a:1', 'title=Spanish',
    resolve(root, 'source.mkv')]);
  await run(ffmpeg, ['-hide_banner', '-loglevel', 'error', '-i', resolve(root, 'source.mkv'), '-map', '0',
    '-c:v', 'libx264', '-preset', 'ultrafast', '-crf', '28', '-g', '48', '-c:a', 'copy', '-movflags', '+faststart', resolve(media, 'Playback Sample.mp4')]);
  await writeFile(resolve(media, 'Broken Sample.mp4'), 'This is deliberately invalid media for decoder recovery tests.');
  await copyFile(resolve(media, 'Playback Sample.mp4'), resolve(media, 'Missing Sample.mp4'));
  await writeFile(resolve(media, 'Playback Sample.en.vtt'), 'WEBVTT\n\n00:00:00.000 --> 00:01:09.000\nPlayback subtitle fixture\n');
  const settingsPath = resolve(root, 'settings.yml');
  await writeFile(settingsPath, JSON.stringify({ general: { data_dir: root }, server: { use_https: false, address: '127.0.0.1', port: Number(process.env.KOKO_E2E_PORT) },
    ffmpeg: { ffmpeg_path: ffmpeg, ffprobe_path: ffprobe }, metadata: { providers: [] } }));
  let executable = process.env.KOKO_E2E_SERVER;
  if (!executable) {
    await run('cargo', ['build', '-p', 'koko', '--locked', '--no-default-features'], { env: { ...process.env, KOKO_SKIP_FFMPEG_DOWNLOAD: '1' } });
    const target = process.env.CARGO_TARGET_DIR ?? resolve(workspace, 'target');
    executable = resolve(target, 'debug', process.platform === 'win32' ? 'koko.exe' : 'koko');
  }
  child = spawn(executable, [], { cwd: workspace, windowsHide: true, stdio: 'inherit',
    env: { ...process.env, KOKO_SETTINGS_DIR: root, KOKO_SETTINGS_PATH: settingsPath, ROCKET_WORKERS: '2' } });
  child.once('error', (error) => { console.error(error); void shutdown(1); });
  for (let attempt = 0; ; attempt++) {
    try { await request('/api/v1/bootstrap'); break; }
    catch (error) {
      if (attempt >= 120 || child.exitCode !== null) throw error;
      await new Promise((done) => setTimeout(done, 250));
    }
  }
  await request('/create_user', 'POST', { username: 'playback-admin', password: process.env.KOKO_E2E_PASSWORD, admin: true });
  const login = await request('/login', 'POST', { username: 'playback-admin', password: process.env.KOKO_E2E_PASSWORD });
  const token = login.token;
  const response = await request('/api/v1/settings', 'GET', undefined, token);
  response.settings.metadata.providers = response.settings.metadata.providers.map((provider) => ({ ...provider, enabled: false }));
  await request('/api/v1/settings', 'PUT', response.settings, token);
  await request('/api/v1/settings/libraries', 'POST', { library: { name: 'Playback E2E', path: media, paths: [media], kind: 'home_videos', scanner: 'directory', metadata_providers: [] } }, token);
  const libraries = await request('/api/v1/libraries', 'GET', undefined, token);
  for (let attempt = 0; ; attempt++) {
    const items = await request(`/api/v1/items?library_id=${libraries[0].id}`, 'GET', undefined, token);
    if (['Playback Sample', 'Broken Sample', 'Missing Sample'].every((title) => items.some((item) => item.display_title === title))) break;
    if (attempt >= 120) throw new Error('Media scan did not publish the playback fixtures.');
    await new Promise((done) => setTimeout(done, 250));
  }
  // A separate loopback listener becomes ready only after the real server has scanned the fixtures.
  await new Promise((done, reject) => {
    readiness.once('error', reject);
    readiness.listen(Number(process.env.KOKO_E2E_PORT) + 1, '127.0.0.1', done);
  });
  console.log('Playback E2E fixtures ready.');
  child.once('exit', (code) => { if (!shuttingDown) void shutdown(code ?? 1); });
} catch (error) {
  console.error(error);
  await shutdown(1);
}
