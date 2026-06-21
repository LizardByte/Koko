<script lang="ts">
  // PersonCard — a single cast/person thumbnail card. Extracted from
  // SectionPeople so the card is independently testable + shares the
  // CardSurface shell with MediaCard/MediaExtraCard.
  // Replaces the inner card of renderPeopleRail()
  // (../client-web/src/app/itemPersonView.ts:259-299).
  import CardSurface from './CardSurface.svelte';
  import { getPersonImageUrl, resolveApiUrl, type ItemMetadataPerson } from '$lib/api';

  type Props = { person: ItemMetadataPerson; onnavigate?: (personId: number) => void };
  let { person, onnavigate }: Props = $props();

  // Image resolution mirrors SectionPeople: prefer the server-cached path,
  // fall back to the provider image_url, else undefined (renders initials).
  const image = $derived(
    person.cached_image_path
      ? getPersonImageUrl(person.person_id)
      : person.image_url
        ? resolveApiUrl(person.image_url)
        : undefined,
  );
</script>

<CardSurface
  tileRadius={8}
  aspectRatio="2 / 3"
  class="person-card"
  label={person.name}
  onclick={() => onnavigate?.(person.person_id)}
>
  {#snippet art()}
    <span
      class="person-card-art"
      class:has-image={Boolean(image)}
      style={image ? `background-image: url('${image}');` : ''}
    >
      {#if !image}<span>{person.name.slice(0, 1).toUpperCase()}</span>{/if}
    </span>
  {/snippet}

  {#snippet body()}
    <span class="person-card-title">{person.name}</span>
    {#if person.character_name || person.role || person.department}
      <span class="person-card-subtitle">{person.character_name ?? person.role ?? person.department}</span>
    {/if}
  {/snippet}
</CardSurface>

<style>
  /* Card-specific: width + gap to match vanilla .person-card (style.css:1819).
     Applied to the CardSurface root via the class prop. Using :global because
     CardSurface renders the root button (the class is on a child component's
     element, not in this component's template). */
  :global(.person-card.card-surface) {
    width: 142px;
    gap: 0.5rem;
  }

  /* Flat background + initials placeholder. overflow:hidden comes from the
     shell tile; here we set the fill + typography. */
  .person-card-art {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.08);
    color: #dfe9ff;
    font-size: 2.2rem;
    font-weight: 700;
  }

  .person-card-title,
  .person-card-subtitle {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .person-card-title {
    font-weight: 700;
  }

  .person-card-subtitle {
    color: var(--muted);
    font-size: 0.8rem;
  }
</style>
