<script module>
  // Shelf stories. Props-driven (title + items); the MediaCard children read
  // the libraries store, so preset 'home' seeds it.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import Shelf from './Shelf.svelte';
  import { movieSummary } from '$lib/storybook/fixtures';

  const { Story } = defineMeta({
    title: 'Components/Shelf',
    component: Shelf,
    tags: ['autodocs'],
    args: { preset: 'home' },
    parameters: {
      docs: {
        description: {
          component:
            'Horizontal card rail with title + count header, scroll buttons (hidden when content fits), and lazy chunked rendering.',
        },
      },
    },
  });

  const few = [movieSummary()];
  const many = Array.from({ length: 14 }, (_, i) => movieSummary({ id: 101 + i, display_title: `Mock Movie ${i + 1}` }));
</script>

<Story name="Many Items" args={{ preset: 'home', title: 'Recently added', id: 'recently_added', rowCountId: 'recently_added' }} asChild>
  <Shelf title="Recently added" items={many} id="recently_added" rowCountId="recently_added" />
</Story>

<Story name="Few Items" args={{ preset: 'home', title: 'Continue watching', id: 'continue_watching', rowCountId: 'continue_watching' }} asChild>
  <Shelf title="Continue watching" items={few} id="continue_watching" rowCountId="continue_watching" />
</Story>

<Story name="Empty" args={{ preset: 'home', title: 'Recommended', id: 'recommended', rowCountId: 'recommended' }} asChild>
  <Shelf title="Recommended" items={[]} id="recommended" rowCountId="recommended" />
</Story>
