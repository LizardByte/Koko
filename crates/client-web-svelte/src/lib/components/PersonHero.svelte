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
  const imageUrl = $derived(
    summary.cached_image_path
      ? getPersonImageUrl(summary.id)
      : summary.image_url
        ? resolveApiUrl(summary.image_url)
        : undefined,
  );
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
  .person-hero {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: 1.5rem;
    align-items: start;
    min-height: min(48vh, 560px);
    padding: 1.3rem 0 0.75rem;
  }
  .person-poster {
    width: min(100%, 240px);
    aspect-ratio: 2 / 3;
    border-radius: 20px;
    box-shadow: 0 24px 44px rgba(0, 0, 0, 0.34);
    display: grid;
    place-items: center;
    overflow: hidden;
    background: linear-gradient(180deg, rgba(93, 123, 255, 0.9), rgba(27, 37, 62, 0.96));
    font-size: 2.2rem;
    font-weight: 800;
    color: rgba(255, 255, 255, 0.85);
  }
  .person-poster img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .item-summary h2,
  .item-title-fallback {
    font-size: 3.2rem;
    line-height: 1.04;
    margin-top: 0;
    margin-bottom: 0.2rem;
    max-width: min(780px, 100%);
    overflow-wrap: anywhere;
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
    margin-top: 0.8rem;
  }
  .button-link {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0.8rem 1rem;
    border-radius: 12px;
    text-decoration: none;
    color: #dbe7ff;
    background: rgba(255, 255, 255, 0.08);
    box-shadow: none;
  }
  .button-link:hover {
    background: rgba(255, 255, 255, 0.16);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.16);
  }
  @media (max-width: 960px) {
    .person-hero {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
