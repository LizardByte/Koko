/** Defines Live TV guide, setup, playback, and personal DVR API contracts. */
import { requestJson, resolveApiUrl, getStoredAuthToken } from './api';

/** A playable channel; upstream stream credentials are never returned. */
export interface TvChannel {
  id: number; source_id: number; source_key: string; name: string; number: string;
  group_name: string; logo?: string | null; guide_key: string; guide_override?: string | null;
  enabled: boolean; available: boolean;
}
/** Provider metadata retained with both guide programs and recordings. */
export interface TvMetadata {
  subtitle?: string | null; description?: string | null; categories?: string[]; image?: string | null;
  rating?: string | null; episode?: string | null; episode_id?: string | null; credits?: string[]; is_new?: boolean | null;
}
/** A scheduled broadcast using UTC seconds. */
export interface TvProgram {
  id: number; source_id: number; guide_key: string; starts_at: number; ends_at: number;
  title: string; metadata: TvMetadata;
}
/** A guide station that can be assigned explicitly to a tuner or IPTV channel. */
export interface TvStation { source_id: number; guide_key: string; name: string; number: string; aliases: string[]; logo?: string | null }
/** An administrator's configured tuner or playlist and refresh health. */
export interface TvSource { id: number; name: string; kind: string; enabled: boolean; capacity: number; next_refresh: number; last_error?: string | null }
/** Source connection, guide, and optional native tuner settings supplied during setup. */
export interface TvSourceConfig {
  url: string; pin: string; xmltv_url: string; guide_kind: string;
  adapter?: number; frontend?: number; zap_path?: string; multiplex_urls?: string[];
  gracenote?: TvGuideSelection; refresh_hours?: number;
  native_device?: string; delivery_system?: string; region?: string; tuning_options?: string;
}
/** Discover driver-backed tuners attached to the Koko server. */
export function discoverTvTuners(): Promise<{ id: string; name: string; info: string }[]> { return requestJson('GET', '/api/v1/live-tv/tuners'); }
/** Scan a local tuner's terrestrial channels again. */
export function rescanTvTuner(id: number): Promise<void> { return requestJson('POST', `/api/v1/live-tv/sources/${id}/scan`); }
/** Selected Gracenote lineup and country/postal lookup context. */
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
export function getTvGuide(start: number, end: number): Promise<{ programs: TvProgram[]; stations: TvStation[] }> { return requestJson('GET', `/api/v1/live-tv/guide?start=${start}&end=${end}`); }
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
