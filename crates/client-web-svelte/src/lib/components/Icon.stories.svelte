<script module lang="ts">
  // Icon gallery — renders every icon name in the ICONS map. Documents the
  // available set and catches missing-icon regressions (a name used in a
  // component but absent from the map renders nothing — visible here).
  //
  // `component: IconPreview` so the "Customize" story can spread color/alpha/
  // size args onto it. Gallery and Single render via asChild snippets (with
  // Icon directly), so the args they pass are decorator-only (preset) and not
  // spread onto IconPreview — hence they omit props that IconPreview doesn't
  // take. The decorator defaults preset to 'empty' regardless.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import Icon, { ICONS } from './Icon.svelte';
  import IconPreview from './IconPreview.svelte';

  const { Story } = defineMeta({
    title: 'Components/Icon',
    component: IconPreview,
    tags: ['autodocs'],
    args: { preset: 'empty' },
    argTypes: {
      color: { control: 'color', description: 'Icon color (via currentColor on the wrapper)' },
      alpha: { control: { type: 'range', min: 0, max: 1, step: 0.05 }, description: 'Icon opacity' },
      size: { control: { type: 'number', min: 8, max: 96, step: 2 }, description: 'Icon size in px' },
    },
    parameters: {
      docs: {
        description: {
          component:
            'Thin wrapper over @lucide/svelte giving a string-name API matching the vanilla client’s `<i data-lucide>`. Any name a component uses must exist in the ICONS map or it renders nothing. This gallery is the canonical set — add new names to Icon.svelte and they appear here automatically. Use the "Customize" story to preview color and opacity interactively.',
        },
      },
    },
  });

  const names = Object.keys(ICONS).sort();
</script>

<Story name="Gallery" asChild>
  <div class="icon-grid">
    {#each names as name (name)}
      <figure>
        <Icon name={name} size={28} />
        <figcaption>{name}</figcaption>
      </figure>
    {/each}
  </div>
</Story>

<!--
  Customize: interactive color + alpha + size preview. Lucide strokes use
  currentColor, so setting `color`/`opacity` on the IconPreview wrapper
  recolors every glyph. No `asChild` so args spread onto IconPreview as props.
-->
<Story
  name="Customize"
  args={{ preset: 'empty', color: '#8dd35f', alpha: 1, size: 40 }}
/>

<Story name="Single" asChild>
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
