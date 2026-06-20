<script lang="ts">
  // LibrarySettings — existing-library cards (all fields + scan/refresh/
  // delete-missing/remove actions) + add-library form. Port of
  // renderLibrarySettingsPage (../client-web/src/app/settingsView.ts:576-636)
  // + card helpers (205-273) + form helpers (51-80, 424-449).
  //
  // Provider/kind sync: metadata-provider checkboxes filter by library kind
  // (supported_kinds). Provider language mode toggles the manual-languages
  // select visibility (port of syncAddLibraryProviderOptions).
  import Button from '../Button.svelte';
  import { settings, libraries, auth, ui, activities } from '$lib/stores';
  import { persistedLibraryForSettings } from '$lib/selectors';
  import type { MediaLibrarySettings, SettingsSnapshot } from '$lib/api';

  // --- Option lists (vanilla settingsView.ts:40-49, 424-449) ---

  const KIND_OPTIONS = [
    ['movies', 'Movies'],
    ['shows', 'Shows'],
    ['music', 'Music'],
    ['photos', 'Photos'],
    ['books', 'Books'],
    ['home_videos', 'Home videos'],
  ] as const;

  const SCANNER_OPTIONS = [
    ['auto', 'Auto'],
    ['directory', 'Directory'],
    ['movies', 'Movies'],
    ['shows', 'Shows'],
    ['music', 'Music'],
    ['photos', 'Photos'],
    ['books', 'Books'],
  ] as const;

  const LANGUAGE_OPTIONS = [
    ['en-US', 'English (United States)'],
    ['en-GB', 'English (United Kingdom)'],
    ['es-ES', 'Spanish (Spain)'],
    ['fr-FR', 'French (France)'],
    ['de-DE', 'German (Germany)'],
    ['it-IT', 'Italian (Italy)'],
    ['ja-JP', 'Japanese (Japan)'],
    ['pt-BR', 'Portuguese (Brazil)'],
  ] as const;

  // Metadata providers filtered by library kind (for the checkbox fieldset).
  function providersForKind(kind: string) {
    return settings.metadataProviders.filter(
      (p) => p.supported_kinds.includes(kind) && p.role !== 'secondary',
    );
  }

  // --- Existing library cards: editable state ---

  // Deep clone of the libraries settings for local editing.
  let editingLibraries = $state<MediaLibrarySettings[]>([]);
  let saving = $state(false);

  $effect(() => {
    const s = settings.settings;
    if (s) {
      // Clone to local state so edits don't mutate the store until Save.
      editingLibraries = s.media.libraries.map((lib) => ({ ...lib, paths: [...lib.paths], metadata_providers: [...lib.metadata_providers], metadata_languages: [...lib.metadata_languages], allowed_user_ids: [...lib.allowed_user_ids] }));
    }
  });

  function pathsToText(paths: string[]): string {
    return paths.filter(Boolean).join('\n');
  }

  function pathsFromText(text: string): string[] {
    return text.split('\n').map((p) => p.trim()).filter(Boolean);
  }

  // Persisted-library actions
  function isScanPending(persistedId: number | undefined): boolean {
    if (!persistedId) return false;
    return (activities.systemActivities?.activities ?? []).some(
      (a) => a.category === 'library_scan' && a.library_id === persistedId && a.state !== 'completed' && a.state !== 'failed',
    );
  }

  async function scanLibrary(persistedId: number) {
    try {
      await libraries.scan(persistedId);
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : 'Failed to scan library.');
    }
  }

  async function refreshMetadata(persistedId: number) {
    try {
      await libraries.refreshMetadata(persistedId);
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : 'Failed to refresh metadata.');
    }
  }

  async function deleteMissing(persistedId: number) {
    try {
      await libraries.deleteMissing(persistedId);
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : 'Failed to delete missing items.');
    }
  }

  async function removeLibrary(index: number) {
    try {
      await settings.deleteLibrary(index);
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : 'Failed to remove library.');
    }
  }

  async function saveLibraries(event: SubmitEvent) {
    event.preventDefault();
    const current = settings.settings;
    if (!current) return;
    saving = true;
    try {
      const next: SettingsSnapshot = {
        ...current,
        media: { ...current.media, libraries: editingLibraries },
      };
      await settings.save(next);
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : 'Failed to save library settings.');
    } finally {
      saving = false;
    }
  }

  // --- Add-library form ---

  let newName = $state('');
  let newPaths = $state('');
  let newKind = $state('movies');
  let newScanner = $state('auto');
  let newRecursive = $state(true);
  let newLangMode = $state<'auto' | 'manual'>('auto');
  let newLanguages = $state<string[]>(['en-US']);
  let newUserIds = $state<number[]>([]);
  let newProviders = $state<string[]>(['tmdb']);
  let adding = $state(false);

  async function addLibrary(event: SubmitEvent) {
    event.preventDefault();
    const library: MediaLibrarySettings = {
      name: newName,
      path: pathsFromText(newPaths)[0] ?? '',
      paths: pathsFromText(newPaths),
      recursive: newRecursive,
      kind: newKind,
      scanner: newScanner,
      metadata_providers: newProviders,
      metadata_language_mode: newLangMode,
      metadata_languages: newLanguages,
      allowed_user_ids: newUserIds,
    };
    adding = true;
    try {
      await settings.addLibrary(library);
      // Reset form
      newName = '';
      newPaths = '';
      newKind = 'movies';
      newScanner = 'auto';
      newRecursive = true;
      newLangMode = 'auto';
      newLanguages = ['en-US'];
      newUserIds = [];
      newProviders = ['tmdb'];
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : 'Failed to add library.');
    } finally {
      adding = false;
    }
  }
</script>

<section class="panel page-panel settings-page-panel">
  <form class="settings-form" onsubmit={saveLibraries}>
    <section>
      <div class="section-heading">
        <h3>Libraries</h3>
      </div>
      <p class="muted">Each logical library can now contain multiple folders. Enter one folder per line.</p>
      <div class="settings-library-list">
        {#if editingLibraries.length === 0}
          <div class="empty-state tight">No libraries are configured yet.</div>
        {:else}
          {#each editingLibraries as library, index (index)}
            {@const persisted = persistedLibraryForSettings(library)}
            {@const scanPending = isScanPending(persisted?.id)}
            {@const missingFiles = persisted?.missing_files ?? 0}
            {@const missingItems = persisted?.missing_items ?? 0}
            {@const hasMissing = missingFiles > 0 || missingItems > 0}
            <section class="settings-library-card">
              <div class="settings-library-header">
                <div>
                  <p class="eyebrow">Library {index + 1}</p>
                  <h3>{library.name || `Library ${index + 1}`}</h3>
                  {#if persisted}
                    <div class="settings-library-tags">
                      {#if scanPending}<span class="tag warning">Scanning catalog</span>{/if}
                      <span class="tag {hasMissing ? 'warning' : 'success'}">{hasMissing ? `${missingItems} missing items` : 'No missing items'}</span>
                      {#if missingFiles > 0}<span class="tag warning">{missingFiles} missing files</span>{/if}
                    </div>
                  {/if}
                </div>
                <div class="settings-library-actions">
                  {#if persisted}
                    <Button variant="secondary" label={scanPending ? 'Scanning' : 'Scan now'} icon="refresh-cw" disabled={scanPending} onclick={() => scanLibrary(persisted.id)} />
                    <Button variant="secondary" label="Refresh metadata" icon="refresh-cw" onclick={() => refreshMetadata(persisted.id)} />
                    <Button variant="secondary" label="Delete missing" icon="trash-2" disabled={!hasMissing} onclick={() => deleteMissing(persisted.id)} />
                  {/if}
                  <Button variant="secondary" label="Remove library" icon="trash-2" onclick={() => removeLibrary(index)} />
                </div>
              </div>
              <div class="form-row">
                <label>Name<input bind:value={library.name} /></label>
                <label>Type
                  <select bind:value={library.kind}>
                    {#each KIND_OPTIONS as [value, label]}<option {value}>{label}</option>{/each}
                  </select>
                </label>
                <label>Scanner
                  <select bind:value={library.scanner}>
                    {#each SCANNER_OPTIONS as [value, label]}<option {value}>{label}</option>{/each}
                  </select>
                </label>
              </div>
              <label>Folders
                <textarea rows="4" placeholder="One folder per line" value={pathsToText(library.paths)} oninput={(e) => (library.paths = pathsFromText(e.currentTarget.value))}></textarea>
              </label>
              <div class="form-row">
                <label class="checkbox-inline"><input type="checkbox" bind:checked={library.recursive} /> Recursive scan</label>
              </div>
              <div class="form-row">
                <label>Provider language mode
                  <select bind:value={library.metadata_language_mode}>
                    <option value="auto">Auto</option>
                    <option value="manual">Manual</option>
                  </select>
                </label>
                {#if library.metadata_language_mode === 'manual'}
                  <label>Manual languages
                    <select multiple size="5" value={library.metadata_languages} onchange={(e) => (library.metadata_languages = [...(e.currentTarget as HTMLSelectElement).selectedOptions].map((o) => o.value))}>
                      {#each LANGUAGE_OPTIONS as [value, label]}<option {value} selected={library.metadata_languages.includes(value)}>{label}</option>{/each}
                    </select>
                  </label>
                {/if}
              </div>
              <div class="form-row">
                <label>Library access
                  <select multiple size="3" value={library.allowed_user_ids} onchange={(e) => (library.allowed_user_ids = [...(e.currentTarget as HTMLSelectElement).selectedOptions].map((o) => Number(o.value)))}>
                    {#each auth.users as user}<option value={user.id} selected={library.allowed_user_ids.includes(user.id)}>{user.username}{user.admin ? ' (admin)' : ''}</option>{/each}
                  </select>
                </label>
              </div>
              <fieldset>
                <legend>Metadata sources</legend>
                {#each providersForKind(library.kind) as provider (provider.id)}
                  <label class="checkbox-inline">
                    <input
                      type="checkbox"
                      checked={library.metadata_providers.includes(provider.id)}
                      onchange={() => {
                        library.metadata_providers = library.metadata_providers.includes(provider.id)
                          ? library.metadata_providers.filter((id) => id !== provider.id)
                          : [...library.metadata_providers, provider.id];
                      }}
                    />
                    {provider.display_name}
                  </label>
                {/each}
              </fieldset>
            </section>
          {/each}
        {/if}
      </div>
    </section>
    <div class="page-actions">
      <Button type="submit" label="Save library settings" icon="save" busy={saving} />
    </div>
  </form>

  <form class="settings-form add-library-form" onsubmit={addLibrary}>
    <section>
      <h3>Add library</h3>
      <label>Name<input bind:value={newName} placeholder="Movies" required /></label>
      <label>Folders
        <textarea rows="4" placeholder="C:/Media/Movies&#10;D:/Overflow/Movies" bind:value={newPaths} required></textarea>
      </label>
      <div class="form-row">
        <label>Type
          <select bind:value={newKind}>
            {#each KIND_OPTIONS as [value, label]}<option {value}>{label}</option>{/each}
          </select>
        </label>
        <label>Scanner
          <select bind:value={newScanner}>
            {#each SCANNER_OPTIONS as [value, label]}<option {value}>{label}</option>{/each}
          </select>
        </label>
        <label class="checkbox-inline"><input type="checkbox" bind:checked={newRecursive} /> Recursive scan</label>
      </div>
      <div class="form-row">
        <label>Provider language mode
          <select bind:value={newLangMode}>
            <option value="auto">Auto</option>
            <option value="manual">Manual</option>
          </select>
        </label>
        {#if newLangMode === 'manual'}
          <label>Manual languages
            <select multiple size="5" value={newLanguages} onchange={(e) => (newLanguages = [...(e.currentTarget as HTMLSelectElement).selectedOptions].map((o) => o.value))}>
              {#each LANGUAGE_OPTIONS as [value, label]}<option {value} selected={newLanguages.includes(value)}>{label}</option>{/each}
            </select>
          </label>
        {/if}
      </div>
      <div class="form-row">
        <label>Library access
          <select multiple size="3" value={newUserIds} onchange={(e) => (newUserIds = [...(e.currentTarget as HTMLSelectElement).selectedOptions].map((o) => Number(o.value)))}>
            {#each auth.users as user}<option value={user.id} selected={newUserIds.includes(user.id)}>{user.username}{user.admin ? ' (admin)' : ''}</option>{/each}
          </select>
        </label>
      </div>
      <fieldset>
        <legend>Metadata sources</legend>
        {#each providersForKind(newKind) as provider (provider.id)}
          <label class="checkbox-inline">
            <input
              type="checkbox"
              checked={newProviders.includes(provider.id)}
              onchange={() => {
                newProviders = newProviders.includes(provider.id)
                  ? newProviders.filter((id) => id !== provider.id)
                  : [...newProviders, provider.id];
              }}
            />
            {provider.display_name}
          </label>
        {/each}
      </fieldset>
    </section>
    <Button label="Add library" icon="plus" type="submit" busy={adding} />
  </form>
</section>
