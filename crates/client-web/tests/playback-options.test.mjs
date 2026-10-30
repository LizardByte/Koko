/** Covers capability preservation and actionable recovery choices separately from native browser playback. */
import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { playbackProfile, playbackDeliveryDetail, playbackDeliveryLabel, playbackRequestProblem, mediaPlaybackProblem, renderPlaybackProblem, renderPlaybackQuality } = await server.ssrLoadModule('/src/app/playbackOptions.ts');
const { ApiError } = await server.ssrLoadModule('/src/api.ts');

test('quality caps preserve capabilities and Original clears limits', () => {
  const browser = { supported_containers: ['mp4'], supported_video_codecs: ['h264'], max_video_width: 0, max_video_height: 0, max_bitrate_kbps: 0 };
  const limited = playbackProfile(browser, '720p', false);
  assert.equal(limited.max_video_height, 720);
  assert.equal(limited.max_bitrate_kbps, 4000);
  assert.deepEqual(limited.supported_video_codecs, browser.supported_video_codecs);
  assert.equal(browser.max_video_height, 0);
  assert.deepEqual(playbackProfile(limited, 'original', true), { ...browser, force_transcode: true });
});

test('access failures do not suggest transcoding around restrictions', () => {
  for (const status of [401, 403, 404, 422]) {
    const problem = playbackRequestProblem(new ApiError(status, 'private internal message'));
    assert.ok(!problem.retry && !problem.compatible);
    assert.ok(!problem.message.includes('private'));
  }
  assert.ok(playbackRequestProblem(new ApiError(401, '')).signIn);
  assert.ok(playbackRequestProblem(new TypeError('offline')).retry);
});

test('decoder recovery actions avoid repeating conversion for an already converted stream', () => {
  for (const code of [3, 4]) {
    assert.ok(mediaPlaybackProblem(code, false).compatible);
    assert.equal(mediaPlaybackProblem(code, true).compatible, false);
  }
  assert.equal(mediaPlaybackProblem(2, false).compatible, undefined);
  const diagnostic = mediaPlaybackProblem(4, false, { error: { code: 'transcode_failed', message: 'Check FFmpeg.' } });
  assert.equal(diagnostic.message, 'Check FFmpeg.');
  assert.equal(diagnostic.compatible, undefined);
  const html = renderPlaybackProblem({ message: '<script>bad</script>', retry: true });
  assert.ok(html.includes('&lt;script&gt;'));
  assert.ok(!html.includes('<script>'));
});

test('quality and delivery labels distinguish source quality from direct play', () => {
  assert.match(renderPlaybackQuality('original'), /Original quality/);
  assert.equal(playbackDeliveryLabel({ transcode_required: false }), 'Direct Play');
  assert.equal(playbackDeliveryLabel({ transcode_required: true }), 'Transcoding');
  assert.equal(playbackDeliveryLabel({ transcode_required: true, audio_transcode_required: true }), 'Transcoding');
  assert.equal(playbackDeliveryLabel({ transcode_required: true, video_transcode_required: true }), 'Transcoding');
  assert.equal(playbackDeliveryLabel({ transcode_required: false }, true), 'Transcoding');
  assert.equal(playbackDeliveryDetail({ transcode_required: false }), 'Direct Play');
  assert.equal(playbackDeliveryDetail({ transcode_required: true }), 'Remuxing');
  assert.equal(playbackDeliveryDetail({ transcode_required: true, audio_transcode_required: true }), 'Audio conversion');
  assert.equal(playbackDeliveryDetail({ transcode_required: true, video_transcode_required: true }), 'Video conversion');
  assert.equal(playbackDeliveryDetail({ transcode_required: true, video_transcode_required: true, audio_transcode_required: true }), 'Video conversion · Audio conversion');
  assert.equal(playbackDeliveryDetail({ transcode_required: false }, true), 'Video conversion · Audio conversion');
});
