import type { MediaItemDetail, MediaItemSummary } from '../api';
import { escapeHtml } from './format';
import { providerAttributionLogo, providerDisplayName } from './providers';

/** Attribute a rating to its actual provider and identify inherited season/series values. */
export function renderRating(item: MediaItemSummary | MediaItemDetail): string {
  if (typeof item.rating !== 'number' || !Number.isFinite(item.rating)) return '';
  const sourceType = 'rating_source_item_type' in item ? item.rating_source_item_type : item.item_type;
  let inheritedLabel = '';
  if (sourceType !== item.item_type) {
    if (sourceType === 'show') inheritedLabel = 'Series';
    else if (sourceType === 'season') inheritedLabel = 'Season';
  }
  const providerId = item.rating_provider_id;
  const providerName = providerId ? providerDisplayName(providerId) : undefined;
  const logo = providerId ? providerAttributionLogo(providerId) : undefined;
  const value = item.rating.toFixed(1);
  const ratingLabel = inheritedLabel ? `${inheritedLabel} rating` : 'Rating';
  const sourceLabel = providerName ? ` from ${providerName}` : '';
  const label = `${ratingLabel}${sourceLabel}: ${value}`;
  let attribution = '';
  if (logo) {
    attribution = `<img class="rating-source-logo" src="${escapeHtml(logo)}" alt="" aria-hidden="true" />`;
  } else if (providerName) {
    attribution = `<span class="rating-source-name">${escapeHtml(providerName)}</span>`;
  }
  const inheritedMarkup = inheritedLabel ? `<span>${inheritedLabel}</span>` : '';
  return `<span class="tag rating-tag" title="${escapeHtml(label)}" aria-label="${escapeHtml(label)}">${inheritedMarkup}${attribution}<span>${value}</span></span>`;
}
