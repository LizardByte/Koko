import { createIcons, icons } from 'lucide';
import { musicCardStatus, type MusicCardStatus, type MusicCardTarget } from './musicPlaybackState';
import { renderIcon } from './ui';
import { setElementHtml } from './domPatcher';

function statusContent(status: MusicCardStatus | undefined): string {
  if (!status) return '';
  const symbol = status === 'paused' ? renderIcon('pause') : '<span class="media-card-levels" aria-hidden="true"><span></span><span></span><span></span><span></span></span>';
  return `${symbol}<span class="sr-only">${status === 'paused' ? 'Paused' : 'Playing'}</span>`;
}

/** Shared active treatment for album, song, and radio cards, including lazy-loaded shelves. */
export function musicCardPresentation(target: MusicCardTarget): { className: string; attributes: string; badge: string } {
  const status = musicCardStatus(target);
  const attributes = Object.entries(target).map(([key, value]) => {
    const name = { itemId: 'item', albumId: 'album', queueId: 'queue' }[key as keyof MusicCardTarget];
    return value === undefined ? '' : `data-music-card-${name}="${value}"`;
  }).filter(Boolean).join(' ');
  return {
    className: status ? ` is-current is-${status}` : '',
    attributes: `${attributes}${status ? ' aria-current="true"' : ''}`,
    badge: `<span class="media-card-playing-status" data-music-card-status data-playback-status="${status ?? ''}" ${status ? '' : 'hidden'}>${statusContent(status)}</span>`,
  };
}

/** Patch active markers without replacing cards, moving focus, or disturbing shelf scrolling. */
export function syncMusicPlaybackCards(root: ParentNode = document): void {
  root.querySelectorAll<HTMLElement>('.media-card[data-music-card-item], .media-card[data-music-card-queue]').forEach((card) => {
    const target: MusicCardTarget = {};
    for (const [attribute, key] of [['musicCardItem', 'itemId'], ['musicCardAlbum', 'albumId'], ['musicCardQueue', 'queueId']] as const) {
      if (card.dataset[attribute] !== undefined) target[key] = Number(card.dataset[attribute]);
    }
    const status = musicCardStatus(target);
    card.classList.toggle('is-current', Boolean(status));
    card.classList.toggle('is-playing', status === 'playing');
    card.classList.toggle('is-paused', status === 'paused');
    if (status) card.setAttribute('aria-current', 'true'); else card.removeAttribute('aria-current');
    const badge = card.querySelector<HTMLElement>('[data-music-card-status]');
    if (!badge) return;
    badge.hidden = !status;
    if (badge.dataset.playbackStatus !== (status ?? '')) {
      setElementHtml(badge, statusContent(status), { preserveDom: true });
      badge.dataset.playbackStatus = status ?? '';
      createIcons({ icons, root: badge });
    }
  });
}
