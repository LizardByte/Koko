<script module>
  // Button stories — fully props-driven (no store reads), so no preset needed.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import Button from './Button.svelte';

  const { Story } = defineMeta({
    title: 'Components/Button',
    component: Button,
    tags: ['autodocs'],
    args: { preset: 'empty' },
    parameters: {
      layout: 'centered',
      docs: {
        description: {
          component:
            'Action button. Reproduces the vanilla client’s `.button-content` / `.button-icon` markup. Variants: primary (brand gradient), secondary (translucent), danger. Optional icon at start or end; busy state shows a spinner.',
        },
      },
    },
  });
</script>

<Story name="Primary" args={{ preset: 'empty', label: 'Play now' }} />

<Story name="Secondary" args={{ preset: 'empty', variant: 'secondary', label: 'Back' }} />

<Story name="Danger" args={{ preset: 'empty', variant: 'danger', label: 'Delete' }} />

<Story name="With Icon Start" args={{ preset: 'empty', label: 'Play', icon: 'play' }} />

<Story name="With Icon End" args={{ preset: 'empty', label: 'Open', icon: 'arrow-right', iconPosition: 'end' }} />

<Story name="Busy" args={{ preset: 'empty', label: 'Saving', icon: 'save', busy: true }} />

<Story name="Disabled" args={{ preset: 'empty', label: 'Play', icon: 'play', disabled: true }} />
