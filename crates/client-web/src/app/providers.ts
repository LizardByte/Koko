/** Resolves provider names, attribution assets, and library metadata-source options. */

import type { MetadataProviderStatus } from '../api';
import { resolveApiUrl } from '../api';
import { state } from './state';
import { escapeHtml } from './format';

/** Render the provider's supplied attribution image, falling back to its accessible name. */
export function renderMetadataSearchProviderAttribution(providerId: string): string {
  const label = providerDisplayName(providerId);
  const logoUrl = providerAttributionLogo(providerId);
  return logoUrl
    ? `<img class="metadata-attribution-logo" src="${escapeHtml(logoUrl)}" alt="${escapeHtml(label)}" title="${escapeHtml(label)}" loading="lazy" />`
    : `<span>${escapeHtml(label)}</span>`;
}

/** Resolve the backend-provided attribution logo against the configured API server. */
export function providerAttributionLogo(providerId: string): string | undefined {
  const provider = state.selectedItemMetadata?.providers.find((entry) => entry.id === providerId)
    ?? state.metadataProviders.find((entry) => entry.id === providerId);
  const logoUrl = provider?.logo_dark_url ?? provider?.logo_light_url;
  return logoUrl ? resolveApiUrl(logoUrl) : undefined;
}

export function providerStatus(providerId: string): MetadataProviderStatus | undefined {
  return state.metadataProviders.find((provider) => provider.id === providerId);
}

export function providerDisplayName(providerId: string): string {
  return state.selectedItemMetadata?.providers.find((provider) => provider.id === providerId)?.display_name
    ?? providerStatus(providerId)?.display_name ?? providerId;
}

export function libraryProviderOptions(libraryKind?: string): MetadataProviderStatus[] {
  return state.metadataProviders
    .filter((provider) => !libraryKind || provider.supported_kinds.includes(libraryKind));
}
