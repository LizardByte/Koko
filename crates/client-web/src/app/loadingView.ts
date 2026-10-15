import { escapeHtml } from './format';

/** Keep the shape of the incoming content without putting placeholder controls in navigation. */
export function renderLoadingCards(label: string, count = 6, layout = 'posters'): string {
  return `<div class="content-loading" role="status" aria-label="${escapeHtml(label)}">
    <span class="sr-only">${escapeHtml(label)}</span>
    <div class="loading-card-grid ${layout === 'episodes' ? 'loading-episodes' : layout === 'tiles' ? 'loading-tiles' : ''}" aria-hidden="true">
      ${Array.from({ length: count }, () => '<div class="loading-card"><div class="loading-art skeleton"></div><div class="loading-line skeleton"></div><div class="loading-line short skeleton"></div></div>').join('')}
    </div>
  </div>`;
}

export function renderLoadingDetail(label: string, itemLayout = false): string {
  if (itemLayout) {
    return `<section class="item-page item-detail-page item-detail-loading" role="status" aria-label="${escapeHtml(label)}" aria-busy="true">
      <span class="sr-only">${escapeHtml(label)}</span>
      <aside class="item-cover-column" aria-hidden="true"><div class="detail-art item-poster skeleton"></div></aside>
      <header class="item-identity" aria-hidden="true"><div class="loading-line skeleton"></div><div class="loading-line short skeleton"></div></header>
      <div class="item-action-dock" aria-hidden="true"><div class="loading-action skeleton"></div></div>
      <section class="item-description-section" aria-hidden="true"><div class="loading-description skeleton"></div></section>
      <div class="item-support-content">${renderLoadingCards(label)}</div>
    </section>`;
  }
  return `<section class="item-page"><div class="item-hero loading-detail" role="status" aria-label="${escapeHtml(label)}">
    <span class="sr-only">${escapeHtml(label)}</span>
    <div class="detail-art skeleton" aria-hidden="true"></div>
    <div class="detail-summary" aria-hidden="true"><div class="loading-line skeleton"></div><div class="loading-line short skeleton"></div><div class="loading-description skeleton"></div><div class="loading-action skeleton"></div></div>
  </div>${renderLoadingCards(label)}</section>`;
}
