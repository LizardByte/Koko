/** Verifies compact source navigation, direct guide playback, hold menus, and readable DVR dialogs. */
import {test,expect} from '@playwright/test';

async function guide(page){
  const now=Math.floor(Date.now()/1000),start=Math.floor(now/1800)*1800;
  const channels=Array.from({length:160},(_,i)=>{let provider='tcl';if(i<150){provider='pluto';}if(i<110){provider='espn';}return {id:i+1,source_id:i<60?1:2,source_key:String(i+1),guide_key:String(i+1),name:`Channel ${i+1}`,number:String(i+1),group_name:'',provider_id:i<60?undefined:provider,logo:`http://127.0.0.1:${process.env.KOKO_E2E_PORT}/assets/providers/tv-espn.png`,enabled:true,available:true};});
  const programs=channels.flatMap(channel=>[0,1].map(i=>({id:channel.id*10+i,source_id:channel.source_id,guide_key:channel.guide_key,title:`Show ${channel.id}${i?' next':''}`,starts_at:i?start+3600:start-600,ends_at:i?start+7200:start+3600,metadata:{description:'Program information',categories:['Sports'],image:`http://127.0.0.1:${process.env.KOKO_E2E_PORT}/assets/providers/tv-foxone.ico`,is_live:!i}})));
  const preferences={favorites:[],recent:[]},tunes=[],recordings=[],errors=[];page.on('pageerror',error=>errors.push(error.message));let failRecording=false;
  await page.route('**/api/v1/live-tv/channels',route=>route.fulfill({json:channels}));
  await page.route('**/api/v1/live-tv/guide?**',route=>route.fulfill({json:{programs,stations:[],preferences,sources:[{id:1,name:'NextPVR',kind:'nextpvr'},{id:2,name:'FastChannels',kind:'m3u',iptv_mode:'fastchannels'}],providers:[{source_id:2,id:'espn',name:'ESPN',enabled:true,channel_count:50},{source_id:2,id:'pluto',name:'Pluto TV',enabled:true,channel_count:40},{source_id:2,id:'tcl',name:'TCL TV+',enabled:true,channel_count:10}]}}));
  await page.route('**/api/v1/live-tv/recordings',route=>{
    if(route.request().method()==='POST'){recordings.push(route.request().postDataJSON());return route.fulfill(failRecording?{status:400,json:{status:400,message:'Recording fixture failure'}}:{json:{}});}
    return route.fulfill({json:{recordings:[],conflicts:[],can_record:true}});
  });
  await page.route('**/api/v1/live-tv/rules',route=>route.fulfill({json:[]}));
  await page.route('**/api/v1/live-tv/sessions',route=>{tunes.push(route.request().postDataJSON());return route.fulfill({status:400,json:{status:400,message:'Playback activation fixture'}});});
  await page.goto('/');await page.getByLabel('Username',{exact:true}).fill('playback-admin');await page.getByLabel('Password',{exact:true}).fill(process.env.KOKO_E2E_PASSWORD);await page.getByRole('button',{name:'Sign in',exact:true}).click();await expect(page.locator('#login-form')).toHaveCount(0);
  await page.goto('/live-tv');await expect(page.locator('#tv-guide-scroll')).toBeVisible();
  return {tunes,recordings,errors,failRecording:value=>{failRecording=value;}};
}

test('source and provider navigation uses large icons, clear nesting, and unclipped focus',async({page},testInfo)=>{
  await guide(page);
  const source=page.getByRole('button',{name:'NextPVR, 60 channels',exact:true});await source.focus();
  const geometry=await source.evaluate(button=>{const ring=getComputedStyle(button),icon=button.querySelector('.tv-source-icon').getBoundingClientRect(),rail=button.closest('.tv-source-rail').getBoundingClientRect(),box=button.getBoundingClientRect();return {offset:parseFloat(ring.outlineOffset),icon:icon.width,left:box.left-rail.left,top:box.top-rail.top,text:button.textContent.trim()};});
  expect(geometry.offset).toBeLessThan(0);expect(geometry.icon).toBeGreaterThanOrEqual(60);expect(geometry.left).toBeGreaterThanOrEqual(0);expect(geometry.top).toBeGreaterThanOrEqual(0);expect(geometry.text).toBe('60');
  await page.screenshot({path:testInfo.outputPath('tv-source-focus.png')});
  await page.getByRole('button',{name:'FastChannels, 100 channels',exact:true}).click();
  const provider=page.getByRole('button',{name:'ESPN, 50 channels',exact:true});await page.keyboard.press('Tab');await provider.focus();
  const nested=await provider.evaluate(button=>{const source=document.querySelector('[data-tv-source="2"]').getBoundingClientRect(),box=button.getBoundingClientRect();return {left:box.left-source.left,width:button.querySelector('.tv-source-icon').getBoundingClientRect().width,offset:parseFloat(getComputedStyle(button).outlineOffset)};});
  expect(nested.left).toBeGreaterThan(8);expect(nested.width).toBeGreaterThanOrEqual(48);expect(nested.offset).toBeLessThan(0);
  expect(await page.locator('.tv-channel-logo-slot').first().evaluate(icon=>icon.getBoundingClientRect().width)).toBeGreaterThanOrEqual(70);
  const tcl=page.getByRole('button',{name:'TCL TV+, 10 channels',exact:true});await expect(tcl.locator('img')).toHaveAttribute('src',/tv-tcl\.png$/);await expect.poll(()=>tcl.locator('img').evaluate(image=>image.complete&&image.naturalWidth>0)).toBeTruthy();await provider.focus();
  await page.screenshot({path:testInfo.outputPath('tv-compact-navigation.png')});
  await tcl.focus();await page.screenshot({path:testInfo.outputPath('tv-tcl-provider.png')});
});

test('guide click and Enter tune the selected channel while holds open recording options without tuning',async({page},testInfo)=>{
  const fixture=await guide(page);
  const watch=async(selector,key)=>{
    const previous=fixture.tunes.length,button=page.locator(selector);await button.focus();
    if(key)await page.keyboard.press('Enter');else await button.click();
    await expect.poll(()=>fixture.tunes.length).toBe(previous+1);
    expect(fixture.tunes.at(-1).channel_id).toBe(2);await expect(page.getByText('Playback activation fixture',{exact:true})).toBeVisible();
    await page.locator('#close-player').click();await page.getByRole('button',{name:'Stop playback',exact:true}).click();
  };
  await watch('#tv-channel-select-2',false);await watch('#tv-program-2-20',true);
  await watch('#tv-program-2-20',false);await watch('#tv-channel-select-2',true);
  const count=fixture.tunes.length,program=page.locator('#tv-program-2-20');await program.focus();
  await page.keyboard.down('Enter');await expect(program).toHaveClass(/is-holding/);await page.waitForTimeout(700);
  const options=page.getByRole('dialog',{name:'Channel options',exact:true});await expect(options).toBeVisible();await page.keyboard.up('Enter');expect(fixture.tunes.length).toBe(count);
  await options.getByRole('button',{name:'Record program or series',exact:true}).click();
  const dialog=page.getByRole('dialog',{name:'Record program or series',exact:true});await expect(dialog).toBeVisible();await expect(dialog).toContainText('Show 2');
  await dialog.getByLabel('Before (minutes)',{exact:true}).fill('4');await dialog.getByLabel('After (minutes)',{exact:true}).fill('7');await dialog.getByLabel('Private (only my account)',{exact:true}).check();await dialog.getByLabel('New episodes only (series)',{exact:true}).check();
  await page.locator('[data-tv-refresh]').evaluate(button=>button.click());await expect(dialog.getByLabel('Before (minutes)',{exact:true})).toHaveValue('4');
  await page.screenshot({path:testInfo.outputPath('tv-recording-modal.png')});
  fixture.failRecording(true);await dialog.getByRole('button',{name:'Record series',exact:true}).click();await expect(dialog.getByRole('alert')).toHaveText('Recording fixture failure');await expect(dialog.getByLabel('Before (minutes)',{exact:true})).toHaveValue('4');
  fixture.failRecording(false);await page.screenshot({path:testInfo.outputPath('tv-recording-error.png')});await dialog.getByRole('button',{name:'Record series',exact:true}).click();await expect(dialog).toHaveCount(0);
  expect(fixture.recordings.at(-1)).toMatchObject({channel_id:2,title:'Show 2',series:true,new_only:true,private:true,padding_before:240,padding_after:420});
  await program.click({button:'right'});await expect(options).toBeVisible();await page.keyboard.press('Escape');await expect(options).toHaveCount(0);await expect(program).toBeFocused();
  const box=await program.boundingBox();await page.mouse.move(box.x+20,box.y+20);await page.mouse.down();await page.waitForTimeout(700);await expect(options).toBeVisible();await page.mouse.up();expect(fixture.tunes.length).toBe(count);await page.keyboard.press('Escape');
  await page.locator('#tv-guide-scroll').evaluate(node=>{node.scrollTop=45*88;});await expect(page.locator('#tv-program-46-460')).toBeAttached();await page.locator('#tv-program-46-460').click({button:'right'});await expect(options).toContainText('Show 46');await page.keyboard.press('Escape');
  await page.setViewportSize({width:390,height:844});await page.locator('#tv-program-46-460').click({button:'right'});await options.getByRole('button',{name:'Record program or series',exact:true}).click();await expect(dialog).toBeVisible();
  const mobile=await dialog.boundingBox();expect(mobile.x).toBeGreaterThanOrEqual(0);expect(mobile.x+mobile.width).toBeLessThanOrEqual(390);await page.screenshot({path:testInfo.outputPath('tv-recording-modal-mobile.png')});await page.keyboard.press('Escape');
  expect(fixture.errors).toEqual([]);
});
