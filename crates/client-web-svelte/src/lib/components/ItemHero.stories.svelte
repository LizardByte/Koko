<script module>
  // ItemHero stories. Reads the `item` store + libraries; each story picks a
  // preset that seeds the item store with a different variant. We forward the
  // store item to the component's `itemValue` prop to mirror the route wiring
  // (items/[id]/+page.svelte passes item.item to ItemHero).
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import ItemHero from './ItemHero.svelte';
  import { item as itemStore } from '$lib/stores';

  const { Story } = defineMeta({
    title: 'Fragments/ItemHero',
    component: ItemHero,
    tags: ['autodocs'],
    args: { preset: 'item-movie' },
    parameters: {
      docs: {
        description: {
          component:
            'Item detail hero: poster, title/logo, tagline, meta-row badges (missing/watched/progress/year/rating/genres), collapsible overview, action buttons (playback stubs), and the technical fact list.',
        },
      },
    },
  });
</script>

<Story name="Movie" args={{ preset: 'item-movie' }} asChild>
  {#if itemStore.item}<ItemHero itemValue={itemStore.item} />{/if}
</Story>

<Story name="Show" args={{ preset: 'item-show' }} asChild>
  {#if itemStore.item}<ItemHero itemValue={itemStore.item} />{/if}
</Story>

<Story name="Missing" args={{ preset: 'item-missing' }} asChild>
  {#if itemStore.item}<ItemHero itemValue={itemStore.item} />{/if}
</Story>

<Story name="Watched" args={{ preset: 'item-watched' }} asChild>
  {#if itemStore.item}<ItemHero itemValue={itemStore.item} />{/if}
</Story>
