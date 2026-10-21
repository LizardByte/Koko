/** Orders audiobook files and maps chapter offsets independently of disc and file boundaries. */

import type { AudiobookRecording, AudiobookRecordingChoices, MediaItemDetail, MediaItemSummary } from '../api';
import { resolveApiUrl } from '../api';
import { escapeHtml, formatDuration } from './format';
import { renderButtonContent } from './ui';
import { renderAudioTrackRow } from './music';
import { activeMusicPlayback } from './musicPlaybackState';
import { renderTrackWarnings } from './itemWarnings';
import { state } from './state';
import { renderMetadataSearchProviderAttribution } from './providers';

/** Restarting a track preserves the selected narration; restarting a book begins its ordered audio queue. */
export type AudiobookRestartScope = 'track' | 'book';

/** Prefer the live file, then the server's saved target, without confusing aggregate runtime with an offset. */
export function audiobookRestartTarget(book: MediaItemDetail, scope: AudiobookRestartScope): MediaItemSummary | undefined {
  const parts = orderedAudiobookParts(book.children).filter((part) => part.playable && !part.missing_since);
  if (scope === 'book') return parts[0];
  const playback = activeMusicPlayback();
  const active = playback && !playback.ended && playback.userId === state.bootstrap?.current_user?.id && playback.item.parent_id === book.id ? playback.item.id : undefined;
  const target = active ?? book.playback_target?.item_id;
  return parts.find((part) => part.id === target) ?? [...parts].sort((a, b) => (b.last_played_at ?? 0) - (a.last_played_at ?? 0))[0];
}

/** A chapter start on one local audio file's original timeline. */
export interface AudioChapterTarget {
  title: string;
  itemId: number;
  startMs: number;
}

function naturalKey(path: string): string {
  return path.toLowerCase().replace(/\d+/g, (digits) => {
    const value = digits.replace(/^0+/, '');
    return `${String(value.length).padStart(10, '0')}:${value}`;
  }) + '\0';
}

/** Use disc/part tags only for completely numbered discs; otherwise use natural filenames. */
export function orderedAudiobookParts(items: MediaItemSummary[]): MediaItemSummary[] {
  const parts = items.filter((item) => item.item_type === 'audiobook_part');
  const discs = new Map<number, { complete: boolean; numbers: Set<number> }>();
  for (const part of parts) {
    const disc = part.audiobook?.disc_number ?? 1;
    const info = discs.get(disc) ?? { complete: true, numbers: new Set<number>() };
    const number = part.audiobook?.part_number;
    if (number == null || info.numbers.has(number)) info.complete = false;
    else info.numbers.add(number);
    discs.set(disc, info);
  }
  const taggedNumber = (part: MediaItemSummary): number => discs.get(part.audiobook?.disc_number ?? 1)?.complete ? part.audiobook?.part_number ?? 0 : 0;
  return parts.sort((a, b) =>
    (a.audiobook?.disc_number ?? 1) - (b.audiobook?.disc_number ?? 1)
    || taggedNumber(a) - taggedNumber(b)
    || (naturalKey(a.relative_path) < naturalKey(b.relative_path) ? -1 : Number(naturalKey(a.relative_path) > naturalKey(b.relative_path)))
    || a.id - b.id);
}

function sectionFileKey(path: string): string {
  return path.split(/[\\/]/).pop()!.toLowerCase().replace(/\.[^.]+$/, '').replace(/_\d+kb$/, '');
}

/** LibriVox section starts can follow the matching local files without accumulating MP3 padding. */
function providerSectionTargets(book: MediaItemDetail, parts: MediaItemSummary[]): AudioChapterTarget[] | undefined {
  const metadata = book.audiobook, files = metadata?.chapter_files;
  if (metadata?.recording?.provider_id !== 'librivox' || !files?.length || files.length !== metadata.chapters.length || files.length !== parts.length) return undefined;
  const used = new Set<number>(), targets: AudioChapterTarget[] = [];
  for (const [index, file] of files.entries()) {
    const matches = parts.filter((part) => file && sectionFileKey(part.relative_path) === sectionFileKey(file));
    const part = matches[0], chapter = metadata.chapters[index];
    if (matches.length !== 1 || !part.playable || part.missing_since || !part.duration_ms || used.has(part.id) || Math.abs(part.duration_ms - (chapter.end_ms - chapter.start_ms)) > 2000) return undefined;
    used.add(part.id); targets.push({ title: chapter.title, itemId: part.id, startMs: 0 });
  }
  return targets;
}

/** Provider marks require an explicitly selected recording and a matching complete runtime. */
export function audiobookChapterTargets(book: MediaItemDetail): AudioChapterTarget[] {
  const parts = orderedAudiobookParts(book.children);
  const embedded = parts.flatMap((part) => !part.missing_since && part.playable
    ? (part.audiobook?.chapters ?? []).map((chapter) => ({ title: chapter.title, itemId: part.id, startMs: chapter.start_ms })) : []);
  if (embedded.length) return embedded;
  const sections = providerSectionTargets(book, parts);
  if (sections) return sections;
  const chapters = book.audiobook?.recording ? book.audiobook.chapters : [];
  if (!chapters.length || !parts.length || parts.some((part) => part.missing_since || !part.playable || !(part.duration_ms && part.duration_ms > 0))) return [];
  const total = parts.reduce((sum, part) => sum + part.duration_ms!, 0);
  if (Math.abs(chapters[chapters.length - 1].end_ms - total) > 2000) return [];
  return chapters.flatMap((chapter) => {
    let offset = 0;
    for (const part of parts) {
      const end = offset + part.duration_ms!;
      if (chapter.start_ms >= offset && chapter.start_ms < end)
        return [{ title: chapter.title, itemId: part.id, startMs: chapter.start_ms - offset }];
      offset = end;
    }
    return [];
  });
}

function renderCredits(metadata: NonNullable<MediaItemSummary['audiobook']>): string {
  const series = [metadata.series, metadata.series_position].filter(Boolean).join(' · ');
  return [metadata.author && `By ${metadata.author}`, metadata.narrator && `Narrated by ${metadata.narrator}`, metadata.language, series]
    .filter((value): value is string => Boolean(value)).map((value) => `<p>${escapeHtml(value)}</p>`).join('');
}

/** Split only explicit track prefixes; trimming delimiters avoids backtracking on long titles. */
function audioTitleNumber(title: string): { number: string; title: string } | undefined {
  const text = title.trim(), prefix = /^(?:(?:track|part)\s+)?(\d{1,4}[a-z]?)/i.exec(text);
  if (!prefix) return;
  const remainder = text.slice(prefix[0].length), trimmed = remainder.trimStart();
  const separator = remainder.length !== trimmed.length || ['-', '_'].includes(remainder[0])
    || ['.', ':'].includes(remainder[0]) && remainder.length > 1 && remainder[1].trim() === '';
  if (!separator) return;
  const content = ['-', '.', ':', '_'].includes(trimmed[0]) ? trimmed.slice(1).trimStart() : trimmed;
  return content ? { number: prefix[1], title: content } : undefined;
}

function renderAudioFile(part: MediaItemSummary, index: number, parts: MediaItemSummary[], bookId: number, multipleDiscs: boolean): string {
  const prefix = audioTitleNumber(part.display_title);
  const numbered = prefix && (Number.parseInt(prefix.number, 10) <= parts.length || Number.parseInt(prefix.number, 10) === part.audiobook?.part_number);
  const title = numbered ? prefix.title : part.display_title;
  const ordinal = numbered ? prefix.number : String(part.audiobook?.part_number ?? index + 1);
  const number = multipleDiscs ? `${part.audiobook?.disc_number ?? 1}.${ordinal}` : ordinal;
  const playback = activeMusicPlayback();
  return renderAudioTrackRow(part, {
    number, title, subtitle: '', isCurrent: playback?.item.id === part.id && !playback.ended,
    menuId: `audiobook-${bookId}-${part.id}`, playAttributes: `data-play-music-track-id="${part.id}"`,
    actions: `<button type="button" data-item-id="${part.id}">File details</button>`, warnings: renderTrackWarnings(part, false),
  });
}

function recordingDescription(recording: AudiobookRecording): string {
  let format: string | undefined;
  if (recording.abridged != null) format = recording.abridged ? 'Abridged' : 'Unabridged';
  return [recording.narrator || 'Narrator unknown', recording.release_year, recording.language,
    format,
    recording.runtime_ms ? formatDuration(recording.runtime_ms) : undefined].filter(Boolean).join(' · ');
}

function renderRecordingChoices(book: MediaItemDetail, canEdit: boolean, request?: AudiobookRecordingChoices): string {
  if (!canEdit || book.item_type !== 'audiobook') return '';
  const current = request?.itemId === book.id ? request : undefined;
  const button = `<button type="button" class="secondary-button" data-audiobook-versions ${current?.loading ? 'disabled' : ''}>${renderButtonContent(current?.loading ? 'Finding versions…' : 'Choose recording', 'book')}</button>`;
  if (!current || current.loading) return button;
  const selectedRecording = book.audiobook?.recording;
  const workId = selectedRecording?.external_id.replace(/\/recording$/, '').split('/');
  if (workId && selectedRecording?.provider_id === 'audiosilo') workId.pop();
  const localChoice = selectedRecording ? `<button type="button" class="secondary-button" data-link-metadata="${book.id}:${escapeHtml(selectedRecording.provider_id)}:${escapeHtml(workId!.join('/'))}:audiobook">Use local recording details</button>` : '';
  const choices = current.choices.map((recording) => {
    const selected = book.audiobook?.recording?.external_id === recording.external_id && book.audiobook.recording.provider_id === recording.provider_id;
    const source = renderMetadataSearchProviderAttribution(recording.provider_id);
    const artwork = recording.artwork_url ? `<img class="audiobook-recording-cover" src="${escapeHtml(resolveApiUrl(recording.artwork_url))}" alt="" loading="lazy" />` : '';
    const action = selected ? 'disabled' : `data-link-metadata="${book.id}:${escapeHtml(recording.provider_id)}:${escapeHtml(recording.external_id)}:audiobook"`;
    const runtimeMismatch = recording.runtime_ms && book.duration_ms && Math.abs(recording.runtime_ms - book.duration_ms) > 120000;
    const warning = runtimeMismatch ? `<p class="muted">Runtime differs from local files (${formatDuration(book.duration_ms)}). Check the edition.</p>` : '';
    return `<div class="audiobook-recording-choice">${artwork}<div class="audiobook-recording-copy"><strong>${escapeHtml(recordingDescription(recording))}</strong><p class="muted audiobook-recording-source">${source}<span>· ${escapeHtml(recording.title)}</span></p>${warning}</div><button type="button" class="secondary-button" ${action}>${selected ? 'Selected' : 'Use this recording'}</button></div>`;
  }).join('');
  const contents = current.error ? `<p role="alert">${escapeHtml(current.error)}</p>` : choices || '<p class="muted">No matching recordings found. You can also search by title and author in Details and management.</p>';
  return `${button}<div class="audiobook-recording-choices"><p class="muted">Choose a version only when its narrator and edition match your files. Local playback and embedded chapters work without a provider version.</p>${contents}${localChoice}</div>`;
}

function renderLocalVersions(book: MediaItemDetail): string {
  if ((book.audiobook_versions?.length ?? 0) < 2) return '';
  const buttons = book.audiobook_versions!.map((version) => {
    const duration = version.duration_ms ? ` · ${formatDuration(version.duration_ms)}` : '';
    return `<button type="button" class="secondary-button" data-item-id="${version.item_id}" ${version.item_id === book.id ? 'disabled aria-current="true"' : ''}>${escapeHtml(version.label)}${duration}</button>`;
  }).join('');
  return `<h4>Local versions</h4><div class="audiobook-versions">${buttons}</div>`;
}

function renderRecordingIdentity(book: MediaItemDetail): string {
  if (book.item_type !== 'audiobook') return '';
  const metadata = book.audiobook!;
  const folder = metadata.version_label ? ` · ${metadata.version_label}` : '';
  const label = metadata.recording ? recordingDescription(metadata.recording) : 'Recording unconfirmed' + folder;
  return `<p>${escapeHtml(label)}</p>`;
}

/** Explain why provider chapter timing is unavailable without inventing chapter boundaries. */
export function audiobookChapterStatus(book: MediaItemDetail): string {
  if (audiobookChapterTargets(book).length) return '';
  if (!book.audiobook?.recording) return 'No embedded chapter markers found. Choose the recording to check for provider chapters.';
  const chapters = book.audiobook.chapters;
  if (!chapters.length) return 'The selected recording has no chapter markers from its metadata providers.';
  const parts = orderedAudiobookParts(book.children);
  if (!parts.length || parts.some((part) => part.missing_since || !part.playable || !part.duration_ms)) return 'Chapter timing cannot be checked until all audio files and their durations are available.';
  const total = parts.reduce((sum, part) => sum + (part.duration_ms ?? 0), 0);
  return `Chapter timing does not match these files: the provider timeline is ${formatDuration(chapters[chapters.length - 1].end_ms)}, and the local recording is ${formatDuration(total)}. Check the narration and edition. You can still play the files in order.`;
}

/** Render recording credits, original files, and real chapter starts without inferring chapters. */
export function renderAudiobookContents(book: MediaItemDetail, canEdit = false, choices?: AudiobookRecordingChoices): string {
  if (!book.audiobook) return '';
  const metadata = book.audiobook;
  const chapters = book.item_type === 'audiobook' ? audiobookChapterTargets(book) : metadata.chapters.map((chapter) => ({ title: chapter.title, itemId: book.id, startMs: chapter.start_ms }));
  const buttons = chapters.map((chapter) => `<button type="button" class="secondary-button" data-playback-target-item-id="${chapter.itemId}" data-playback-target-start-ms="${chapter.startMs}">${renderButtonContent(chapter.title, 'play', 'start', chapter.startMs === 0 ? '0:00' : formatDuration(chapter.startMs))}</button>`).join('');
  const status = book.item_type === 'audiobook' ? audiobookChapterStatus(book) : 'No embedded chapter markers in this file.';
  const chapterList = buttons ? `<h4>Chapters</h4><div class="audiobook-chapters">${buttons}</div>` : `<p class="muted">${escapeHtml(status)}</p>`;
  const parts = orderedAudiobookParts(book.children);
  const multipleDiscs = new Set(parts.map((part) => part.audiobook?.disc_number ?? 1)).size > 1;
  const files = parts.map((part, index) => renderAudioFile(part, index, parts, book.id, multipleDiscs)).join('');
  const fileList = files ? `<h4>Audio files</h4><div class="music-track-list audiobook-track-list">${files}</div>` : '';
  const work = metadata.openlibrary_work_id;
  const workLink = work && /^works\/OL\d+W$/.test(work) ? `<p><a href="https://openlibrary.org/${escapeHtml(work)}" target="_blank" rel="noopener noreferrer">Book details on Open Library</a></p>` : '';
  return `<section class="panel page-panel item-section audiobook-contents" data-nav-section="audiobook" data-nav-label="Audiobook"><div class="section-heading"><h3>Audiobook version</h3></div>${renderCredits(metadata)}${renderRecordingIdentity(book)}${renderRecordingChoices(book, canEdit, choices)}${workLink}${renderLocalVersions(book)}${chapterList}${fileList}</section>`;
}
