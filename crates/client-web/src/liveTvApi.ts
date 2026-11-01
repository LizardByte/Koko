/** Defines Live TV guide, setup, playback, and personal DVR API contracts. */
import { requestJson, resolveApiUrl, getStoredAuthToken } from './api';

/** A playable channel; upstream stream credentials are never returned. */
export interface TvChannel {
  id: number; source_id: number; source_key: string; name: string; number: string;
  group_name: string; logo?: string | null; guide_key: string; guide_override?: string | null;
  enabled: boolean; available: boolean;
  provider_id?:string|null;
}
/** Provider metadata retained with both guide programs and recordings. */
export interface TvMetadata {
  subtitle?: string | null; description?: string | null; categories?: string[]; image?: string | null;
  rating?: string | null; episode?: string | null; episode_id?: string | null; credits?: string[]; is_new?: boolean | null; poster?: string | null;
  is_live?: boolean | null; is_premiere?: boolean | null; is_finale?: boolean | null;
  year?: number | null; series_id?: string | null; artwork_sources?: string[];
}
/** A scheduled broadcast using UTC seconds. */
export interface TvProgram {
  id: number; source_id: number; guide_key: string; starts_at: number; ends_at: number;
  title: string; metadata: TvMetadata;
}
/** A guide station that can be assigned explicitly to a tuner or IPTV channel. */
export interface TvStation { source_id: number; guide_key: string; name: string; number: string; aliases: string[]; logo?: string | null }
/** An administrator's configured tuner or playlist and refresh health. */
export interface TvSource { id: number; name: string; kind: string; enabled: boolean; capacity: number; next_refresh: number; last_error?: string | null;channel_count?:number;playlist_bytes?:number|null;guide_bytes?:number|null;image_url?:string|null;iptv_mode?:string|null;providers?:TvProvider[] }
/** A provider within an IPTV connection; identity remains stable when its display name or image changes. */
export interface TvProvider {source_id:number;id:string;name:string;enabled:boolean;channel_count:number;image_url?:string|null;icon_channel_id?:number|null;has_custom_icon:boolean}
/** Named guide lineup combining independent source connections and stream capacities. */
export interface TvSourceGroup { id:string; name:string; source_ids:number[] }
/** Editable source settings; a stored PIN stays masked and can be preserved on save. */
export interface TvSourceConnection { source:TvSourceInput; has_pin:boolean }
/** Public source labels for the guide, without setup details or credentials. */
export interface TvGuideSource { id: number; name: string; kind: string;image_url?:string|null;iptv_mode?:string|null }
/** Account-specific channel lists synchronized by the server. */
export interface TvPreferences { favorites: number[]; recent: number[] }
/** A private, recording-backed live viewing session and its currently seekable range. */
export interface TvBufferInfo {
  session_id: string; channel_id: number; title: string; start_ms: number; end_ms: number;
  recording_id?: number | null; finished: boolean; error?: string | null; can_keep: boolean;
}
/** Admit a cancellable tune immediately; playback waits for video while the seekable buffer grows. */
export function startTvBuffer(channel_id: number): Promise<TvBufferInfo> { return requestJson('POST','/api/v1/live-tv/sessions',{channel_id,wait:false}); }
/** Renew the viewing session and read the rolling buffer range. */
export function getTvBuffer(id: string): Promise<TvBufferInfo> { return requestJson('GET',`/api/v1/live-tv/sessions/${encodeURIComponent(id)}`); }
/** Stop temporary capture, preserving an explicitly kept DVR recording. */
export function closeTvBuffer(id: string): Promise<void> { return requestJson('DELETE',`/api/v1/live-tv/sessions/${encodeURIComponent(id)}`); }
/** Preserve buffered content and continue recording the current program. */
export function keepTvBuffer(id: string, privateRecording: boolean): Promise<number> { return requestJson('POST',`/api/v1/live-tv/sessions/${encodeURIComponent(id)}/keep`,{private:privateRecording}); }
/** Resolve a buffer playback URL without exposing upstream credentials. */
export function tvBufferStreamUrl(id: string, start: number, quality: string, attempt: number): string {
  return resolveApiUrl(`/api/v1/live-tv/sessions/${encodeURIComponent(id)}/stream?start_ms=${Math.max(0,Math.floor(start))}&quality=${encodeURIComponent(quality)}&attempt=${attempt}`);
}
/** Release a buffer when the browser exits, retaining authorization on the keepalive request. */
export function closeTvBufferOnExit(id: string): void {
  const token = getStoredAuthToken();
  if (token) void fetch(resolveApiUrl(`/api/v1/live-tv/sessions/${encodeURIComponent(id)}`), {method:'DELETE',credentials:'include',keepalive:true,headers:{Authorization:`Bearer ${token}`}}).catch(()=>undefined);
}
/** Source connection, guide, and optional native tuner settings supplied during setup. */
export interface TvSourceConfig {
  url: string; pin: string; xmltv_url: string; guide_kind: string;
  iptv_mode?:string;provider_grouping?:boolean;bridge_capacity?:number;
  artwork_provider?: string;
  preserve_pin?:boolean;
  adapter?: number; frontend?: number; zap_path?: string; multiplex_urls?: string[];
  zap2it?: TvGuideSelection; refresh_hours?: number;
  native_device?: string; delivery_system?: string; region?: string; tuning_options?: string;
}
/** Discover driver-backed tuners attached to the Koko server. */
export function discoverTvTuners(): Promise<{ id: string; name: string; info: string }[]> { return requestJson('GET', '/api/v1/live-tv/tuners'); }
/** Frequency regions provided by Koko's bundled tuner support. */
export function getTvTunerRegions(): Promise<string[]> { return requestJson('GET', '/api/v1/live-tv/tuners/regions'); }
/** A tuner or server advertised on the Koko server's local network. */
export interface TvDiscoveredSource { name: string; url: string; capacity: number }
/** Search local advertisements for the selected source type. */
export function discoverTvSources(kind: string): Promise<TvDiscoveredSource[]> { return requestJson('GET', `/api/v1/live-tv/discover/${encodeURIComponent(kind)}`, undefined, 20000); }
/** Scan a local tuner's terrestrial channels again. */
export function rescanTvTuner(id: number): Promise<void> { return requestJson('POST', `/api/v1/live-tv/sources/${id}/scan`); }
/** Selected Zap2It lineup and country/postal lookup context. */
export interface TvGuideSelection { country: string; postal_code: string; lineup_id: string; headend_id: string; device: string; timezone: string; days: number }
/** A postal-code guide provider, including OTA, cable, or satellite. */
export interface TvGuideProvider { type: string; device: string; lineupId: string; name: string; location: string; timezone: string; headendId: string }
/** Administrator-controlled storage and per-account DVR permissions. */
export interface TvDvrSettings { settings: { recording_dir: string }; users: { user_id: number; username: string; can_record: boolean }[] }
/** Find guide providers available for an American or Canadian postal code. */
export function findTvGuideProviders(country: string, postalCode: string): Promise<TvGuideProvider[]> { return requestJson('GET', `/api/v1/live-tv/guide/providers?country=${encodeURIComponent(country)}&postal_code=${encodeURIComponent(postalCode)}`); }
/** Read DVR settings visible only to administrators. */
export function getTvDvrSettings(): Promise<TvDvrSettings> { return requestJson('GET', '/api/v1/live-tv/dvr/settings'); }
/** Change the save location for future recordings. */
export function setTvDvrSettings(recordingDir: string): Promise<void> { return requestJson('PUT', '/api/v1/live-tv/dvr/settings', { recording_dir: recordingDir }); }
/** Allow or deny scheduling for one account. */
export function setTvRecordingPermission(userId: number, canRecord: boolean): Promise<void> { return requestJson('PUT', `/api/v1/live-tv/dvr/permissions/${userId}`, { can_record: canRecord }); }
/** A supported source and its simultaneous stream allowance. */
export interface TvSourceInput { name: string; kind: string; capacity: number; config: TvSourceConfig }
/** A recording timer or completed capture belonging to one account. */
export interface TvRecording {
  id: number; user_id: number; channel_id: number; rule_id?: number | null; title: string;
  starts_at: number; ends_at: number; padding_before: number; padding_after: number;
  state: string; has_video: boolean; last_error?: string | null; metadata: TvMetadata;
  shared: boolean;
}
/** A series reservation on a selected channel. */
export interface TvSeriesRule { id: number; user_id: number; channel_id: number; title: string; new_only: boolean; enabled: boolean; shared: boolean }
/** A one-off timer or request for a recurring series. */
export interface TvSchedule {
  channel_id: number; title: string; starts_at: number; ends_at: number; series: boolean;
  new_only: boolean; padding_before: number; padding_after: number;
  private?: boolean;
}
/** Read enabled channels or the administrator's complete channel list. */
export function getTvChannels(setup = false): Promise<TvChannel[]> { return requestJson('GET', `/api/v1/live-tv/${setup ? 'setup/' : ''}channels`); }
/** Fetch at most seven days of schedules and available guide station mappings. */
export function getTvGuide(start: number, end: number): Promise<{ programs: TvProgram[]; stations: TvStation[]; sources?: TvGuideSource[]; preferences?: TvPreferences;source_groups?:TvSourceGroup[];providers?:TvProvider[] }> { return requestJson('GET', `/api/v1/live-tv/guide?start=${start}&end=${end}`); }
/** Rename/show a provider and optionally keep its icon linked to one of its own channels. */
export function setTvProvider(source:number,id:string,body:{name:string;enabled:boolean;icon_channel_id:number|null;remove_icon:boolean}):Promise<void> {return requestJson('PUT',`/api/v1/live-tv/sources/${source}/providers/${encodeURIComponent(id)}`,body);}
/** Upload or restore source/provider artwork, using the authenticated image endpoint. */
export async function setTvSourceImage(source:number,provider:string|undefined,file?:File):Promise<void> {
  if (file && file.size>5*1024*1024) throw new Error('Choose an image smaller than 5 MiB.');
  const token=getStoredAuthToken(),params=provider?`?provider=${encodeURIComponent(provider)}`:'';
  const response=await fetch(resolveApiUrl(`/api/v1/live-tv/sources/${source}/image${params}`),{method:file?'PUT':'DELETE',credentials:'include',headers:token?{Authorization:`Bearer ${token}`}:{},body:file});
  if(!response.ok) {const body=await response.json().catch(()=>({})) as {message?:string};throw new Error(body.message ?? 'Unable to update the icon.');}
}
/** Load saved source settings on demand for the administrator's connection editor. */
export function getTvSourceConnection(id:number):Promise<TvSourceConnection> { return requestJson('GET',`/api/v1/live-tv/sources/${id}/connection`); }
/** Persist shared lineup groups while leaving their member connections independent. */
export function setTvSourceGroups(groups:TvSourceGroup[]):Promise<void> { return requestJson('PUT','/api/v1/live-tv/source-groups',groups); }
/** Update a channel favorite for the signed-in account. */
export function setTvFavorite(id: number, favorite: boolean): Promise<void> { return requestJson('PUT', `/api/v1/live-tv/channels/${id}/favorite`, {favorite}); }
/** Read administrator source health. */
export function getTvSources(): Promise<TvSource[]> { return requestJson('GET', '/api/v1/live-tv/sources'); }
/** Read recordings and scheduling conflicts visible to the signed-in account. */
export function getTvRecordings(): Promise<{ recordings: TvRecording[]; conflicts: number[]; can_record: boolean }> { return requestJson('GET', '/api/v1/live-tv/recordings'); }
/** Read the signed-in account's recurring rules. */
export function getTvRules(): Promise<TvSeriesRule[]> { return requestJson('GET', '/api/v1/live-tv/rules'); }
/** Save a source and report guide or channel refresh errors without losing its configuration. */
export function addTvSource(source: TvSourceInput): Promise<{ id: number; refresh_error?: string | null }> { return requestJson('POST', '/api/v1/live-tv/sources', source, 180000); }
/** Replace all source connection settings while preserving channels and DVR schedules. */
export function replaceTvSource(id: number, source: TvSourceInput): Promise<void> { return requestJson('PUT', `/api/v1/live-tv/sources/${id}/connection`, source, 180000); }
/** Refresh a configured source immediately. */
export function refreshTvSource(id: number): Promise<void> { return requestJson('POST', `/api/v1/live-tv/sources/${id}/refresh`, undefined, 180000); }
/** Remove a source and its future schedules while preserving saved recordings. */
export function removeTvSource(id:number):Promise<void> {return requestJson('DELETE',`/api/v1/live-tv/sources/${id}`);}
/** Update source availability and stream capacity. */
export function setTvSource(source: Pick<TvSource, 'id' | 'enabled' | 'capacity'>): Promise<void> { return requestJson('PUT', `/api/v1/live-tv/sources/${source.id}`, source); }
/** Set channel visibility and an optional explicit guide station. */
export function setTvChannel(id: number, enabled: boolean, guideOverride: string | null): Promise<void> { return requestJson('PUT', `/api/v1/live-tv/channels/${id}`, { enabled, guide_override: guideOverride }); }
/** Reserve one broadcast or a recurring series. */
export function scheduleTvRecording(timer: TvSchedule): Promise<void> { return requestJson('POST', '/api/v1/live-tv/recordings', timer); }
/** Stop a capture or cancel a timer while retaining any saved video. */
export function cancelTvRecording(id: number): Promise<void> { return requestJson('POST', `/api/v1/live-tv/recordings/${id}/cancel`); }
/** Delete the current account's stopped recording and saved video. */
export function deleteTvRecording(id: number): Promise<void> { return requestJson('DELETE', `/api/v1/live-tv/recordings/${id}`); }
/** Remove a recurring series rule and cancel its future timers. */
export function deleteTvRule(id: number): Promise<void> { return requestJson('DELETE', `/api/v1/live-tv/rules/${id}`); }
/** Resolve authenticated same-server video URLs without including credentials in the URL. */
export function tvStreamUrl(kind: 'channels' | 'recordings', id: number): string { return resolveApiUrl(`/api/v1/live-tv/${kind}/${id}/stream`); }
/** Upload XMLTV directly when the guide is stored on the user's computer. */
export async function uploadTvGuide(id: number, file: File): Promise<void> {
  const token = getStoredAuthToken();
  const response = await fetch(resolveApiUrl(`/api/v1/live-tv/sources/${id}/xmltv`), {
    method: 'POST', credentials: 'include', body: file, headers: token ? { Authorization: `Bearer ${token}` } : {},
  });
  if (!response.ok) {
    const body = await response.json().catch(() => ({})) as { message?: string };
    throw new Error(body.message ?? 'The XMLTV guide could not be imported.');
  }
}
