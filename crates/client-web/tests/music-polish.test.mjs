import assert from 'node:assert/strict';
import { after,test } from 'node:test';
import {fileURLToPath} from 'node:url';
import {createServer} from 'vite';
globalThis.location={origin:'http://localhost',pathname:'/'};
globalThis.localStorage={getItem:()=>null};
const server=await createServer({root:fileURLToPath(new URL('..',import.meta.url)),server:{middlewareMode:true,hmr:false,ws:false}});
after(()=>server.close());
const {apiErrorMessage}=await server.ssrLoadModule('/src/apiErrors.ts');
const {boundedMenuPosition,musicMediaMetadata,revealPanelEntry}=await server.ssrLoadModule('/src/app/audioPlayerSupport.ts');
const {renderWatchProgress,renderShelfStack,homeShelfEntries}=await server.ssrLoadModule('/src/app/homeView.ts');
const {state}=await server.ssrLoadModule('/src/app/state.ts');
const {renderAlbumTracks,renderActiveAlbumButton,renderTrackLyrics,renderTrackListeningStatus}=await server.ssrLoadModule('/src/app/music.ts');
const {renderLinkedMetadataSummary,renderItemPage}=await server.ssrLoadModule('/src/app/itemPersonView.ts');
const {itemWarnings,renderItemWarnings}=await server.ssrLoadModule('/src/app/itemWarnings.ts');
const {setMusicPlayback,clearMusicPlayback,musicPlaybackItem}=await server.ssrLoadModule('/src/app/musicPlaybackState.ts');
const {renderAudioPlayer,currentLyricText}=await server.ssrLoadModule('/src/app/audioPlayerView.ts');
const {AudioQueue}=await server.ssrLoadModule('/src/app/audioQueue.ts');
const mock=await server.ssrLoadModule('/src/mockApi.ts');

test('shared API errors hide raw HTML, backend JSON and server details while retaining validation guidance',()=>{
  for(const status of [401,403,404,429,500,502,503,504])for(const body of ['<!DOCTYPE html><h1>Unknown error</h1>','{"error":{"description":"SQL error at private/path"}}']) {
    const message=apiErrorMessage(status,body);assert.ok(message.length>10);assert.doesNotMatch(message,/[<>{}]|SQL|private/);
  }
  assert.equal(apiErrorMessage(422,'{"message":"Playlist name is required."}'),'Playlist name is required.');
  assert.match(apiErrorMessage(422,'{"error":{"reason":"Unprocessable Entity"}}'),/Please try again/);
  assert.match(apiErrorMessage(400,'<html>broken</html>'),/Please try again/);
});
test('song menus stay within the viewport and above the dock including narrow layouts',()=>{
  for(const width of [320,390,1280])for(const left of [0,width-40])for(const top of [0,500]) {
    const bounds=boundedMenuPosition({right:left+40,top,bottom:top+40},170,160,width,560);
    assert.ok(bounds.left>=12 && bounds.left+170<=width-12);
    assert.ok(bounds.top>=12 && bounds.top+160<=548);
  }
});
test('media metadata includes an absolute album image and descriptive music fields',()=>{
  const song={...mock.getMockItem(103),artwork_item_id:302,display_title:'My song',music:{artist:'A musician',album:'An album'}};
  const meta=musicMediaMetadata(song);
  assert.equal(meta.title,'My song');assert.equal(meta.artist,'A musician');assert.equal(meta.album,'An album');
  assert.match(meta.artwork[0].src,/^http:\/\/localhost\/api\/v1\/items\/302\/artwork\?/);
  song.audiobook = { title: 'The Book', author: 'The Author' };
  const audio = musicMediaMetadata(song);
  assert.equal(audio.title, 'The Book'); assert.equal(audio.artist, 'The Author');
});
test('music progress says listened and individual songs show progress or completion',()=>{
  const album=mock.getMockItem(302),track={...mock.getMockItem(103),duration_ms:200000,playback_duration_ms:200000,playback_position_ms:100000,playback_completed:false,watch_count:0};
  assert.match(renderWatchProgress(track),/50% listened/);assert.doesNotMatch(renderWatchProgress(track),/watched/);
  assert.match(renderAlbumTracks({...album,children:[track]}),/50% listened/);
  assert.match(renderAlbumTracks({...album,children:[{...track,playback_completed:true}]}),/aria-label="Listened"/);
  assert.match(renderWatchProgress({...track,media_kind:'video',item_type:'movie'}),/50% watched/);
});

test('track numbers remain before a separate listening indicator for both progress and completion',()=>{
  const album=mock.getMockItem(302);
  const track={...mock.getMockItem(103),music:{track_number:7,disc_number:2},duration_ms:200000,playback_position_ms:100000,playback_completed:false};
  for(const completed of [false,true]) {
    const html=renderAlbumTracks({...album,children:[{...track,playback_completed:completed}]});
    assert.match(html,/<span class="music-track-number">7<\/span>\s*<span class="music-track-status">/);
    assert.ok(html.indexOf('music-track-status')<html.indexOf('music-track-title'));
    assert.match(html,completed ? /aria-label="Listened"/ : /50% listened/);
  }
});

test('inline lyrics have a single expansion target without a moving close button',()=>{
  const queue=new AudioQueue();queue.replace([mock.getMockItem(103)]);
  const html=renderAudioPlayer({queue,item:queue.current.item,loading:false,paused:false,volume:.5,muted:false,lyricsEnabled:true,lyricsLoading:false,saving:false});
  assert.match(html,/data-current-lyric/);
  assert.match(html,/class="music-inline-lyrics" hidden/);
  assert.doesNotMatch(html,/Open lyrics|Looking up lyrics|No timed lyrics/);
  assert.doesNotMatch(html,/hide-lyrics|Hide lyrics/);
  assert.match(html,/aria-label="Stop audio"/);
});

test('inline lyrics remain empty before the first timed line and during blank instrumental gaps',()=>{
  const synced='[00:01.00]First line\n[00:05.00]\n[00:08.00]Next line\n[00:10.00]';
  assert.equal(currentLyricText(synced,0),'');
  assert.equal(currentLyricText(synced,2),'First line');
  assert.equal(currentLyricText(synced,6),'');
  assert.equal(currentLyricText(synced,8),'Next line');
  assert.equal(currentLyricText(synced,11),'');
  assert.equal(currentLyricText('',2),'');
});

test('missing saved lyrics direct users to metadata refresh instead of a playback lookup',()=>{
  const queue=new AudioQueue();queue.replace([{id:1,display_title:'Song',item_type:'track'}]);
  const html=renderAudioPlayer({queue,item:queue.current.item,loading:false,paused:false,volume:.5,muted:false,panel:'lyrics',lyricsEnabled:true,lyrics:{instrumental:false,source:'LRCLIB',source_url:'https://lrclib.net/'},lyricsLoading:false,saving:false});
  assert.match(html,/Match the album and track, select LRCLIB in the library’s metadata sources/);
  assert.match(html,/refresh this track’s metadata/);
  assert.doesNotMatch(html,/Looking up lyrics|Lyrics from|retry-lyrics/);
});

test('radio cards share Continue listening with albums and count all cards for pagination',()=>{
  const original=state.home;
  const track=mock.getMockItem(103),album=mock.getMockItem(302);
  const radio={playlist:{id:77,name:'My <radio>',updated_at:100,current_index:0,position_ms:50000},artwork:{item_id:302,library_id:3},items:[track]};
  try {
    state.home={library_id:3,shelves:[{id:'continue_watching',title:'Continue listening',items:[album]},{id:'recently_added',title:'Recently added',items:[album]}],listening_queues:[radio],collections:[]};
    let html=renderShelfStack();
    assert.equal((html.match(/data-nav-section="shelf:continue_watching"/g) ?? []).length,1);
    assert.match(html,/class="media-card radio-card" data-resume-radio="77"/);
    assert.match(html,/My &lt;radio&gt;/);assert.match(html,/background-image:url/);
    assert.match(html,/>2 items</);assert.doesNotMatch(html,/Continue listening to radio/);
    assert.ok(html.indexOf('data-resume-radio="77"')<html.indexOf('data-item-id="302"'));
    assert.match(html,/data-lazy-rendered-count="2"/);
    state.home={shelves:[{id:'continue_watching',title:'Continue watching',items:[]}],listening_queues:[radio],collections:[]};
    html=renderShelfStack();assert.match(html,/<h3>Continue playing<\/h3>/);assert.match(html,/>1 item</);
    state.home={shelves:[],listening_queues:[radio],collections:[]};
    html=renderShelfStack();assert.match(html,/data-resume-radio="77"/);assert.doesNotMatch(html,/No shelves/);
  } finally { state.home=original; }
});

test('Continue rows merge all media and radio by most recent playback, including later pages',()=>{
  const original=state.home;
  const base=mock.getMockItem(103);
  const items=Array.from({length:100},(_,index)=>({...base,id:1000+index,last_played_at:200-index,item_type:index%2 ? 'album' : 'movie'}));
  const radios=[{playlist:{id:77,updated_at:175.5},items:[base]},{playlist:{id:78,updated_at:250},items:[base]}];
  const shelf={id:'continue_watching',title:'Continue',items:items.toReversed()};
  try {
    state.home={shelves:[shelf],listening_queues:radios};
    const entries=homeShelfEntries(shelf);
    assert.equal(entries.length,102);
    assert.equal(entries[0].radio.playlist.id,78);
    assert.equal(entries[1].item.id,1000);
    assert.equal(entries[26].radio.playlist.id,77);
    assert.equal(entries[101].item.id,1099);
    assert.equal(homeShelfEntries({...shelf,id:'recently_added'})[0].item.id,1099);
  } finally {state.home=original;}
});

test('live music overlays saved progress, identifies the current row and changes album controls without restarting',()=>{
  const album=mock.getMockItem(302),track=mock.getMockItem(103);
  try {
    setMusicPlayback({item:track,positionMs:100000,durationMs:200000,paused:false,ended:false});
    let html=renderAlbumTracks({...album,children:[track]});
    assert.match(html,/is-current[^>]*data-music-track-id="103"[^>]*aria-current="true"/);
    assert.match(html,/media-card-levels/);assert.doesNotMatch(html,/50% listened/);assert.match(html,/aria-label="Pause Mock Song"/);
    assert.match(renderActiveAlbumButton(album.id),/Pause album/);
    setMusicPlayback({item:track,positionMs:100000,durationMs:200000,paused:true,ended:false});
    html=renderAlbumTracks({...album,children:[track]});
    assert.match(html,/<span class="sr-only">Paused<\/span>/);assert.doesNotMatch(html,/media-card-levels/);
    assert.match(renderActiveAlbumButton(album.id),/Resume album/);
    setMusicPlayback({item:track,positionMs:200000,durationMs:200000,paused:true,ended:true});
    html=renderAlbumTracks({...album,children:[track]});
    assert.match(html,/aria-label="Listened"/);assert.doesNotMatch(html,/aria-current="true"/);
    assert.equal(renderActiveAlbumButton(album.id),'');
    assert.equal(musicPlaybackItem(track).playback_completed,true);
    clearMusicPlayback();
    assert.equal(musicPlaybackItem(track).playback_position_ms,track.playback_position_ms);
  } finally {clearMusicPlayback();}
});

test('active track levels replace its progress ring while other tracks retain progress',()=>{
  const track={...mock.getMockItem(103),playback_position_ms:100000,playback_duration_ms:200000,playback_completed:false,watch_count:0};
  try {
    setMusicPlayback({item:track,positionMs:100000,durationMs:200000,paused:false,ended:false});
    assert.match(renderTrackListeningStatus(track,true),/media-card-levels/);
    assert.match(renderTrackListeningStatus(track,true),/>Playing</);
    assert.doesNotMatch(renderTrackListeningStatus(track,true),/track-listening-status/);
    assert.match(renderTrackListeningStatus({...track,id:999}),/50% listened/);
  } finally {clearMusicPlayback();}
});

test('song details display saved plain or timed lyrics safely, without empty sections',()=>{
  const song=mock.getMockItem(103);
  const lyrics={plain_lyrics:'Line <one>\nNext & final',instrumental:false,source:'LRCLIB',source_url:'https://lrclib.net/'};
  assert.match(renderTrackLyrics({...song,lyrics}),/Line &lt;one&gt;\nNext &amp; final/);
  const synced={...lyrics,plain_lyrics:'',synced_lyrics:'[ar:Artist]\n[00:01.00]First\n[00:02.00]Second'};
  assert.match(renderTrackLyrics({...song,lyrics:synced}),/First\nSecond/);
  assert.doesNotMatch(renderTrackLyrics({...song,lyrics:synced}),/00:01|ar:Artist/);
  assert.equal(renderTrackLyrics({...song,lyrics:{...lyrics,plain_lyrics:'  '}}),'');
  assert.equal(renderTrackLyrics({...song,lyrics:{...lyrics,instrumental:true}}),'');
  assert.equal(renderTrackLyrics({...song,lyrics:null}),'');
  assert.equal(renderTrackLyrics({...mock.getMockItem(302),lyrics}),'');
});

test('saved lyrics contribute an LRCLIB image alongside metadata provider images, even without a match',()=>{
  const original={selectedItem:state.selectedItem,selectedItemMetadata:state.selectedItemMetadata,metadataProviders:state.metadataProviders,itemMetadataLoading:state.itemMetadataLoading};
  try {
    state.metadataProviders=mock.getMockMetadataProviders();
    state.itemMetadataLoading=false;
    state.selectedItem={...mock.getMockItem(103),lyrics:{plain_lyrics:'Saved lyric',instrumental:false,source:'LRCLIB',source_url:'https://lrclib.net/'}};
    state.selectedItemMetadata={matches:[{provider_id:'musicbrainz',external_id:'recording',relation_kind:'primary',media_type:'recording',title:'Song'}],providers:state.metadataProviders.filter(provider=>provider.id==='musicbrainz')};
    const markup=renderLinkedMetadataSummary();
    assert.match(markup,/providers\/musicbrainz/);assert.match(markup,/providers\/lrclib\.png/);
    assert.doesNotMatch(markup,/providers\/fanart/);
    assert.match(renderItemPage(),/data-nav-section="lyrics"/);
    state.selectedItemMetadata.matches=[];
    assert.match(renderLinkedMetadataSummary(),/providers\/lrclib\.png/);
    state.selectedItem.lyrics=undefined;
    assert.doesNotMatch(renderLinkedMetadataSummary(),/providers\/lrclib\.png/);
  } finally {Object.assign(state,original);}
});

test('item banners explain unmatched metadata, failed refreshes and missing files across media types',()=>{
  for(const item_type of ['artist','album','track','movie','show','episode']) {
    const item={...mock.getMockItem(103),item_type,has_metadata:false,metadata_refresh_state:undefined};
    assert.match(renderItemWarnings(item),/Metadata not matched/);
    assert.match(renderItemWarnings(item),/No metadata is linked/);
    assert.equal(renderItemWarnings({...item,has_metadata:true}),'');
    assert.match(renderItemWarnings({...item,metadata_refresh_state:'pending'}),/matching has not finished/);
    const broken={...item,missing_since:100,metadata_refresh_state:'error',metadata_refresh_error:'Provider unavailable'};
    assert.equal(itemWarnings(broken).length,2);
    assert.match(renderItemWarnings(broken),/Missing from disk/);
    assert.match(renderItemWarnings(broken),/Provider unavailable/);
    assert.doesNotMatch(renderItemWarnings({...broken,metadata_refresh_error:'<html>502 failure</html>'}),/<html>/);
  }
  const album=mock.getMockItem(302),track={...mock.getMockItem(103),has_metadata:false,metadata_refresh_state:'error',metadata_refresh_error:'Provider unavailable'};
  assert.match(renderAlbumTracks({...album,children:[track]}),/class="music-track-warning"[^>]*>.*Metadata refresh failed/);
  assert.match(renderAlbumTracks({...album,children:[{...track,missing_since:100}]}),/Missing from disk/);
});
test('panel scrolling changes only the panel, and lyrics errors have no lookup retry control',()=>{
  const container={scrollTop:0,clientHeight:300,getBoundingClientRect:()=>({top:50})};
  revealPanelEntry(container,{getBoundingClientRect:()=>({top:800,height:60})});assert.equal(container.scrollTop,630);
  const queue=new AudioQueue();queue.replace([mock.getMockItem(103)]);
  const html=renderAudioPlayer({queue,item:queue.current.item,loading:false,paused:false,volume:.5,muted:false,panel:'lyrics',lyricsEnabled:true,lyricsError:'Please retry',lyricsLoading:false,saving:false});
  assert.match(html,/data-current-lyric/);assert.doesNotMatch(html,/Try lyrics again|retry-lyrics/);assert.match(html,/Jump to current lyrics/);assert.match(html,/class="player-volume"/);
});
