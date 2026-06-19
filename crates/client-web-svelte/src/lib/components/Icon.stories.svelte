<script module>
  // Icon gallery — renders every icon name in the ICONS map. Documents the
  // available set and catches missing-icon regressions (a name used in a
  // component but absent from the map renders nothing — visible here).
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import Icon, { ICONS } from './Icon.svelte';

  const { Story } = defineMeta({
    title: 'Components/Icon',
    component: Icon,
    tags: ['autodocs'],
    args: { preset: 'empty' },
    parameters: {
      docs: {
        description: {
          component:
            'Thin wrapper over @lucide/svelte giving a string-name API matching the vanilla client’s `<i data-lucide>`. Any name a component uses must exist in the ICONS map or it renders nothing. This gallery is the canonical set — add new names to Icon.svelte and they appear here automatically.',
        },
      },
    },
  });

  const names = Object.keys(ICONS).sort();
</script>

<Story name="Gallery" args={{ preset: 'empty' }}>
  <div class="icon-grid">
    {#each names as name (name)}
      <figure>
        <Icon name={name} size={28} />
        <figcaption>{name}</figcaption>
      </figure>
    {/each}
  </div>
</Story>

<Story name="Single" args={{ preset: 'empty' }}>
  <div class="single-frame">
    <Icon name="house" size={48} />
  </div>
</Story>

<style>
  .icon-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
    gap: 1rem;
    padding: 1rem;
    color: #f4f7fb;
  }
  figure {
    margin: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.5rem;
    padding: 1rem 0.5rem;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.04);
  }
  figcaption {
    font-size: 0.78rem;
    color: #9ab1d1;
    font-family: monospace;
  }
  .single-frame {
    padding: 2rem;
    color: #f4f7fb;
  }
</style>
