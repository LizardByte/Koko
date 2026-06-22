<script lang="ts">
  // PersonHero — replaces renderPersonPage()'s hero (../client-web/src/app/
  // itemPersonView.ts:526-581). Portrait, name, provider tag, credits count,
  // birthday + age, gender, birthplace, collapsible biography, known-for tags,
  // Back + provider-page buttons.
  import Button from './Button.svelte';
  import CollapsibleText from './CollapsibleText.svelte';
  import { getPersonImageUrl, resolveApiUrl, type MetadataPersonResponse } from '$lib/api';
  import { formatPersonDate, personAgeLabel } from '$lib';

  type Props = {
    person: MetadataPersonResponse;
    onBack: () => void;
  };
  let { person, onBack }: Props = $props();

  const summary = $derived(person.person);
  function resolveImageUrl(): string | undefined {
    if (summary.cached_image_path) return getPersonImageUrl(summary.id);
    if (summary.image_url) return resolveApiUrl(summary.image_url);
    return undefined;
  }
  const imageUrl = $derived(resolveImageUrl());
  const ageLabel = $derived(personAgeLabel(summary.birthday, summary.deathday));
  const knownFor = $derived(summary.known_for);
</script>

<section class="item-hero person-hero">
  <div class="detail-art item-poster person-poster" class:has-image={Boolean(imageUrl)}>
    {#if imageUrl}
      <img src={imageUrl} alt={summary.name} />
    {:else}
      <span>{summary.name.slice(0, 1).toUpperCase()}</span>
    {/if}
  </div>
  <div class="detail-summary item-summary">
    <h2 class="item-title-fallback">{summary.name}</h2>
    <div class="hero-meta-row">
      <span class="tag">{summary.provider_id}</span>
      <span class="tag">{person.credits.length} item{person.credits.length === 1 ? '' : 's'}</span>
      {#if summary.birthday}
        <span class="tag">{formatPersonDate(summary.birthday)}{#if ageLabel} · {ageLabel}{/if}</span>
      {/if}
      {#if summary.gender}<span class="tag">{summary.gender}</span>{/if}
    </div>
    {#if summary.birth_place}
      <p class="hero-tagline">{summary.birth_place}</p>
    {/if}
    {#if summary.biography}
      <CollapsibleText text={summary.biography} storageKey="person-biography:{summary.id}" className="hero-description" />
    {/if}
    {#if knownFor.length}
      <div class="hero-meta-row">
        {#each knownFor as title (title)}<span class="tag">{title}</span>{/each}
      </div>
    {/if}
    <div class="detail-actions">
      <Button variant="secondary" label="Back" icon="arrow-left" onclick={onBack} />
      {#if summary.profile_url}
        <a class="button-link secondary-button" href={summary.profile_url} target="_blank" rel="noreferrer">Provider page</a>
      {/if}
    </div>
  </div>
</section>

<style>
  /*
   * Component-owned (PersonHero-only). The shared .item-hero / .detail-art /
   * .item-poster / .item-summary / .item-title-fallback / .hero-tagline /
   * .hero-meta-row / .detail-actions rules live in app.css (used by SectionHero
   * too). .person-poster / .button-link / .person-hero are PersonHero-only.
   * Values mirror vanilla style.css:1541-1574, 1710-1718, 1863-1865.
   */
  .person-poster {
    width: min(100%, 240px);
  }

  .person-poster img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .button-link {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2.4rem;
    padding: 0.65rem 1rem;
    border-radius: 999px;
    text-decoration: none;
  }

  @media (max-width: 960px) {
    .person-hero {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
