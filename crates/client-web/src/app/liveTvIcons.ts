/** Bundled application and provider logos, with uploaded or channel-linked image overrides. */
import type { TvGuideSource, TvProvider } from '../liveTvApi';
import { resolveApiUrl } from '../api';
import { escapeHtml as escape } from './format';
import { renderIcon } from './ui';
import type { AppIconName } from './types';

const PROVIDERS:Record<string,string>={adultswim:'png',amazon:'svg',bally:'svg',cbs:'png',espn:'png',foxone:'ico',freelivesports:'png',fubo:'png',hallmark:'png',hulu:'png',lg:'svg',localnow:'png',nfl:'png',plex:'svg',pluto:'png',reelz:'png',roku:'svg',samsung:'svg',sling:'svg',spectrum:'png',stirr:'png',tcl:'png',tubi:'png',vizio:'png',whale:'svg',wsn:'png',xumo:'svg',youtube:'png'};
const ALIASES:Record<string,string>={adult_swim:'adultswim',amazon_prime_free:'amazon',ballysports:'bally',cbs_sports:'cbs',charter:'spectrum',fox_one:'foxone',fubotv:'fubo','lg-channels':'lg',pluto_tv:'pluto',prime:'amazon',reelz_tve:'reelz',roku_channel:'roku',samsungtvplus:'samsung',sling_freestream:'sling',slingtv:'sling',watchfree:'vizio',youtube_tv:'youtube',youtubetv:'youtube'};
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
/** Prefer custom/channel artwork, then the real provider logo, with Lucide for unknown brands. */
export function renderTvProviderIcon(provider:Pick<TvProvider,'id'|'name'|'image_url'>):string {
  const id=ALIASES[provider.id] ?? provider.id,extension=PROVIDERS[id];
  const logo=extension?resolveApiUrl(`/assets/providers/tv-${id}.${extension}`):'',custom=tvSourceImage(provider.image_url);
  return frame(renderIcon('tv','tv-type-symbol'),custom||logo,`${provider.name} provider icon`,custom?logo:'');
}
