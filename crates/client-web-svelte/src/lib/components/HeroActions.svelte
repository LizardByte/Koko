<script lang="ts">
  // HeroActions — the action-button row of the item-detail hero: Resume,
  // Play now / Start over, Play (target), Restart target, Trailer, Theme, Back.
  // Extracted from SectionHero so the hero stays focused on poster/title/badges.
  // Replaces renderSelectedItemActions() (../client-web/src/app/itemPersonView.ts:
  // 764-909). Playback dispatches into the item store (playbackController spike).
  import Button from './Button.svelte';
  import { goto } from '$app/navigation';
  import {
    backNavigationTarget,
  } from '$lib/selectors';
  import { resumablePlaybackPositionMs } from '$lib/playbackProgress';
  import { libraries, item, playback } from '$lib/stores';
  import { currentTrailerOptions, currentThemeSongTarget, themeSongSourceFromUrl } from '$lib/mediaTargets';
  import type { MediaItemDetail, MediaPlaybackTarget } from '$lib/api';

  type Props = { itemValue: MediaItemDetail };
  let { itemValue }: Props = $props();

  const resumeMs = $derived(resumablePlaybackPositionMs(itemValue));
  const backTarget = $derived(backNavigationTarget(itemValue));
  const library = $derived(libraries.byId(itemValue.library_id));
  const primaryTarget = $derived(itemValue.playable ? undefined : itemValue.playback_target ?? undefined);
  const restartTarget = $derived(
    itemValue.playable ? undefined : itemValue.restart_playback_target ?? undefined,
  );
  const hasTrailer = $derived(Boolean(itemValue.trailer_url) || (itemValue.extras ?? []).some((e) => e.extra_type === 'trailer' && e.url));
  const hasThemeSong = $derived(Boolean(itemValue.theme_song_url));

  function back() {
    if (backTarget.parentId !== undefined) {
      goto(`/items/${backTarget.parentId}`);
    } else if (library) {
      goto(`/libraries/${library.id}`);
    } else {
      goto('/');
    }
  }

  function play(startMs: number) {
    playback.start(itemValue, startMs);
  }
  function playTarget(target: MediaPlaybackTarget) {
    playback.start(itemValue, target.start_ms);
  }
  function playTrailer() {
    const options = currentTrailerOptions(itemValue);
    if (options[0]) {
      playback.openTrailer(options[0]);
    }
  }
  function playThemeSong() {
    const target = currentThemeSongTarget(itemValue);
    if (target) {
      const source = themeSongSourceFromUrl(target.url, target.title);
      playback.playThemeSong(source);
    }
  }

  function formatResumeLabel(ms: number): string {
    const totalSeconds = Math.floor(ms / 1000);
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    if (hours > 0) return `${hours}h ${minutes}m`;
    if (minutes > 0) return `${minutes}m`;
    return `${totalSeconds}s`;
  }
</script>

<div class="detail-actions">
  {#if resumeMs > 0}
    <Button label="Resume {formatResumeLabel(resumeMs)}" icon="play" onclick={() => play(resumeMs)} />
  {/if}
  {#if itemValue.playable}
    <Button
      variant={resumeMs > 0 ? 'secondary' : 'primary'}
      label={resumeMs > 0 ? 'Start over' : 'Play now'}
      icon="play"
      onclick={() => play(0)}
    />
  {/if}
  {#if primaryTarget}
    <Button label="Play" onclick={() => playTarget(primaryTarget)} title={primaryTarget.display_title} />
  {/if}
  {#if restartTarget}
    <Button variant="secondary" label={restartTarget.label} onclick={() => playTarget(restartTarget)} title={restartTarget.display_title} />
  {/if}
  {#if hasTrailer}
    <Button variant="secondary" label="Play Trailer" icon="play" onclick={playTrailer} title={itemValue.trailer_title ?? ''} />
  {/if}
  {#if hasThemeSong}
    <Button variant="secondary" label="Play Theme" icon="volume-2" onclick={playThemeSong} />
  {/if}
  <Button variant="secondary" label={backTarget.label} icon="arrow-left" onclick={back} />
</div>

<p class="muted">{item.playback?.reason ?? 'Loading playback capabilities…'}</p>
