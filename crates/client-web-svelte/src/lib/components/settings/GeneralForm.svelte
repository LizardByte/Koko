<script lang="ts">
  // GeneralForm — server config (data_dir, address, port, HTTPS/certs) +
  // ffmpeg paths. Port of renderGeneralSettingsPage
  // (../client-web/src/app/settingsView.ts:528-574).
  //
  // Uses Svelte bind:value for form state, building a SettingsSnapshot
  // reactively. Only the server/https + ffmpeg + general fields are included
  // in the save payload (vanilla gates these to the general section,
  // settingsView.ts:768-775 — the other sections manage their own fields).
  import Button from '../Button.svelte';
  import { goto } from '$app/navigation';
  import { settings, ui } from '$lib/stores';
  import type { SettingsSnapshot } from '$lib/api';

  // Local editable copies — initialized from the loaded settings.
  let dataDir = $state('');
  let address = $state('');
  let port = $state(0);
  let useHttps = $state(false);
  let useCustomCerts = $state(false);
  let certPath = $state('');
  let keyPath = $state('');
  let ffmpegPath = $state('');
  let ffprobePath = $state('');
  let saving = $state(false);

  // Initialize local state when settings load.
  $effect(() => {
    const s = settings.settings;
    if (s) {
      dataDir = s.general.data_dir;
      address = s.server.address;
      port = s.server.port;
      useHttps = s.server.use_https;
      useCustomCerts = s.server.use_custom_certs;
      certPath = s.server.cert_path;
      keyPath = s.server.key_path;
      ffmpegPath = s.ffmpeg.ffmpeg_path;
      ffprobePath = s.ffmpeg.ffprobe_path;
    }
  });

  async function save(event: SubmitEvent) {
    event.preventDefault();
    const current = settings.settings;
    if (!current) return;
    saving = true;
    try {
      const next: SettingsSnapshot = {
        ...current,
        general: { ...current.general, data_dir: dataDir },
        server: { address, port, use_https: useHttps, use_custom_certs: useCustomCerts, cert_path: certPath, key_path: keyPath },
        ffmpeg: { ffmpeg_path: ffmpegPath, ffprobe_path: ffprobePath },
      };
      await settings.save(next);
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : 'Failed to save settings.');
    } finally {
      saving = false;
    }
  }
</script>

<section class="panel page-panel settings-page-panel">
  <form class="settings-form" onsubmit={save}>
    <section>
      <h3>Server</h3>
      <label>Data directory<input bind:value={dataDir} /></label>
      <div class="form-row">
        <label>Address<input bind:value={address} /></label>
        <label>Port<input type="number" min="1" bind:value={port} /></label>
      </div>
      <div class="form-row checkbox-row">
        <label><input type="checkbox" bind:checked={useHttps} /> Use HTTPS</label>
        <label><input type="checkbox" bind:checked={useCustomCerts} /> Use custom certificates</label>
      </div>
      <div class="form-row">
        <label>Certificate path<input bind:value={certPath} /></label>
        <label>Key path<input bind:value={keyPath} /></label>
      </div>
    </section>

    <section>
      <h3>FFmpeg</h3>
      <div class="form-row">
        <label>ffmpeg path<input bind:value={ffmpegPath} /></label>
        <label>ffprobe path<input bind:value={ffprobePath} /></label>
      </div>
    </section>

    <section>
      <h3>Metadata providers</h3>
      <p class="muted">Provider credentials and refresh behavior are configured on their own settings page.</p>
      <Button variant="secondary" label="Open provider settings" icon="settings" onclick={() => goto('/settings/providers')} />
    </section>

    <div class="page-actions">
      <Button type="submit" label="Save settings" icon="save" busy={saving} />
      <Button variant="secondary" label="Back home" icon="house" onclick={() => goto('/')} />
    </div>
  </form>
</section>
