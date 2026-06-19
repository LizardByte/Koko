<script lang="ts">
  // ItemPeople — replaces renderPeopleRail() + renderPersonCredit()
  // (../client-web/src/app/itemPersonView.ts:259-299). A horizontal rail of
  // person cards driven by the first metadata match's people array.
  import { goto } from '$app/navigation';
  import { getPersonImageUrl, resolveApiUrl, type ItemMetadataPerson } from '$lib/api';
  import { selectedItemPeople } from '$lib/selectors';
  import type { ItemMetadataResponse } from '$lib/api';

  type Props = { metadata: ItemMetadataResponse | undefined };
  let { metadata }: Props = $props();

  const people = $derived(selectedItemPeople(metadata));

  function personImage(person: ItemMetadataPerson): string | undefined {
    if (person.cached_image_path) return getPersonImageUrl(person.person_id);
    if (person.image_url) return resolveApiUrl(person.image_url);
    return undefined;
  }
</script>

{#if people.length}
  <section class="panel page-panel item-section item-people-section">
    <div class="section-heading section-heading-actions">
      <div><h3>People</h3></div>
      <span class="muted">{people.length} credit{people.length === 1 ? '' : 's'}</span>
    </div>
    <div class="people-row">
      {#each people as person (person.person_id)}
        <button type="button" class="person-card" onclick={() => goto(`/people/${person.person_id}`)}>
          <span class="person-card-art" class:has-image={Boolean(personImage(person))} style={personImage(person) ? `background-image: url('${personImage(person)}');` : ''}>
            {#if !personImage(person)}<span>{person.name.slice(0, 1).toUpperCase()}</span>{/if}
          </span>
          <span class="person-card-title">{person.name}</span>
          {#if person.character_name || person.role || person.department}
            <span class="person-card-subtitle">{person.character_name ?? person.role ?? person.department}</span>
          {/if}
        </button>
      {/each}
    </div>
  </section>
{/if}

<style>
  /*
   * Component-owned (ItemPeople-only). Values mirror vanilla style.css
   * :1819-1861. The flat .people-row layout is shared (app.css); grouping
   * wrappers (.people-groups / .people-group h4) aren't rendered by this port
   * (people render as a single flat rail), so they're omitted.
   */
  .item-people-section .section-heading {
    margin-bottom: 0.6rem;
  }

  .person-card {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    align-items: stretch;
    width: 142px;
    padding: 0;
    border-radius: 12px;
    background: transparent;
    box-shadow: none;
    text-align: left;
  }

  .person-card-art {
    display: flex;
    align-items: center;
    justify-content: center;
    aspect-ratio: 2 / 3;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.08);
    background-size: cover;
    background-position: center;
    overflow: hidden;
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
