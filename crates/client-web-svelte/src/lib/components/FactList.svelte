<script lang="ts">
  // FactList — the technical-facts grid at the bottom of the item-detail hero
  // (codec, resolution, duration, etc.). Extracted from SectionHero so the hero
  // stays focused on poster/title/badges.
  // Replaces renderSelectedItemFactList() (../client-web/src/app/itemPersonView.ts).
  import { selectedItemTechnicalFacts } from '$lib/selectors';
  import type { MediaItemDetail } from '$lib/api';

  type Props = { itemValue: MediaItemDetail };
  let { itemValue }: Props = $props();

  const facts = $derived(selectedItemTechnicalFacts(itemValue));
</script>

<div class="item-fact-list">
  {#each facts as fact (fact.label)}
    <div class="item-fact">
      <span class="label">{fact.label}</span>
      <strong>{fact.value}</strong>
    </div>
  {/each}
</div>

<style>
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
</style>
