/** Verifies a measured playable reserve at startup and after underruns, preserving user playback intent. */
import assert from 'node:assert/strict';
import {after,test} from 'node:test';
import {fileURLToPath} from 'node:url';
import {createServer} from 'vite';

const server=await createServer({root:fileURLToPath(new URL('..',import.meta.url)),server:{middlewareMode:true,hmr:false,ws:false}});
after(()=>server.close());
const {bufferedTvHeadroom,bindTvPlaybackHeadroom,tvLivePosition}=await server.ssrLoadModule('/src/app/tvPlaybackHeadroom.ts');
function media(extra={}){
  const video=Object.assign(new EventTarget(),{currentTime:10,paused:false,seeking:false,readyState:3,HAVE_FUTURE_DATA:3,dataset:{},playbackRate:1.25,volume:0.3,src:'/same-decoder',ranges:[[0,10.1]],...extra});
  video.buffered={get length(){return video.ranges.length;},start:index=>video.ranges[index][0],end:index=>video.ranges[index][1]};
  return video;
}
const playback=()=>({buffer:{session_id:'buffer',start_ms:0,end_ms:20000,finished:false}});
const progress=video=>video.dispatchEvent(new Event('progress'));
const waiting=video=>video.dispatchEvent(new Event('waiting'));

test('headroom measures contiguous playable media, allowing initial timestamps but excluding buffered islands',()=>{
  const video=media({ranges:[[0,11.5],[20,40]]});
  assert.equal(bufferedTvHeadroom(video),1500);
  video.currentTime=12;assert.equal(bufferedTvHeadroom(video),0);
  video.currentTime=0;video.ranges=[[0.1,2.2]];assert.equal(bufferedTvHeadroom(video),2100);
});

test('startup waits for two seconds of actual video instead of releasing on a readyState or elapsed time',()=>{
  const video=media({currentTime:0,paused:true,readyState:1,ranges:[]}),tv=playback();
  const cleanup=bindTvPlaybackHeadroom(video,tv,()=>true,()=>false);
  try{
    assert.equal(video.playbackRate,0);
    video.readyState=3;video.ranges=[[0,0.25]];video.dispatchEvent(new Event('canplay'));
    assert.equal(video.playbackRate,0);
    video.ranges=[[0,1.9]];progress(video);assert.equal(video.playbackRate,0);
    video.ranges=[[0,2.1]];progress(video);assert.equal(video.playbackRate,1.25);
    assert.equal(video.currentTime,0);assert.equal(video.src,'/same-decoder');assert.equal(video.volume,0.3);
  }finally{cleanup();}
});

test('the first underrun rebuilds the reserve, including while time shifted, without a seek or decoder replacement',()=>{
  const video=media(),tv=playback();tv.buffer.end_ms=100000;
  const cleanup=bindTvPlaybackHeadroom(video,tv,()=>true,()=>false);
  try{
    waiting(video);assert.equal(video.playbackRate,0);
    video.ranges=[[0,10.9]];progress(video);assert.equal(video.playbackRate,0);
    video.ranges=[[0,12.1]];progress(video);assert.equal(video.playbackRate,1.25);
    assert.equal(video.currentTime,10);assert.equal(video.src,'/same-decoder');assert.equal(video.volume,0.3);
    assert.equal(video.paused,false);
  }finally{cleanup();}
});

test('short finished captures release the startup hold even when less than two seconds remain',()=>{
  const video=media({currentTime:0,readyState:1,ranges:[]}),tv=playback();
  const cleanup=bindTvPlaybackHeadroom(video,tv,()=>true,()=>false);
  try{
    assert.equal(video.playbackRate,0);video.ranges=[[0,0.5]];video.readyState=3;progress(video);
    assert.equal(video.playbackRate,0);tv.buffer.finished=true;progress(video);assert.equal(video.playbackRate,1.25);
  }finally{cleanup();}
});

test('intentional pauses, seeks, inactive views, and disposed bindings cannot restart playback',()=>{
  const video=media(),tv=playback();let active=true,paused=true;
  const cleanup=bindTvPlaybackHeadroom(video,tv,()=>active,()=>paused);
  try{
    waiting(video);assert.equal(video.playbackRate,1.25);
    paused=false;video.seeking=true;waiting(video);assert.equal(video.playbackRate,1.25);
    video.seeking=false;active=false;waiting(video);assert.equal(video.playbackRate,1.25);
    active=true;waiting(video);assert.equal(video.playbackRate,0);
    video.paused=true;video.dispatchEvent(new Event('pause'));assert.equal(video.playbackRate,1.25);assert.equal(video.paused,true);
    video.paused=false;waiting(video);assert.equal(video.playbackRate,0);
    cleanup();assert.equal(video.playbackRate,1.25);waiting(video);assert.equal(video.playbackRate,1.25);
    assert.equal(tvLivePosition(tv),12000);tv.buffer.start_ms=15000;assert.equal(tvLivePosition(tv),15000);
  }finally{cleanup();}
});
