<script lang="ts">
  // ItemHero — replaces renderSelectedItemHero() + renderSelectedItemActions()
  // + renderSelectedItemFactList() (../client-web/src/app/itemPersonView.ts:
  // 764-909). The poster, title (logo or fallback), tagline, meta row in fixed
  // badge order (missing, playback, year, content rating, rating, genres),
  // collapsible overview, action buttons, and the technical fact list.
  import Button from './Button.svelte';
  import CollapsibleText from './CollapsibleText.svelte';
  import Icon from './Icon.svelte';
  import { getArtworkUrl, resolveApiUrl, type MediaItemDetail, type MediaPlaybackTarget } from '$lib/api';
  import {
    backNavigationTarget,
    selectedItemTechnicalFacts,
  } from '$lib/selectors';
  import { playbackProgressPercent, resumablePlaybackPositionMs } from '$lib/playbackProgress';
  import { formatTimestamp } from '$lib/format';
  import { libraries } from '$lib/stores';
  import { item, ui } from '$lib/stores';
  import { goto } from '$app/navigation';

  type Props = { itemValue: MediaItemDetail };
  let { itemValue }: Props = $props();

  const posterUrl = $derived(
    itemValue.poster_url
      ? getArtworkUrl(itemValue.id, 'poster', itemValue.artwork_updated_at)
      : undefined,
  );
  const logoUrl = $derived(itemValue.logo_url ? resolveApiUrl(itemValue.logo_url) : undefined);
  const library = $derived(libraries.byId(itemValue.library_id));
  const resumeMs = $derived(resumablePlaybackPositionMs(itemValue));
  const backTarget = $derived(backNavigationTarget(itemValue));
  const facts = $derived(selectedItemTechnicalFacts(itemValue));
  const isMissing = $derived(Boolean(itemValue.missing_since));
  const genres = $derived(itemValue.genres);

  // Detail-badge state — mirrors playbackDetailBadgeMarkup / missingItemDetail
  // BadgeMarkup (homeView.ts:330-382): watch count + label, progress %, and
  // the missing-since timestamp for the warning tag.
  const watchCount = $derived(itemValue.watch_count ?? 0);
  const watchedLabel = $derived(watchCount === 1 ? 'Watched' : `Watched ${watchCount}x`);
  const watchedTitle = $derived(
    itemValue.last_watched_at
      ? `Last watched ${formatTimestamp(itemValue.last_watched_at)}`
      : watchedLabel,
  );
  // MediaItemDetail allows null on the playback fields where MediaItemSummary
  // uses undefined; coerce to the summary shape playbackProgressPercent wants.
  const summaryForProgress = $derived({
    ...itemValue,
    playback_position_ms: itemValue.playback_position_ms ?? undefined,
    playback_duration_ms: itemValue.playback_duration_ms ?? undefined,
    duration_ms: itemValue.duration_ms ?? undefined,
  });
  const progressPercent = $derived(playbackProgressPercent(summaryForProgress));
  const missingTitle = $derived(`Missing from disk since ${formatTimestamp(itemValue.missing_since ?? undefined)}`);

  const primaryTarget = $derived(itemValue.playable ? undefined : itemValue.playback_target ?? undefined);
  const restartTarget = $derived(
    itemValue.playable ? undefined : itemValue.restart_playback_target ?? undefined,
  );
  const hasTrailer = $derived(Boolean(itemValue.trailer_url));
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

  // Playback actions dispatch into the item store — the actual player lands in
  // the playbackController spike. For now they surface a message.
  function play(_startMs: number) {
    ui.setError(`Playback of "${itemValue.display_title}" is not yet implemented (playbackController spike).`);
  }
  function playTarget(target: MediaPlaybackTarget) {
    ui.setError(`Playback target "${target.label}" is not yet implemented (playbackController spike).`);
  }
  function playTrailer() {
    ui.setError(`Trailer playback is not yet implemented (playbackController spike).`);
  }
  function playThemeSong() {
    ui.setError(`Theme song playback is not yet implemented (playbackController spike).`);
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

<section class="item-hero" class:episode-hero={itemValue.item_type === 'episode'}>
  <div class="detail-art item-poster" class:item-thumbnail={itemValue.item_type === 'episode'} class:has-image={Boolean(posterUrl)}>
    {#if posterUrl}
      <img src={posterUrl} alt="{itemValue.display_title} poster" />
    {:else}
      <span>{itemValue.display_title.slice(0, 1).toUpperCase()}</span>
    {/if}
  </div>
  <div class="detail-summary item-summary">
    {#if logoUrl}
      <img class="item-title-logo" src={logoUrl} alt={itemValue.display_title} />
    {:else}
      <h2 class="item-title-fallback">{itemValue.display_title}</h2>
    {/if}

    {#if itemValue.tagline}
      <p class="hero-tagline">{itemValue.tagline}</p>
    {/if}

    <div class="hero-meta-row">
      {#if isMissing}
        <span class="tag warning status-tag" title={missingTitle} aria-label={missingTitle}>
          <span class="status-icon"><Icon name="triangle-alert" size={15} strokeWidth={2.2} /></span>
          <span>Missing</span>
        </span>
      {/if}
      {#if watchCount > 0}
        <span class="tag success status-tag" title={watchedTitle}>
          <span class="status-icon"><Icon name="circle-check" size={15} strokeWidth={2.2} /></span>
          <span>{watchedLabel}</span>
        </span>
      {/if}
      {#if progressPercent !== undefined}
        <span class="tag status-tag">{progressPercent}% watched</span>
      {/if}
      {#if itemValue.release_year}<span class="tag">{itemValue.release_year}</span>{/if}
      {#if itemValue.content_rating}<span class="tag">{itemValue.content_rating}</span>{/if}
      {#if typeof itemValue.rating === 'number'}<span class="tag">{itemValue.rating.toFixed(1)}</span>{/if}
      {#each genres as genre (genre)}<span class="tag">{genre}</span>{/each}
    </div>

    <CollapsibleText text={itemValue.overview ?? 'No description is stored for this item yet.'} storageKey="item-overview:{itemValue.id}" className="hero-description" />

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

    <div class="item-fact-list">
      {#each facts as fact (fact.label)}
        <div class="item-fact">
          <span class="label">{fact.label}</span>
          <strong>{fact.value}</strong>
        </div>
      {/each}
    </div>
  </div>
</section>

<style>
  /*
   * Component-owned (ItemHero-only). Shared .item-hero / .item-poster /
   * .item-summary / .item-title-fallback / .detail-art / .detail-summary /
   * .hero-meta-row / .hero-tagline / .detail-actions live in app.css (used by
   * PersonHero too). Values mirror vanilla style.css:1537-1592.
   */
  .item-hero.episode-hero {
    grid-template-columns: minmax(280px, 360px) minmax(0, 1fr);
  }

  .item-thumbnail {
    width: min(100%, 360px);
    aspect-ratio: 16 / 9;
  }

  .item-title-logo {
    display: block;
    max-width: min(340px, 100%);
    max-height: 120px;
    object-fit: contain;
    object-position: left center;
  }

  .item-fact-list {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: 0.8rem;
    margin-top: 0.5rem;
  }

  .item-fact {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    padding: 0.75rem 0.9rem;
    border-radius: 18px;
    background: rgba(8, 11, 18, 0.28);
    border: 1px solid rgba(255, 255, 255, 0.08);
    backdrop-filter: blur(16px);
  }

  @media (max-width: 960px) {
    .item-hero {
      grid-template-columns: minmax(0, 1fr);
    }
    .item-poster {
      width: min(220px, 100%);
    }
  }
</style>
