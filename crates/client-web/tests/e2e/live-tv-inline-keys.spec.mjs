/** Verifies inline-key IPTV and high-bitrate streams whose audio starts after video. */
import {test,expect} from '@playwright/test';
import {createServer} from 'node:http';
import {spawn} from 'node:child_process';
import {createCipheriv} from 'node:crypto';

test('IPTV with inline HLS keys and delayed audio decodes through the temporary TV buffer',async({page})=>{
  test.setTimeout(120000);
  const transport=await new Promise((done,reject)=>{
    const generator=spawn(process.env.KOKO_E2E_GENERATOR_FFMPEG??process.env.KOKO_E2E_FFMPEG??'ffmpeg',[
      '-hide_banner','-loglevel','error','-f','lavfi','-i','testsrc2=size=640x360:rate=25',
      '-itsoffset','5','-f','lavfi','-i','sine=frequency=440:sample_rate=48000','-t','12',
      // The default 5 MB probe ends before the first audio packet in this stream.
      '-c:v','mpeg2video','-b:v','12000000','-minrate','12000000','-maxrate','12000000',
      '-bufsize','12000000','-g','25','-c:a','aac','-f','mpegts','pipe:1',
    ],{windowsHide:true,stdio:['ignore','pipe','ignore']});
    const chunks=[];generator.stdout.on('data',chunk=>chunks.push(chunk));
    generator.once('error',reject);generator.once('exit',code=>code===0?done(Buffer.concat(chunks)):reject(new Error('Unable to generate encrypted IPTV fixture')));
  });
  const key=Buffer.from('00112233445566778899aabbccddeeff','hex'),iv=Buffer.alloc(16);
  const cipher=createCipheriv('aes-128-cbc',key,iv);
  const encrypted=Buffer.concat([cipher.update(transport),cipher.final()]);
  const inline=`data:text/plain;base64,${key.toString('base64')}`;
  const attributes=`METHOD=AES-128,URI="${inline}",IV=0x${iv.toString('hex')}`;
  let sourceId,sessionId,headers,segments=0;
  const fixture=createServer((request,response)=>{
    if(request.url==='/channels.m3u')response.end(`#EXTM3U\n#EXTINF:-1 tvg-id="inline" tvg-chno="410",Inline Key TV\nhttp://127.0.0.1:${fixture.address().port}/master.m3u8\n`);
    else if(request.url==='/master.m3u8'){
      response.setHeader('Content-Type','application/vnd.apple.mpegurl');
      response.end(`#EXTM3U\n#EXT-X-VERSION:5\n#EXT-X-SESSION-KEY:${attributes}\n#EXT-X-STREAM-INF:BANDWIDTH=12000000,RESOLUTION=640x360\nmedia.m3u8\n`);
    }else if(request.url==='/media.m3u8'){
      response.setHeader('Content-Type','application/vnd.apple.mpegurl');
      response.end(`#EXTM3U\n#EXT-X-VERSION:5\n#EXT-X-TARGETDURATION:12\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-KEY:${attributes}\n#EXTINF:12,\nsegment.ts\n#EXT-X-ENDLIST\n`);
    }else if(request.url==='/segment.ts'){
      segments++;response.setHeader('Content-Type','video/mp2t');response.end(encrypted);
    }else {response.writeHead(404);response.end();}
  });
  await new Promise(done=>fixture.listen(0,'127.0.0.1',done));
  try{
    await page.goto('/');await page.getByLabel('Username',{exact:true}).fill('playback-admin');await page.getByLabel('Password',{exact:true}).fill(process.env.KOKO_E2E_PASSWORD);await page.getByRole('button',{name:'Sign in',exact:true}).click();await expect(page.locator('#login-form')).toHaveCount(0);
    const token=await page.evaluate(()=>localStorage.getItem('koko-client-web-auth-token'));headers={Authorization:`Bearer ${token}`};
    const added=await page.request.post('/api/v1/live-tv/sources',{headers,data:{name:'Inline Key IPTV',kind:'m3u',capacity:1,config:{url:`http://127.0.0.1:${fixture.address().port}/channels.m3u`,guide_kind:'automatic'}}});expect(added.ok()).toBeTruthy();sourceId=(await added.json()).id;
    let channel;
    await expect.poll(async()=>{channel=(await(await page.request.get('/api/v1/live-tv/channels',{headers})).json()).find(c=>c.source_id===sourceId);return !!channel;},{timeout:45000}).toBeTruthy();
    await page.goto('/live-tv');
    const tuning=page.waitForResponse(response=>response.url().endsWith('/api/v1/live-tv/sessions')&&response.request().method()==='POST');
    await page.locator(`[data-tv-play="${channel.id}"]`).first().click();
    const tuned=await tuning,session=await tuned.json();expect(tuned.ok(),JSON.stringify(session)).toBeTruthy();sessionId=session.session_id;
    await expect.poll(()=>page.locator('#media-player').evaluate(video=>video.readyState>=2&&video.currentTime>0.2&&video.videoWidth===640),{timeout:45000}).toBeTruthy();
    expect(segments).toBeGreaterThan(0);
  }finally{
    if(sessionId&&headers)await page.request.delete(`/api/v1/live-tv/sessions/${sessionId}`,{headers});
    if(sourceId&&headers)await page.request.delete(`/api/v1/live-tv/sources/${sourceId}`,{headers});
    fixture.closeAllConnections();await new Promise(done=>fixture.close(done));
  }
});
