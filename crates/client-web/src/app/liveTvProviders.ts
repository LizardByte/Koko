/** Renders bounded provider editors, including live channel-logo references and image uploads. */
import type { TvSource, TvProvider } from '../liveTvApi';
import type { LiveTvState } from './liveTvActions';
import { escapeHtml as escape } from './format';
import { renderTvProviderIcon,renderTvSourceIcon } from './liveTvIcons';
import { renderIcon } from './ui';

/** Avatar-style image picker shared by connection and provider appearance controls. */
export function tvIconPicker(source:number,name:string,provider?:string):string {
  const attribute=provider?'data-tv-icon-provider="'+escape(provider)+'"':'';
  return `<label class="secondary-button tv-icon-upload">Upload icon<input type="file" accept="image/png,image/jpeg,image/webp,image/gif" data-tv-icon-source="${source}" ${attribute} aria-label="Upload icon for ${escape(name)}"/></label>`;
}
/** Source identity and image controls span the card instead of squeezing connection fields. */
export function renderTvSourceHeading(source:TvSource):string {
  return `<span class="tv-source-heading">${renderTvSourceIcon(source)}<strong>${escape(source.name)}</strong></span>`;
}
/** Keep upload and reset actions outside the source card's disclosure button. */
export function renderTvSourceAppearance(source:TvSource):string {
  const reset=source.image_url?'<button type="button" class="secondary-button" data-tv-icon-reset="'+source.id+'">Use default icon</button>':'';
  return `<div class="live-tv-actions">${tvIconPicker(source.id,source.name)}${reset}</div>`;
}
function editor(source:TvSource,provider:TvProvider,data:LiveTvState):string {
  const channels=data.channels.filter(c=>c.source_id===source.id&&c.provider_id===provider.id&&c.logo);
  let choice=provider.has_custom_icon?'upload':'default';
  if(provider.icon_channel_id) choice='channel:'+provider.icon_channel_id;
  const uploadSelected=choice==='upload'?'selected':'';
  const uploadChoice=provider.has_custom_icon?'<option value="upload" '+uploadSelected+'>Uploaded image</option>':'';
  let channelChoices=channels.map(c=>{
    const selected=choice==='channel:'+c.id;
    return `<option value="channel:${c.id}" ${selected?'selected':''}>${escape(c.number)} ${escape(c.name)}</option>`;
  }).join('');
  if(provider.icon_channel_id&&!channels.some(c=>c.id===provider.icon_channel_id)) channelChoices+='<option value="channel:'+provider.icon_channel_id+'" selected>Selected channel (currently unavailable)</option>';
  return `<form data-tv-provider-form="${escape(provider.id)}" data-tv-provider-source="${source.id}" class="live-tv-form">
    <label>Provider name<input name="name" value="${escape(provider.name)}" maxlength="200" required/></label><label><input type="checkbox" name="enabled" ${provider.enabled?'checked':''}/> Show provider in the guide</label>
    <label class="tv-provider-icon-choice">Provider icon<select name="icon_channel"><option value="default" ${choice==='default'?'selected':''}>Default provider icon</option>${uploadChoice}<optgroup label="Use a channel's icon">${channelChoices}</optgroup></select></label>
    <p class="live-tv-form-help">A channel icon stays linked to the channel and updates when its logo changes.</p><div class="live-tv-actions"><button type="submit">Save provider</button>${tvIconPicker(source.id,provider.name,provider.id)}<button type="button" class="secondary-button" data-tv-provider-channels="${escape(provider.id)}" data-tv-provider-source="${source.id}">Manage channels</button></div></form>`;
}
/** Provider preferences survive upstream refreshes; only the expanded icon selector mounts its channels. */
export function renderTvProviders(source:TvSource,data:LiveTvState):string {
  const providers=source.providers ?? [];if(!providers.length) {return '';}
  const cards=providers.map(p=>{
    const key=source.id+':'+p.id,open=data.providerEditor===key,content=open?editor(source,p,data):'';
    return `<details class="live-tv-panel tv-provider-setting" data-controlled-open data-tv-provider-editor="${escape(key)}" ${open?'open':''}><summary>${renderTvProviderIcon(p)}<strong>${escape(p.name)}</strong><span>${p.channel_count} ${p.channel_count===1?'channel':'channels'} · ${p.enabled?'Shown':'Hidden'}</span>${renderIcon('chevron-right','tv-disclosure-icon')}</summary>${content}</details>`;
  }).join('');
  return `<section class="tv-provider-settings"><h4>Providers</h4><p>Choose which providers appear in the guide, rename them, and select their icons.</p>${cards}</section>`;
}
