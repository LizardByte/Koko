<script module lang="ts">
  // SectionChildren stories — the seasons/episodes/contained-items grid for
  // show/season/container item types. Reads item.children (populated in the
  // fixture below). Empty when the item has no children.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import SectionChildren from './SectionChildren.svelte';
  import { movieDetail, showSummary, seasonSummary, episodeSummary } from '$lib/storybook/fixtures';

  const { Story } = defineMeta({
    title: 'Components/SectionChildren',
    tags: ['autodocs'],
    args: { preset: 'item-movie' },
    parameters: {
      docs: {
        description: {
          component:
            'Seasons/episodes/contained-items grid. Title adapts: "Seasons" for shows, "Episodes" for seasons, "Contained items" otherwise. Empty when the item has no children (movies, unmatched).',
        },
      },
    },
  });

  // A show with 2 seasons
  const show = movieDetail({
    ...showSummary(),
    item_type: 'show',
    children: [
      { ...seasonSummary(), display_title: 'Season 1', child_count: 3 },
      { ...seasonSummary({ id: 212, display_title: 'Season 2', season_number: 2 }), child_count: 2 },
    ],
  });

  // A season with 3 episodes
  const season = movieDetail({
    ...seasonSummary(),
    item_type: 'season',
    children: [
      episodeSummary(),
      { ...episodeSummary({ id: 213, display_title: 'Episode 2' }) },
      { ...episodeSummary({ id: 214, display_title: 'Episode 3' }) },
    ],
  });

  // A movie (no children → renders nothing)
  const movie = movieDetail();
</script>

<Story name="Show (Seasons)" args={{ preset: 'item-movie' }} asChild>
  <SectionChildren item={show} />
</Story>

<Story name="Season (Episodes)" args={{ preset: 'item-movie' }} asChild>
  <SectionChildren item={season} />
</Story>

<Story name="Movie (Empty)" args={{ preset: 'item-movie' }} asChild>
  <SectionChildren item={movie} />
</Story>
