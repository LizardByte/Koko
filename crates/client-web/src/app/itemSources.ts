/** Lazy, paginated source inspection shared by all present and future item kinds. */
import { ApiError, getItemSourceFiles, type MediaItemDetail, type MediaItemSourceFile } from '../api';
import { escapeHtml, formatBitRate, formatDuration, formatFileSize, formatTimestamp } from './format';
import { state } from './state';

/** Discard abandoned requests and avoid re-fetching on every details toggle/render. */
export async function loadItemSources(render: () => void, offset?: number, request = getItemSourceFiles): Promise<void> {
  const itemId = state.selectedItem?.id;
  if (itemId === undefined || state.route.page !== 'item' || state.route.itemId !== itemId) return;
  const userId = state.bootstrap?.current_user?.id;
  const previous = state.itemSources?.itemId === itemId && state.itemSources.userId === userId ? state.itemSources : undefined;
  if (previous?.loading || (offset === undefined && previous)) return;
  const pending = { itemId, userId, loading: true, data: previous?.data, error: undefined as string | undefined };
  state.itemSources = pending;
  render();
  const current = () => state.itemSources === pending && state.selectedItem?.id === itemId
    && state.route.page === 'item' && state.route.itemId === itemId && state.bootstrap?.current_user?.id === userId;
  try {
    const data = await request(itemId, offset ?? 0);
    if (current()) pending.data = data;
  } catch (error) {
    if (!current()) return;
    if (error instanceof ApiError && [401, 403, 404].includes(error.status)) pending.data = undefined;
    pending.error = error instanceof Error ? error.message : 'Unable to load source files.';
  } finally {
    if (current()) { pending.loading = false; render(); }
  }
}

function fact(label: string, value: string): string {
  return `<div><span class="label">${escapeHtml(label)}</span><strong>${escapeHtml(value)}</strong></div>`;
}

function renderFile(file: MediaItemSourceFile, itemId: number): string {
  const technical = [file.container?.toUpperCase(), file.video_codec, file.audio_codec,
    file.duration_ms ? formatDuration(file.duration_ms) : undefined,
    file.width && file.height ? `${file.width}×${file.height}` : undefined,
    file.bit_rate ? formatBitRate(file.bit_rate) : undefined,
    file.file_size === 0 ? '0 B' : formatFileSize(file.file_size)].filter(Boolean).join(' · ');
  return `<li class="source-file">
    <div class="section-heading section-heading-actions"><strong>${escapeHtml(file.display_title)}</strong>
      ${file.item_id !== itemId ? `<button type="button" class="secondary-button" data-item-id="${file.item_id}">Open item</button>` : ''}
    </div>
    ${file.missing_since ? `<p class="status-warning">Missing since ${escapeHtml(formatTimestamp(file.missing_since))}</p>` : ''}
    <dl class="source-file-paths"><dt>Full path</dt><dd>${escapeHtml(file.path)}</dd>
      <dt>Library folder</dt><dd>${escapeHtml(file.source_root_path)}</dd>
      <dt>Relative path</dt><dd>${escapeHtml(file.relative_path)}</dd></dl>
    <p class="muted">${escapeHtml(technical)}</p>
    ${file.modified_at ? `<p class="muted">File modified ${escapeHtml(formatTimestamp(file.modified_at))}</p>` : ''}
  </li>`;
}

/** Paths are text only, so server paths remain selectable on any client platform. */
export function renderItemSources(item: MediaItemDetail, libraryName: string): string {
  const sources = state.itemSources?.itemId === item.id && state.itemSources.userId === state.bootstrap?.current_user?.id ? state.itemSources : undefined;
  const data = sources?.data;
  const offset = data?.offset ?? 0;
  const button = (label: string, page: number, disabled = false) => `<button type="button" class="secondary-button" data-item-sources-page="${page}" ${sources?.loading || disabled ? 'disabled' : ''}>${label}</button>`;
  return `<section class="panel page-panel item-section item-sources" aria-label="Source files">
    <div class="section-heading section-heading-actions"><h3>Source files</h3>${button('Refresh files', offset)}</div>
    <div class="item-info-list">${fact('Library', libraryName)}${fact('Item type', item.item_type)}
      ${data ? fact('Local catalog title', data.catalog_title) : ''}
      ${data ? fact('Files', `${data.total_files}${data.missing_files ? ` (${data.missing_files} missing)` : ''}`) + fact('Total size', data.total_size ? formatFileSize(data.total_size) : '0 B') : ''}</div>
    ${sources?.loading ? '<p role="status">Loading source files…</p>' : ''}
    ${sources?.error ? `<p role="alert">${escapeHtml(sources.error)}</p>` : ''}
    ${!sources ? '<p class="muted">Source files load when media details are opened.</p>' : ''}
    ${data?.total_files === 0 ? '<p class="muted">No source files are attached to this item or its children.</p>' : ''}
    ${data ? `<ul class="source-file-list">${data.files.map((file) => renderFile(file, item.id)).join('')}</ul>` : ''}
    ${data && data.total_files > 0 ? `<div class="source-file-pagination">
      ${button('Previous', Math.max(0, offset - data.limit), offset === 0)}
      <span>${data.files.length ? offset + 1 : 0}–${offset + data.files.length} of ${data.total_files} files</span>
      ${button('Next', offset + data.limit, offset + data.limit >= data.total_files)}</div>` : ''}
  </section>`;
}
