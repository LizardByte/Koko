<script lang="ts">
  // MetadataSearchPanel — the interactive metadata search + link UI for the
  // item-detail support grid. Renders a search form (query/year/language/
  // provider checkboxes) + a results list of provider matches, each with a
  // Link button. Extracted from vanilla renderMetadataSearchPanel +
  // renderMetadataSearchResults + renderMetadataSearchProviderControls
  // (../client-web/src/app/itemPersonView.ts:37-160, 973-988).
  //
  // Shown only for movies/shows (canManuallyLinkMetadata); seasons/episodes
  // show the "inherited metadata" empty-state instead (handled by parent).
  import Button from './Button.svelte';
  import { item as itemStore, ui } from '$lib/stores';
  import { resolveApiUrl, type MediaItemDetail, type ItemMetadataResponse, type MetadataSearchResult } from '$lib/api';
  import {
    metadataProviderOptions,
    defaultMetadataSearchTitle,
    defaultMetadataSearchYear,
    defaultMetadataSearchLanguage,
    defaultMetadataSearchProviderIds,
  } from '$lib/selectors';
  import { libraries } from '$lib/stores';

  type Props = { itemValue: MediaItemDetail; metadata: ItemMetadataResponse | undefined };
  let { itemValue, metadata }: Props = $props();

  const library = $derived(libraries.byId(itemValue.library_id));
  const providers = $derived(metadataProviderOptions(metadata, library?.kind));

  // Form state — pre-filled with defaults from the current item/metadata.
  // These capture the initial values intentionally (the form shouldn't reset
  // when the user is mid-typing). Untracked reads avoid the Svelte warning.
  const initialTitle = defaultMetadataSearchTitle(itemValue, metadata);
  const initialYear = defaultMetadataSearchYear(itemValue, metadata);
  const initialLanguage = defaultMetadataSearchLanguage(metadata);
  const initialProviderIds = defaultMetadataSearchProviderIds(metadata, library?.kind);

  let query = $state(initialTitle);
  let year = $state(initialYear);
  let language = $state(initialLanguage);
  let selectedProviderIds = $state<string[]>(initialProviderIds);

  const results = $derived<MetadataSearchResult[]>(itemStore.metadataSearchResults);
  const searching = $derived(itemStore.metadataSearching);

  async function submitSearch(event: SubmitEvent) {
    event.preventDefault();
    try {
      await itemStore.searchMetadata(itemValue.id, {
        query: query.trim() || undefined,
        year: year.trim() || undefined,
        language: language.trim() || undefined,
        providers: selectedProviderIds.length ? selectedProviderIds : undefined,
      });
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : 'Failed to search metadata.');
    }
  }

  async function link(result: MetadataSearchResult) {
    try {
      await itemStore.linkMetadata(itemValue.id, {
        provider_id: result.provider_id,
        external_id: result.external_id,
        media_type: result.media_type,
      });
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : 'Failed to link metadata.');
    }
  }

  // Provider attribution label for a result.
  function providerName(providerId: string): string {
    return metadata?.providers.find((p) => p.id === providerId)?.display_name ?? providerId;
  }
</script>

<form class="metadata-search-form" onsubmit={submitSearch}>
  <input
    type="search"
    value={query}
    placeholder={defaultMetadataSearchTitle(itemValue, metadata) || 'Search query'}
    oninput={(e) => (query = (e.currentTarget as HTMLInputElement).value)}
    autocomplete="off"
  />
  <input
    type="number"
    min="1800"
    max="2200"
    value={year}
    placeholder={defaultMetadataSearchYear(itemValue, metadata) || 'Year'}
    oninput={(e) => (year = (e.currentTarget as HTMLInputElement).value)}
    autocomplete="off"
  />
  <input
    type="text"
    value={language}
    placeholder={defaultMetadataSearchLanguage(metadata)}
    oninput={(e) => (language = (e.currentTarget as HTMLInputElement).value)}
    autocomplete="off"
  />
  {#if providers.length}
    <div class="metadata-provider-picker">
      {#each providers as provider (provider.id)}
        <label class="checkbox-inline">
          <input
            type="checkbox"
            value={provider.id}
            checked={selectedProviderIds.includes(provider.id)}
            onchange={(e) => {
              const checked = (e.currentTarget as HTMLInputElement).checked;
              selectedProviderIds = checked
                ? [...selectedProviderIds, provider.id]
                : selectedProviderIds.filter((id) => id !== provider.id);
            }}
          />
          <span>{provider.display_name}</span>
        </label>
      {/each}
    </div>
  {/if}
  <Button type="submit" label="Search metadata" icon="search" busy={searching} />
</form>

<div class="metadata-search-list">
  {#if results.length === 0}
    <div class="empty-state tight">Search metadata providers to link rich metadata and artwork.</div>
  {:else}
    {#each results as result (result.provider_id + ':' + result.external_id)}
      <article class="metadata-search-card">
        {#if result.artwork_url}
          <img class="metadata-search-poster" src={resolveApiUrl(result.artwork_url)} alt="" loading="lazy" />
        {/if}
        <div>
          <strong>{result.title}</strong>
          <p>{result.overview ?? 'No overview available.'}</p>
          <div class="metadata-match-meta">
            <span>{providerName(result.provider_id)}</span>
            <span>{result.release_year ?? 'Unknown year'}</span>
            <span>{result.media_type}</span>
            {#if typeof result.score === 'number'}
              <span>{Math.round(result.score * 100)}% match</span>
            {/if}
          </div>
        </div>
        <Button variant="secondary" label="Link" icon="link-2" onclick={() => link(result)} />
      </article>
    {/each}
  {/if}
</div>
