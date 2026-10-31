/** Shared source-type symbols and provider badges, with uploaded/channel-linked image overrides. */
import type { TvGuideSource, TvProvider } from '../liveTvApi';
import { resolveApiUrl } from '../api';
import { escapeHtml as escape } from './format';
import { renderIcon } from './ui';
import type { AppIconName } from './types';

const PROVIDERS:Record<string,[string,string]>={espn:['ESPN','#cf2439'],adultswim:['AS','#252c3c'],sling:['SLING','#0c6d9b'],pluto:['pluto','#5a3cb0'],roku:['roku','#662d91'],tubi:['tubi','#665500'],foxone:['FOX','#125aa8'],bally:['BALLY','#a52936'],wsn:['WSN','#673bb1'],plex:['plex','#806300'],samsung:['TV+','#16348d'],lg:['LG','#a60940'],amazon:['prime','#12658a']};
const ALIASES:Record<string,string>={amazon_prime_free:'amazon',ballysports:'bally','lg-channels':'lg'};
const SOURCE_NAMES:Record<string,string>={fastchannels:'FastChannels',hdhomerun:'HDHomeRun',nextpvr:'NextPVR',m3u:'IPTV',tvheadend:'Tvheadend',native:'Local tuner',dvb:'Local tuner'};
const SOURCE_LOGOS:Record<string,string>={fastchannels:'fastchannels.svg',hdhomerun:'hdhomerun.ico',nextpvr:'nextpvr.ico',tvheadend:'tvheadend.png'};
/** Use authenticated Koko artwork or an ordinary HTTP image, never arbitrary markup or data URLs. */
export function tvSourceImage(value?:string|null):string {
  if (!value) return '';
  if (/^\/api\/v1\/live-tv\/sources\/\d+\/image(?:\?[^#]*)?$/.test(value)) return resolveApiUrl(value);
  try {const url=new URL(value);return ['http:','https:'].includes(url.protocol)&&!url.username&&!url.password?url.href:'';}catch{return '';}
}
function frame(fallback:string,image:string,label:string,backup=''):string {
  const backupAttribute=backup?' data-tv-source-fallback="'+escape(backup)+'"':'';
  const picture=image?'<img data-tv-source-image src="'+escape(image)+'"'+backupAttribute+' alt="" loading="lazy" referrerpolicy="no-referrer"/>':'';
  return `<span class="tv-source-icon" role="img" aria-label="${escape(label)}"><span class="tv-icon-fallback">${fallback}</span>${picture}</span>`;
}
/** Prefer uploaded artwork, then the bundled application logo, then a Lucide symbol. */
export function renderTvSourceIcon(source:Pick<TvGuideSource,'kind'|'image_url'|'iptv_mode'>):string {
  const icons:Record<string,AppIconName>={hdhomerun:'tv',nextpvr:'database-zap',m3u:'link-2',tvheadend:'radio',native:'layers',dvb:'layers'};
  const type=source.kind==='m3u'&&source.iptv_mode==='fastchannels'?'fastchannels':source.kind;
  const uploaded=tvSourceImage(source.image_url),logo=SOURCE_LOGOS[type]?resolveApiUrl('/assets/providers/'+SOURCE_LOGOS[type]):'';
  return frame(renderIcon(icons[source.kind] ?? 'tv','tv-type-symbol'),uploaded||logo,`${SOURCE_NAMES[type] ?? 'TV'} source icon`,uploaded?logo:'');
}
/** Keep the same icon slot for the aggregate provider view. */
export function renderTvAllProvidersIcon():string {return frame(renderIcon('layers','tv-type-symbol'),'','All providers icon');}
/** Failed uploads fall back to the application logo; failed logos reveal the Lucide symbol. */
export function recoverTvSourceImage(image:HTMLImageElement):void {
  if(!image.matches?.('[data-tv-source-image]')) return;
  const backup=image.dataset.tvSourceFallback;
  if(backup){delete image.dataset.tvSourceFallback;image.src=backup;image.hidden=false;}
  else image.hidden=true;
}
/** Bundled provider badges work offline; an uploaded or channel-linked image overrides the badge. */
export function renderTvProviderIcon(provider:Pick<TvProvider,'id'|'name'|'image_url'>):string {
  const [text,color]=PROVIDERS[ALIASES[provider.id] ?? provider.id] ?? [provider.name.split(/\s+/).map(s=>s[0] ?? '').join('').slice(0,3).toUpperCase(),'#34445b'];
  const fallback=`<svg viewBox="0 0 64 64" aria-hidden="true"><rect width="64" height="64" rx="12" fill="${color}"/><text x="32" y="34" fill="white" text-anchor="middle" dominant-baseline="middle" font-family="system-ui,sans-serif" font-size="${text.length>4?13:17}" font-weight="700">${escape(text)}</text></svg>`;
  return frame(fallback,tvSourceImage(provider.image_url),`${provider.name} provider icon`);
}
