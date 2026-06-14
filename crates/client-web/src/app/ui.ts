import type { BootstrapUser, MediaItemSummary } from '../api';
import { resolveApiUrl } from '../api';
import { COLLAPSIBLE_TEXT_LENGTH, COLLAPSIBLE_TEXT_LINE_COUNT } from './constants';
import { escapeHtml, formatDuration } from './format';
import { state } from './state';
import type { AppIconName } from './types';

export function renderCollapsibleText(text: string, key: string, className = 'hero-description'): string {
  const normalized = text.trim();
  const lineCount = normalized.split(/\r\n|\r|\n/).length;
  const shouldCollapse = normalized.length > COLLAPSIBLE_TEXT_LENGTH || lineCount > COLLAPSIBLE_TEXT_LINE_COUNT;
  const isExpanded = state.expandedTextKeys.has(key);
  const stateClass = shouldCollapse && !isExpanded ? 'is-collapsed' : '';
  const toggleExpanded = isExpanded ? 'true' : 'false';
  const toggleLabel = isExpanded ? 'show less' : '... see more';
  const toggle = shouldCollapse
    ? `<button type="button" class="text-toggle-button" data-toggle-text="${escapeHtml(key)}" aria-expanded="${toggleExpanded}">${toggleLabel}</button>`
    : '';

  return `
    <div class="collapsible-text ${className} ${stateClass}" data-collapsible-text="${escapeHtml(key)}">${escapeHtml(normalized)}</div>
    ${toggle}
  `;
}

export function setButtonBusy(button: HTMLButtonElement | null | undefined, busy: boolean): void {
  if (!button) {
    return;
  }
  button.disabled = busy;
  button.classList.toggle('is-busy', busy);
  button.setAttribute('aria-busy', busy ? 'true' : 'false');
}

export function selectedLibraryIcon(kind?: string): AppIconName {
  switch (kind) {
    case 'mixed':
      return 'layout-grid';
    case 'movies':
      return 'clapperboard';
    case 'shows':
      return 'tv';
    case 'music':
      return 'music';
    case 'photos':
      return 'image';
    case 'books':
      return 'book';
    case 'home_videos':
      return 'film';
    default:
      return 'layout-grid';
  }
}

export function renderIcon(iconName: AppIconName, className = 'rail-icon'): string {
  return `<span class="${className}"><i data-lucide="${iconName}"></i></span>`;
}

export function renderButtonContent(label: string, iconName?: AppIconName, iconPosition: 'start' | 'end' = 'start'): string {
  if (!iconName) {
    return escapeHtml(label);
  }

  return `
    <span class="button-content${iconPosition === 'end' ? ' icon-end' : ''}">
      ${renderIcon(iconName, 'button-icon')}
      <span>${escapeHtml(label)}</span>
    </span>
  `;
}

export function renderUserAvatar(user: BootstrapUser, className = ''): string {
  const imageUrl = user.profile_image_url ? resolveApiUrl(user.profile_image_url) : undefined;
  const initial = user.username.trim().charAt(0).toUpperCase() || '?';
  return `
    <span class="user-avatar ${className}">
      ${imageUrl
        ? `<img src="${escapeHtml(imageUrl)}" alt="" loading="lazy" />`
        : `<span>${escapeHtml(initial)}</span>`}
    </span>
  `;
}

export function humanizeItemType(itemType: string): string {
  switch (itemType) {
    case 'show':
      return 'Show';
    case 'season':
      return 'Season';
    case 'episode':
      return 'Episode';
    case 'movie':
      return 'Movie';
    case 'track':
      return 'Track';
    case 'photo':
      return 'Photo';
    case 'book':
      return 'Book';
    default:
      return itemType.replace(/_/g, ' ').replace(/\b\w/g, (character) => character.toUpperCase());
  }
}

export function formatChildCount(item: MediaItemSummary): string {
  if (!item.child_count) {
    return formatDuration(item.duration_ms);
  }

  if (item.item_type === 'show') {
    const seasonCount = item.available_season_count ?? item.child_count;
    return `${seasonCount} season${seasonCount === 1 ? '' : 's'}`;
  }

  if (item.item_type === 'season') {
    return `${item.child_count} episode${item.child_count === 1 ? '' : 's'}`;
  }

  return `${item.child_count} item${item.child_count === 1 ? '' : 's'}`;
}

export function libraryStatusLabel(status: string): string {
  switch (status) {
    case 'never_scanned':
      return 'Pending first scan';
    case 'available':
      return 'Ready';
    case 'missing_path':
      return 'Missing path';
    case 'not_directory':
      return 'Invalid folder';
    case 'unreadable':
      return 'Unreadable';
    case 'empty_path':
      return 'No folder';
    default:
      return status.replace(/_/g, ' ');
  }
}

export function renderPageNavbar(eyebrow: string, title: string, description: string, actions = ''): string {
  return `
    <header class="content-navbar panel page-panel">
      <div class="content-navbar-copy">
        <p class="eyebrow">${escapeHtml(eyebrow)}</p>
        <h2>${escapeHtml(title)}</h2>
        <p class="muted">${escapeHtml(description)}</p>
      </div>
      ${actions ? `<div class="content-navbar-actions">${actions}</div>` : ''}
    </header>
  `;
}

export function subtitleLanguage(trackLabel: string): string {
  const normalized = trackLabel.trim().toLowerCase();
  if (/^[a-z]{2,3}$/.test(normalized)) {
    return normalized;
  }

  return 'en';
}
