/** Keeps channel mapping dropdowns compact until the user opens an individual station list. */
import type { TvChannel, TvStation } from '../liveTvApi';
import { escapeHtml as escape } from './format';

function stationOption(station: TvStation): string {
  const label = `${station.number} ${station.name} (${station.guide_key})`;
  return `<option value="${escape(station.guide_key)}">${escape(label)}</option>`;
}

/** Render only Automatic and the current override, avoiding channels × stations options. */
export function renderTvGuideMapping(channel: TvChannel, stations: TvStation[]): string {
  const selected = channel.guide_override;
  const station = selected ? stations.find(s => s.source_id === channel.source_id && s.guide_key === selected) : undefined;
  const option = selected ? stationOption(station ?? {source_id:channel.source_id,guide_key:selected,name:selected,number:'',aliases:[]}).replace('<option ', '<option selected ') : '';
  return `<select name="guide" data-tv-guide-source="${channel.source_id}"><option value="">Automatic (${escape(channel.guide_key || 'unmapped')})</option>${option}</select>`;
}

/** Populate the focused native select and discard its other options when focus leaves it. */
export function bindTvGuideMappings(root: ParentNode, signal: AbortSignal, stations: TvStation[]): void {
  const sources = new Map<number, TvStation[]>();
  for (const station of stations) {
    const list = sources.get(station.source_id) ?? [];
    list.push(station); sources.set(station.source_id, list);
  }
  const choices = new Map<number, string>();
  root.querySelectorAll<HTMLSelectElement>('[data-tv-guide-source]').forEach(select => {
    select.addEventListener('focus', () => {
      const current = select.value;
      const source = Number(select.dataset.tvGuideSource);
      const rows = sources.get(source) ?? [];
      let options = choices.get(source);
      if (options === undefined) { options = rows.map(stationOption).join(''); choices.set(source, options); }
      const missing = current && !rows.some(s => s.guide_key === current) ? select.selectedOptions[0]?.outerHTML ?? '' : '';
      select.innerHTML = select.options[0].outerHTML + options + missing;
      select.value = current;
    }, {signal});
    select.addEventListener('blur', () => {
      for (const option of Array.from(select.options)) if (option.value && !option.selected) option.remove();
    }, {signal});
  });
}
