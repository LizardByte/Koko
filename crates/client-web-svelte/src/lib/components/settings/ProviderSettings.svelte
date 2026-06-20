<script lang="ts">
  // ProviderSettings — metadata provider configuration cards + clear-cache.
  // Port of renderProviderSettingsPage (settingsView.ts:503-526) + card
  // (451-501). Provider priority reordering via move-up/move-down buttons
  // (reorders the settings.metadata.providers array) + dependency sync
  // (secondary providers disabled when their primary isn't enabled).
  // Port of eventBindings.ts:538-553 + syncProviderDependencyOptions (1329).
  import Button from '../Button.svelte';
  import Icon from '../Icon.svelte';
  import { settings, ui } from '$lib/stores';
  import type { SettingsSnapshot, MetadataProviderSettings, MetadataProviderStatus } from '$lib/api';

  // Local editable copy of the providers array (reordered by move buttons).
  let editingProviders = $state<MetadataProviderSettings[]>([]);
  let saving = $state(false);

  $effect(() => {
    const s = settings.settings;
    if (s) {
      editingProviders = s.metadata.providers.map((p) => ({ ...p }));
    }
  });

  // Provider status (global) for logos, descriptions, role, requires_api_key.
  function statusFor(providerId: string): MetadataProviderStatus | undefined {
    return settings.metadataProviders.find((p) => p.id === providerId);
  }

  function isPrimary(providerId: string): boolean {
    return statusFor(providerId)?.role !== 'secondary';
  }

  // Whether a secondary provider is available (its primary is enabled).
  function isSecondaryAvailable(providerId: string): boolean {
    const status = statusFor(providerId);
    if (status?.role !== 'secondary') return true;
    return status.extends_provider_ids.some((primaryId) =>
      editingProviders.some((p) => p.id === primaryId && p.enabled),
    );
  }

  // Priority label for a primary provider (1-based, counting only primaries).
  function priorityLabel(providerId: string): string {
    if (!isPrimary(providerId)) return 'Secondary';
    let priority = 0;
    for (const p of editingProviders) {
      if (isPrimary(p.id)) priority++;
      if (p.id === providerId) return `Priority ${priority}`;
    }
    return '';
  }

  // Move a provider up/down in the array (changes priority order).
  function moveProvider(index: number, direction: 'up' | 'down') {
    const targetIndex = direction === 'up' ? index - 1 : index + 1;
    if (targetIndex < 0 || targetIndex >= editingProviders.length) return;
    const arr = [...editingProviders];
    [arr[index], arr[targetIndex]] = [arr[targetIndex], arr[index]];
    editingProviders = arr;
  }

  // Toggle provider enabled (with dependency sync: disabling a primary
  // disables its secondaries).
  function toggleProvider(index: number) {
    const provider = editingProviders[index];
    const newEnabled = !provider.enabled;
    editingProviders[index] = { ...provider, enabled: newEnabled };
    editingProviders = [...editingProviders];
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    const current = settings.settings;
    if (!current) return;
    saving = true;
    try {
      // Only update the metadata.providers section.
      const next: SettingsSnapshot = {
        ...current,
        metadata: { ...current.metadata, providers: editingProviders },
      };
      await settings.save(next);
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : 'Failed to save provider settings.');
    } finally {
      saving = false;
    }
  }

  let clearingCache = $state(false);
  async function clearCache() {
    clearingCache = true;
    try {
      await settings.clearMetadataCache();
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : 'Failed to clear metadata cache.');
    } finally {
      clearingCache = false;
    }
  }
</script>

<section class="panel page-panel settings-page-panel">
  <form class="settings-form" onsubmit={save}>
    <section>
      <div class="section-heading section-heading-actions">
        <h3>Metadata providers</h3>
        <Button variant="secondary" label="Clear metadata cache" icon="trash-2" busy={clearingCache} onclick={clearCache} />
      </div>
      {#each editingProviders as provider, index (provider.id)}
        {@const status = statusFor(provider.id)}
        {@const showApiKey = Boolean(status?.requires_api_key)}
        {@const apiKeyConfigured = Boolean(provider.api_key_configured || provider.api_key_secret_ref || provider.api_key)}
        {@const showRequestSettings = provider.id !== 'local_nfo'}
        {@const logoUrl = status?.logo_dark_url ?? status?.logo_light_url}
        {@const available = isSecondaryAvailable(provider.id)}
        <section class="settings-library-card provider-settings-card" id="provider-{provider.id}">
          <div class="settings-library-header">
            <div class="provider-settings-title">
              {#if logoUrl}<img class="provider-settings-logo" src={logoUrl} alt="" />{/if}
              <div>
                <p class="eyebrow">Provider</p>
                <h3>{status?.display_name ?? provider.id}</h3>
              </div>
            </div>
            <div class="provider-tags">
              <span class="tag">{priorityLabel(provider.id)}</span>
              <label class="checkbox-inline">
                <input type="checkbox" checked={provider.enabled} disabled={!available} onchange={() => toggleProvider(index)} /> Enabled
              </label>
            </div>
          </div>
          {#if status?.description}<p class="muted">{status.description}</p>{/if}
          {#if status?.attribution_text}<p class="muted">{status.attribution_text}</p>{/if}
          {#if showApiKey || showRequestSettings}
            <div class="form-row">
              {#if showApiKey}
                <label>API key<input type="password" placeholder={apiKeyConfigured ? 'Saved' : ''} autocomplete="new-password" bind:value={editingProviders[index].api_key} /></label>
              {/if}
              {#if showApiKey && apiKeyConfigured}
                <label class="checkbox-inline"><input type="checkbox" bind:checked={editingProviders[index].clear_api_key} /> Clear saved API key</label>
              {/if}
              {#if showRequestSettings}
                <label>Rate limit (requests/second)<input type="number" min="1" bind:value={editingProviders[index].rate_limit_per_second} /></label>
                <label>Retry attempts<input type="number" min="0" bind:value={editingProviders[index].retry_attempts} /></label>
                <label>Retry backoff (ms)<input type="number" min="1" step="1" bind:value={editingProviders[index].retry_backoff_ms} /></label>
              {/if}
            </div>
          {:else}
            <p class="muted">This provider does not require provider-specific settings.</p>
          {/if}
          {#if isPrimary(provider.id)}
            <div class="provider-option-actions">
              <button type="button" class="secondary-button icon-only" title="Move up" aria-label="Move up" disabled={index === 0} onclick={() => moveProvider(index, 'up')}><Icon name="chevron-up" size={16} /></button>
              <button type="button" class="secondary-button icon-only" title="Move down" aria-label="Move down" disabled={index === editingProviders.length - 1} onclick={() => moveProvider(index, 'down')}><Icon name="chevron-down" size={16} /></button>
            </div>
          {/if}
        </section>
      {/each}
    </section>
    <div class="page-actions">
      <Button type="submit" label="Save provider settings" icon="save" busy={saving} />
    </div>
  </form>
</section>
