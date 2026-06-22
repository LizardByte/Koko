<script lang="ts">
  // ControlsHelp — a modal overlay showing gamepad + keyboard mappings for
  // the current context (browse vs player). Triggered by:
  //   - Gamepad: Select/Back button (button 8) or Guide/Home (button 16)
  //   - Keyboard: "?" key
  //   - Settings: a "Controls" link (future)
  import Icon from '../Icon.svelte';
  import { playback } from '$lib/stores';

  let {
    isOpen = false,
    onclose,
  }: { isOpen?: boolean; onclose?: () => void } = $props();

  const isPlayerContext = $derived(playback.isOpen);

  const browseControls = [
    { action: 'Navigate (up/down/left/right)', gamepad: 'D-pad / Left stick', keys: 'Arrow keys' },
    { action: 'Select / Activate', gamepad: 'A button', keys: 'Enter' },
    { action: 'Go back', gamepad: 'B button', keys: 'Backspace / Esc' },
    { action: 'Switch tab (left/right)', gamepad: 'L / R bumper', keys: '[ / ]' },
    { action: 'Scroll (right stick)', gamepad: 'Right stick', keys: 'Page Up/Down' },
    { action: 'Show this help', gamepad: 'Select / Back', keys: '?' },
  ];

  const playerControls = [
    { action: 'Play / Pause', gamepad: 'A button', keys: 'Space / K' },
    { action: 'Close player', gamepad: 'B button', keys: 'Escape' },
    { action: 'Seek (back / forward)', gamepad: 'D-pad / Left stick ← →', keys: '← →' },
    { action: 'Volume (up / down)', gamepad: 'D-pad ↑ ↓  or  Right stick ↑ ↓', keys: '↑ ↓' },
    { action: 'Mute', gamepad: '—', keys: 'M' },
    { action: 'Fullscreen', gamepad: '—', keys: 'F' },
    { action: 'Show this help', gamepad: 'Select / Back', keys: '?' },
  ];

  const controls = $derived(isPlayerContext ? playerControls : browseControls);
</script>

{#if isOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="player-overlay"
    role="dialog"
    aria-modal="true"
    aria-label="Controls help"
    tabindex="-1"
    onclick={onclose}
  >
    <div
      class="panel page-panel controls-help-panel"
      onclick={(e) => e.stopPropagation()}
      role="presentation"
    >
      <div class="section-heading section-heading-actions">
        <div>
          <h2>Controls</h2>
          <p class="muted">{isPlayerContext ? 'Player' : 'Browse'} mode</p>
        </div>
        <button type="button" class="player-icon-button" title="Close" aria-label="Close help" onclick={onclose}>
          <Icon name="x" size={20} />
        </button>
      </div>

      <table class="data-table">
        <thead>
          <tr>
            <th>Action</th>
            <th>Gamepad</th>
            <th>Keyboard</th>
          </tr>
        </thead>
        <tbody>
          {#each controls as row (row.action)}
            <tr>
              <td><strong>{row.action}</strong></td>
              <td>{row.gamepad}</td>
              <td class="muted">{row.keys}</td>
            </tr>
          {/each}
        </tbody>
      </table>

      <p class="muted controls-help-note">
        Tip: Use a controller for the best couch/TV experience. Navigation is spatial —
        pressing a direction moves focus to the nearest element in that direction.
      </p>
    </div>
  </div>
{/if}

<style>
  .controls-help-panel {
    max-width: 560px;
    margin: auto;
  }

  .controls-help-note {
    margin-top: 1rem;
    font-size: 0.85rem;
  }
</style>
