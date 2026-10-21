/** Derives media warnings and renders shared detail banners and track labels. */

import type { MediaItemSummary } from '../api';
import { itemIsMetadataPending } from './activities';
import { escapeHtml } from './format';
import { renderIcon } from './ui';

interface ItemWarning { kind: 'missing' | 'metadata'; title: string; message: string }

/** Keep card tooltips, detail banners, and track labels consistent across media types. */
export function itemWarnings(item: MediaItemSummary): ItemWarning[] {
  const warnings: ItemWarning[] = [];
  if (item.missing_since) warnings.push({ kind: 'missing', title: 'Missing from disk', message: 'Source files for this item are missing. Check the library folders and rescan the library.' });
  if (item.metadata_refresh_state === 'error' && !itemIsMetadataPending(item)) {
    const detail = item.metadata_refresh_error?.trim();
    const readable = detail && detail.length <= 300 && !/[<>{}\r\n]/.test(detail);
    warnings.push({ kind: 'metadata', title: 'Metadata refresh failed', message: readable ? detail : 'The last metadata refresh failed. Refresh this item’s metadata to try again.' });
  } else if (!item.has_metadata) {
    const pending = itemIsMetadataPending(item);
    warnings.push({ kind: 'metadata', title: pending ? 'Matching metadata' : 'Metadata not matched', message: pending ? 'Metadata matching has not finished yet. Artwork and descriptive information may still be incomplete.' : 'No metadata is linked to this item. Artwork and descriptive information may be missing until a match is selected.' });
  }
  return warnings;
}

export function renderItemWarnings(item: MediaItemSummary): string {
  const warnings = itemWarnings(item);
  if (!warnings.length) return '';
  const contents = warnings.map(warning => `<div><strong>${escapeHtml(warning.title)}</strong><p>${escapeHtml(warning.message)}</p></div>`).join('');
  return `<section class="item-warning-banner" role="status" aria-label="Item warnings">${renderIcon('triangle-alert')}<div>${contents}</div></section>`;
}

/** Audio files can omit metadata warnings when identity is matched on their parent recording. */
export function renderTrackWarnings(item: MediaItemSummary, includeMetadata = true): string {
  return itemWarnings(item).filter(warning => includeMetadata || warning.kind === 'missing').map(warning => `<span class="music-track-warning" title="${escapeHtml(warning.message)}">${renderIcon('triangle-alert')}${escapeHtml(warning.title)}</span>`).join('');
}
