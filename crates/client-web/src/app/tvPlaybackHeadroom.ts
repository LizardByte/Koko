/** Maintains a playable Live TV reserve before starting or resuming a growing buffer. */
import type { TvPlayback } from './types';

const RESERVE_MS = 2000;

/** Measure the contiguous downloaded video at the current position, excluding later buffered islands. */
export function bufferedTvHeadroom(video: HTMLMediaElement): number {
  const position = video.currentTime;
  for (let index=0;index<video.buffered.length;index++) {
    const start = video.buffered.start(index), end = video.buffered.end(index);
    // A newly attached decoder can initially point just before its first timestamp.
    const initial = position < 0.25 && start < 1;
    if ((start<=position || initial) && end>=position) return Math.max(0,end-Math.max(position,start))*1000;
  }
  return 0;
}

/** Skip to the newest safe point while retaining room for the capture and playback pipelines. */
export function tvLivePosition(tv: TvPlayback): number {
  const buffer = tv.buffer;
  return buffer ? Math.max(buffer.start_ms,buffer.end_ms-6000-RESERVE_MS) : 0;
}

/** Startup and underruns resume only after actual playable media accumulates; no retune or seek. */
export function bindTvPlaybackHeadroom(video: HTMLMediaElement, tv: TvPlayback, active: () => boolean, paused: () => boolean): () => void {
  const events = new AbortController();
  let played = !video.paused && video.readyState >= video.HAVE_FUTURE_DATA;
  const rebuffer = bindHeadroomHold(video,events.signal,() => Boolean(tv.buffer?.finished || video.ended));
  video.addEventListener('playing',() => { played = true; },{signal:events.signal});
  video.addEventListener('waiting',() => {
    const buffer = tv.buffer;
    if (!active() || !played || paused() || video.paused || video.seeking || !buffer || buffer.finished || buffer.error) return;
    rebuffer.hold();
  },{signal:events.signal});
  if (active() && video.currentTime<0.25 && !paused() && !tv.buffer?.finished && !tv.buffer?.error) rebuffer.hold();
  return () => { events.abort(); rebuffer.cancel(); };
}

/** Hold the playback clock while loading, retaining user pause intent and the existing decoder. */
function bindHeadroomHold(video: HTMLMediaElement, signal: AbortSignal, finished: () => boolean): {hold:()=>void;cancel:()=>void} {
  let timer:ReturnType<typeof setInterval>|undefined, rate=1;
  const cancel = () => {
    if (timer===undefined) return;
    globalThis.clearInterval(timer);timer=undefined;
    if (video.playbackRate===0) video.playbackRate=rate;
  };
  const ready = () => {
    if (finished() || (video.readyState>=video.HAVE_FUTURE_DATA && bufferedTvHeadroom(video)>=RESERVE_MS)) cancel();
  };
  video.addEventListener('pause',cancel,{signal});
  video.addEventListener('seeking',cancel,{signal});
  video.addEventListener('progress',ready,{signal});
  video.addEventListener('canplay',ready,{signal});
  const hold = () => {
    if (timer!==undefined || video.playbackRate===0 || bufferedTvHeadroom(video)>=RESERVE_MS) return;
    rate=video.playbackRate;video.playbackRate=0;
    timer=globalThis.setInterval(ready,100);
  };
  return {hold,cancel};
}
