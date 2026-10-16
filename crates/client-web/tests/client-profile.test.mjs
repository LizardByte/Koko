import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { getWebClientProfile } = await server.ssrLoadModule('/src/api.ts');

function profileWithSupport(videoTypes, audioTypes = new Map()) {
  const previousDocument = globalThis.document;
  globalThis.document = {
    createElement: (tag) => ({ canPlayType: (type) => (tag === 'video' ? videoTypes : audioTypes).get(type) ?? '' }),
  };
  try {
    return getWebClientProfile();
  } finally {
    globalThis.document = previousDocument;
  }
}

test('MP4 playback preserves container aliases and requires probable video support', () => {
  for (const [type, codec] of [
    ['video/mp4; codecs="avc1.42E01E, mp4a.40.2"', 'h264'],
    ['video/mp4; codecs="hvc1.1.6.L93.B0, mp4a.40.2"', 'hevc'],
  ]) {
    const profile = profileWithSupport(new Map([[type, 'probably']]));
    assert.deepEqual(profile.supported_containers, ['mp4', 'm4v']);
    assert.deepEqual(profile.supported_video_codecs, [codec]);
    assert.deepEqual(profile.supported_audio_codecs, ['aac']);
    assert.deepEqual(profileWithSupport(new Map([[type, 'maybe']])).supported_containers, []);
  }
  const av1 = profileWithSupport(new Map([['video/mp4; codecs="av01.0.05M.08, mp4a.40.2"', 'probably']]));
  assert.deepEqual(av1.supported_containers, ['mp4']);
  assert.deepEqual(av1.supported_video_codecs, ['av1']);
  assert.deepEqual(av1.supported_audio_codecs, ['aac']);
});

test('WebM capabilities include the matching audio codec without inventing MP4 support', () => {
  for (const [type, video, audio] of [
    ['video/webm; codecs="vp8, vorbis"', 'vp8', 'vorbis'],
    ['video/webm; codecs="vp9, opus"', 'vp9', 'opus'],
    ['video/webm; codecs="av01.0.05M.08, opus"', 'av1', 'opus'],
  ]) {
    const profile = profileWithSupport(new Map([[type, 'probably']]));
    assert.deepEqual(profile.supported_containers, ['webm']);
    assert.deepEqual(profile.supported_video_codecs, [video]);
    assert.deepEqual(profile.supported_audio_codecs, [audio]);
  }
});

test('combined capabilities stay unique while audio accepts possible support', () => {
  const profile = profileWithSupport(new Map([
    ['video/mp4; codecs="avc1.42E01E, mp4a.40.2"', 'probably'],
    ['video/mp4; codecs="hvc1.1.6.L93.B0, mp4a.40.2"', 'probably'],
    ['video/webm; codecs="vp9, opus"', 'probably'],
  ]), new Map([
    ['audio/mp4; codecs="mp4a.40.2"', 'maybe'],
    ['audio/ogg; codecs="opus"', 'probably'],
  ]));
  assert.deepEqual(profile.supported_containers, ['mp4', 'm4v', 'webm', 'm4a', 'ogg']);
  assert.deepEqual(profile.supported_video_codecs, ['h264', 'hevc', 'vp9']);
  assert.deepEqual(profile.supported_audio_codecs, ['aac', 'opus']);
});
