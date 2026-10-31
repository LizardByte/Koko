/** Renders source-specific channel pages and shared guide group settings. */
import type { TvChannel, TvSource, TvSourceGroup } from '../liveTvApi';
import type { LiveTvState } from './liveTvActions';
import { escapeHtml as escape } from './format';
import { renderTvGuideMapping } from './liveTvMapping';

/** Limit the channel editor's mounted forms without limiting imported channels. */
export const TV_MAPPING_PAGE_SIZE = 50;

function channelForm(channel: TvChannel, data: LiveTvState): string {
  return `<form data-tv-channel-update="${channel.id}" class="live-tv-channel-mapping"><strong>${escape(channel.number)} ${escape(channel.name)}</strong>
    <label><input type="checkbox" name="enabled" ${channel.enabled ? 'checked' : ''} /> Enabled</label><label>Guide station${renderTvGuideMapping(channel,data.stations)}</label>
    ${channel.available ? '' : '<span>Missing from source</span>'}<button type="submit" class="secondary-button">Save</button></form>`;
}

/** Open one source at a time, with search and explicit pages for large channel lineups. */
export function renderTvSourceChannels(source: TvSource, data: LiveTvState): string {
  const open = data.mappingSource === source.id;
  let content = '';
  if (open) {
    const query = (data.mappingQuery ?? '').toLocaleLowerCase();
    const channels = data.channels.filter(c=>c.source_id === source.id && (!data.mappingProvider || c.provider_id===data.mappingProvider) && (!query || `${c.number} ${c.name}`.toLocaleLowerCase().includes(query)));
    const page = Math.min(Math.max(0,data.mappingPage ?? 0),Math.max(0,Math.ceil(channels.length/TV_MAPPING_PAGE_SIZE)-1));
    const start = page*TV_MAPPING_PAGE_SIZE;
    const providers=source.providers ?? [];
    const providerOptions=providers.map(p=>`<option value="${escape(p.id)}" ${p.id===data.mappingProvider?'selected':''}>${escape(p.name)} (${p.channel_count})</option>`).join('');
    const providerFilter=providers.length?'<label>Provider<select name="provider"><option value="">All providers</option>'+providerOptions+'</select></label>':'';
    content = `<form data-tv-mapping-search="${source.id}" class="live-tv-inline-form">${providerFilter}<label>Find a channel<input type="search" name="query" value="${escape(data.mappingQuery ?? '')}" /></label><button type="submit" class="secondary-button">Search</button></form>
      <div class="live-tv-actions"><button type="button" class="secondary-button" data-tv-mapping-page="${page-1}" ${page ? '' : 'disabled'}>Previous channels</button><span>${channels.length ? start+1 : 0}–${Math.min(channels.length,start+TV_MAPPING_PAGE_SIZE)} of ${channels.length} channels</span><button type="button" class="secondary-button" data-tv-mapping-page="${page+1}" ${start+TV_MAPPING_PAGE_SIZE < channels.length ? '' : 'disabled'}>Next channels</button></div>
      ${channels.slice(start,start+TV_MAPPING_PAGE_SIZE).map(c=>channelForm(c,data)).join('') || '<p>No channels match this search.</p>'}`;
  }
  return `<details class="live-tv-panel" data-controlled-open data-tv-mapping-source="${source.id}" ${open ? 'open' : ''}><summary>Channel visibility and guide mapping</summary><p>Show or hide channels and choose guide stations for ${escape(source.name)}.</p>${content}</details>`;
}

function groupForm(data: LiveTvState, group?: TvSourceGroup): string {
  const members = new Set(group?.source_ids);
  const choices = data.sources.map(source=>`<label><input type="checkbox" name="source_id" value="${source.id}" ${members.has(source.id) ? 'checked' : ''} /> ${escape(source.name)}</label>`).join('');
  const remove = group ? `<button type="button" class="secondary-button" data-tv-group-delete="${escape(group.id)}">Delete group</button>` : '';
  return `<form data-tv-group-form="${escape(group?.id ?? '')}" class="live-tv-form"><label>Group name<input name="name" value="${escape(group?.name ?? '')}" maxlength="200" required /></label><fieldset class="live-tv-group-members"><legend>Sources</legend>${choices}</fieldset><div class="live-tv-actions"><button type="submit">${group ? 'Save group' : 'Create group'}</button>${remove}</div></form>`;
}

/** Group source connections into one guide view while retaining their individual tuner limits. */
export function renderTvSourceGroups(data: LiveTvState): string {
  if (data.sources.length < 2 && !data.sourceGroups?.length) return '';
  return `<section class="live-tv-groups"><h2>Guide source groups</h2><p>Combine two or more sources into one guide lineup. Each source keeps its connection settings and simultaneous stream limit.</p>
    ${(data.sourceGroups ?? []).map(group=>`<details class="live-tv-panel"><summary>${escape(group.name)}</summary>${groupForm(data,group)}</details>`).join('')}
    <details class="live-tv-panel"><summary>Add a source group</summary>${groupForm(data)}</details></section>`;
}
