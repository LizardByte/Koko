<script lang="ts">
  // Item detail page — replaces renderItemPage() (../client-web/src/app/
  // itemPersonView.ts). Demonstrates SvelteKit's $page params (replacing the
  // vanilla client's regex route parsing in routes.ts), data load keyed on the
  // id param, and a hero/backdrop layout with play CTA. Playback itself is out
  // of scope (see PROPOSAL.md §4 — playbackController.ts is the hard seam).
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import { getItem, type MediaItemDetail } from '$lib/api';
  import { formatDuration, formatFileSize, formatRating } from '$lib/format';
  import Icon from '$lib/components/Icon.svelte';
  import Tag from '$lib/components/Tag.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import MediaCard from '$lib/components/MediaCard.svelte';

  let item = $state<MediaItemDetail | undefined>(undefined);
  let loading = $state(true);
  let error = $state<string | undefined>(undefined);

  const itemId = $derived(Number(page.params.id));

  async function load() {
    loading = true;
    error = undefined;
    try {
      item = await getItem(itemId);
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  // Reload when navigating between items without unmounting.
  $effect(() => {
    if (Number.isFinite(itemId)) {
      load();
    }
  });

  const contentRatingTone = $derived(
    item?.content_rating === 'R' || item?.content_rating === 'NC-17'
      ? 'danger'
      : item?.content_rating?.startsWith('TV-MA')
        ? 'danger'
        : 'info',
  );
</script>

<svelte:head><title>{item ? `${item.display_title} — Koko` : 'Koko'}</title></svelte:head>

{#if loading && !item}
  <Spinner />
{:else if error}
  <div class="empty-state">Failed to load item: {error}</div>
{:else if item}
  <article class="item-page">
    <div class="hero" style={item.backdrop_url ? `background-image: url('${item.backdrop_url}')` : ''}>
      <div class="hero-overlay"></div>
      <div class="hero-content">
        {#if item.tagline}
          <p class="tagline">{item.tagline}</p>
        {/if}
        <h1 class="title">{item.display_title}</h1>
        <div class="meta-row">
          {#if item.release_year}<span>{item.release_year}</span>{/if}
          {#if item.duration_ms}<span>· {formatDuration(item.duration_ms)}</span>{/if}
          {#if item.content_rating}<Tag variant={contentRatingTone}>{item.content_rating}</Tag>{/if}
          {#if item.rating}
            <span class="rating"><Icon name="star" size={14} /> {formatRating(item.rating)}</span>
          {/if}
        </div>
        {#if item.overview}
          <p class="overview">{item.overview}</p>
        {/if}
        {#if item.genres.length}
          <div class="genres">
            {#each item.genres as genre (genre)}
              <span class="genre-chip">{genre}</span>
            {/each}
          </div>
        {/if}
        {#if item.playable}
          <button class="play-cta" type="button">
            <Icon name="play" size={20} /> Play
          </button>
        {/if}
      </div>
    </div>

    <div class="details-grid">
      <section class="panel">
        <h3>Details</h3>
        <dl>
          {#if item.file_size}
            <div><dt>File size</dt><dd>{formatFileSize(item.file_size)}</dd></div>
          {/if}
          {#if item.container}<div><dt>Container</dt><dd>{item.container}</dd></div>{/if}
          {#if item.video_codec}<div><dt>Video</dt><dd>{item.video_codec}</dd></div>{/if}
          {#if item.audio_codec}<div><dt>Audio</dt><dd>{item.audio_codec}</dd></div>{/if}
          {#if item.audio_tracks.length}
            <div><dt>Audio tracks</dt><dd>{item.audio_tracks.length}</dd></div>
          {/if}
          <div><dt>Type</dt><dd>{item.item_type}</dd></div>
          <div>
            <dt>Metadata</dt>
            <dd>{item.has_metadata ? 'Linked' : 'Unlinked'}</dd>
          </div>
        </dl>
      </section>

      {#if item.hierarchy.length > 0}
        <section class="panel">
          <h3>Location</h3>
          <nav class="breadcrumb">
            {#each item.hierarchy as ancestor (ancestor.id)}
              <a href="/items/{ancestor.id}">{ancestor.display_title}</a>
              <span class="sep">/</span>
            {/each}
            <span class="current">{item.display_title}</span>
          </nav>
        </section>
      {/if}
    </div>

    {#if item.children.length > 0}
      <section class="children">
        <h2>{item.item_type === 'show' ? 'Seasons' : 'Items'}</h2>
        <div class="children-grid">
          {#each item.children as child (child.id)}
            <MediaCard item={child} />
          {/each}
        </div>
      </section>
    {/if}
  </article>
{/if}

<style>
  .hero {
    position: relative;
    margin: -1.5rem -1.5rem 1.5rem;
    padding: 4rem 1.5rem 2rem;
    background-size: cover;
    background-position: center;
    background-color: #1f2937;
  }
  .hero-overlay {
    position: absolute;
    inset: 0;
    background: linear-gradient(to top, var(--koko-surface, #000) 5%, rgba(31, 41, 55, 0.6) 50%, rgba(31, 41, 55, 0.3) 100%);
  }
  .hero-content {
    position: relative;
    max-width: 720px;
  }
  .tagline {
    font-style: italic;
    color: rgba(255, 255, 255, 0.85);
    margin: 0 0 0.4rem;
  }
  .title {
    font-size: 2rem;
    font-weight: 700;
    margin: 0 0 0.6rem;
    color: #fff;
  }
  .meta-row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
    font-size: 0.9rem;
    color: rgba(255, 255, 255, 0.9);
    margin-bottom: 1rem;
  }
  .rating {
    display: inline-flex;
    align-items: center;
    gap: 0.2rem;
  }
  .overview {
    color: rgba(255, 255, 255, 0.92);
    line-height: 1.5;
    margin: 0 0 1rem;
  }
  .genres {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
    margin-bottom: 1.2rem;
  }
  .genre-chip {
    font-size: 0.78rem;
    padding: 0.15rem 0.55rem;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.15);
    color: #fff;
  }
  .play-cta {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.6rem 1.4rem;
    border-radius: 6px;
    border: none;
    background: #fff;
    color: #111;
    font-size: 1rem;
    font-weight: 600;
    cursor: pointer;
  }
  .play-cta:hover {
    background: #e5e7eb;
  }
  .details-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 1rem;
    margin-bottom: 1.5rem;
  }
  .panel {
    background: var(--koko-surface, #fff);
    border: 1px solid var(--koko-border, #ddd);
    border-radius: 8px;
    padding: 1rem 1.25rem;
  }
  .panel h3 {
    margin: 0 0 0.6rem;
    font-size: 1rem;
  }
  dl {
    margin: 0;
    display: grid;
    gap: 0.35rem;
  }
  dl div {
    display: grid;
    grid-template-columns: 110px 1fr;
    font-size: 0.85rem;
  }
  dt {
    color: var(--koko-muted, #777);
  }
  dd {
    margin: 0;
  }
  .breadcrumb {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.85rem;
  }
  .breadcrumb a {
    color: #2563eb;
    text-decoration: none;
  }
  .breadcrumb a:hover {
    text-decoration: underline;
  }
  .sep {
    color: var(--koko-muted, #777);
  }
  .current {
    font-weight: 500;
  }
  .children h2 {
    font-size: 1.1rem;
    margin: 0 0 0.8rem;
  }
  .children-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 0.9rem;
  }
  .empty-state {
    color: var(--koko-muted, #777);
    padding: 2rem;
    text-align: center;
  }
</style>
