import type { MetadataProviderStatus } from '../api';
import { resolveApiUrl } from '../api';
import { state } from './state';

/** Resolve the backend-provided attribution logo against the configured API server. */
export function providerAttributionLogo(providerId: string): string | undefined {
  const provider = (state.selectedItemMetadata?.providers ?? state.metadataProviders)
    .find((entry) => entry.id === providerId);
  const logoUrl = provider?.logo_dark_url ?? provider?.logo_light_url;
  return logoUrl ? resolveApiUrl(logoUrl) : undefined;
}

export function providerStatus(providerId: string): MetadataProviderStatus | undefined {
  return state.metadataProviders.find((provider) => provider.id === providerId);
}

export function providerDisplayName(providerId: string): string {
  return providerStatus(providerId)?.display_name ?? providerId;
}

export function libraryProviderOptions(libraryKind?: string): MetadataProviderStatus[] {
  return state.metadataProviders
    .filter((provider) => !libraryKind || provider.supported_kinds.includes(libraryKind));
}
