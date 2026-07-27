<script lang="ts">
  // SectionPeople — replaces renderPeopleRail()
  // (../client-web/src/app/itemPersonView.ts:259-299). A horizontal rail of
  // person cards (each a PersonCard sharing the CardSurface shell) driven by
  // the first metadata match's people array.
  import { goto } from '$app/navigation';
  import PersonCard from './PersonCard.svelte';
  import { selectedItemPeople } from '$lib/selectors';
  import type { ItemMetadataResponse } from '$lib/api';

  type Props = { metadata: ItemMetadataResponse | undefined };
  let { metadata }: Props = $props();

  const people = $derived(selectedItemPeople(metadata));

  // Injected navigation so PersonCard stays decoupled from $app/navigation.
  function navigate(personId: number) {
    goto(`/people/${personId}`);
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
        <PersonCard {person} onnavigate={navigate} />
      {/each}
    </div>
  </section>
{/if}

<style>
  .item-people-section .section-heading {
    margin-bottom: 0.6rem;
  }
</style>
