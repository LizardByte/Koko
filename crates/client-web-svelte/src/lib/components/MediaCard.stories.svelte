<script module>
  // MediaCard stories — documents every card variant + the badge subsystem
  // (the class of bugs we fixed: unmatched/watched/pending/has-multiple).
  // Each story picks a fixture preset so the store-seeding decorator
  // (WithStores) loads the libraries the card reads for its kind icon.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import MediaCard from './MediaCard.svelte';
  import {
    movieSummary,
    showSummary,
    seasonSummary,
    episodeSummary,
    trackSummary,
  } from '$lib/storybook/fixtures';

  const { Story } = defineMeta({
    title: 'Components/MediaCard',
    component: MediaCard,
    tags: ['autodocs'],
    args: { preset: 'home', route: '/' },
    parameters: {
      layout: 'centered',
      docs: {
        description: {
          component:
            'Browseable item card. Renders the poster art (mock gradient fallback in mock mode), a kind/duration pill row, and dynamic badges (unmatched metadata, watched, in-progress ring, missing). The most variant-rich component — this story file pins every state.',
        },
      },
    },
  });
</script>

<Story name="Movie" args={{ preset: 'home', route: '/' }} asChild>
  <MediaCard item={movieSummary()} />
</Story>

<Story name="Show" args={{ preset: 'home', route: '/' }} asChild>
  <MediaCard item={showSummary()} />
</Story>

<Story name="Season" args={{ preset: 'home', route: '/' }} asChild>
  <MediaCard item={seasonSummary()} />
</Story>

<Story name="Episode" args={{ preset: 'home', route: '/' }} asChild>
  <MediaCard item={episodeSummary()} />
</Story>

<Story name="Track" args={{ preset: 'home', route: '/' }} asChild>
  <MediaCard item={trackSummary()} />
</Story>

<!--
  Unmatched: has_metadata absent → warning triangle badge. Regression cage:
  if the isUnmatched predicate breaks (=== false instead of !== true), this
  card silently loses its badge.
-->
<Story name="Unmatched" args={{ preset: 'home', route: '/' }} asChild>
  <MediaCard item={trackSummary()} />
</Story>

<!-- Watched: watch_count > 0 → checkmark badge, title "Watched Nx" -->
<Story name="Watched" args={{ preset: 'home', route: '/' }} asChild>
  <MediaCard item={movieSummary({ watch_count: 3, last_watched_at: 1_760_900_000 })} />
</Story>

<!-- In progress: playback_position_ms set → progress ring -->
<Story name="In Progress" args={{ preset: 'home', route: '/' }} asChild>
  <MediaCard
    item={movieSummary({
      playback_position_ms: 1_260_000,
      playback_duration_ms: 5_400_000,
      playback_completed: false,
    })}
  />
</Story>

<!-- Missing from disk → amber border + missing badge -->
<Story name="Missing" args={{ preset: 'home', route: '/' }} asChild>
  <MediaCard item={movieSummary({ missing_since: 1_760_000_000 })} />
</Story>

<!-- Metadata refresh pending → spinner badge -->
<Story name="Metadata Pending" args={{ preset: 'home', route: '/' }} asChild>
  <MediaCard item={movieSummary({ metadata_refresh_state: 'pending' })} />
</Story>
