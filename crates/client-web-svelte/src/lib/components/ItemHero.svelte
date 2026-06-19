<script lang="ts">
  // ItemHero — replaces renderSelectedItemHero() + renderSelectedItemActions()
  // + renderSelectedItemFactList() (../client-web/src/app/itemPersonView.ts:
  // 764-909). The poster, title (logo or fallback), tagline, meta row in fixed
  // badge order (missing, playback, year, content rating, rating, genres),
  // collapsible overview, action buttons, and the technical fact list.
  import Button from './Button.svelte';
  import CollapsibleText from './CollapsibleText.svelte';
  import { getArtworkUrl, resolveApiUrl, type MediaItemDetail, type MediaPlaybackTarget } from '$lib/api';
  import {
    backNavigationTarget,
    selectedItemTechnicalFacts,
  } from '$lib/selectors';
  import { resumablePlaybackPositionMs } from '$lib/playbackProgress';
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
        <span class="tag warning status-tag">Missing from disk</span>
      {/if}
      {#if itemValue.playback_completed}
        <span class="tag success status-tag">Watched</span>
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
  .item-hero {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: 1.5rem;
    align-items: start;
    min-height: min(58vh, 720px);
    padding: 1.3rem 0 0.75rem;
  }

  .item-hero.episode-hero {
    grid-template-columns: minmax(280px, 360px) minmax(0, 1fr);
  }

  .item-poster {
    width: min(100%, 220px);
    box-shadow: 0 24px 44px rgba(0, 0, 0, 0.34);
  }

  .item-thumbnail {
    width: min(100%, 360px);
    aspect-ratio: 16 / 9;
  }

  .detail-art {
    aspect-ratio: 2 / 3;
    border-radius: 20px;
    display: grid;
    place-items: center;
    overflow: hidden;
    background: linear-gradient(180deg, rgba(93, 123, 255, 0.9), rgba(27, 37, 62, 0.96));
    font-size: 2.2rem;
    font-weight: 800;
    color: rgba(255, 255, 255, 0.85);
  }

  .detail-art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .item-summary {
    align-self: start;
    padding: 0.35rem 0 1rem;
  }

  .item-summary h2,
  .item-title-fallback {
    font-size: 3.2rem;
    line-height: 1.04;
    margin-top: 0;
    margin-bottom: 0.2rem;
  }

  .item-title-fallback {
    max-width: min(780px, 100%);
    overflow-wrap: anywhere;
  }

  .item-title-logo {
    display: block;
    max-width: min(340px, 100%);
    max-height: 120px;
    object-fit: contain;
    object-position: left center;
  }

  .hero-tagline {
    margin: 0;
    font-size: 1.05rem;
    color: #d6e5ff;
  }

  .hero-meta-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.55rem;
    margin: 0.5rem 0;
  }

  .detail-actions {
    display: flex;
    gap: 0.7rem;
    flex-wrap: wrap;
    margin: 0.8rem 0;
  }

  .item-fact-list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 0.6rem;
    margin-top: 0.5rem;
  }

  .item-fact {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    padding: 0.6rem 0.8rem;
    border-radius: 18px;
    background: rgba(255, 255, 255, 0.04);
    backdrop-filter: blur(16px);
    font-size: 0.85rem;
  }

  .item-fact strong {
    color: #f4f7fb;
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
