<script module>
  // CollapsibleText stories. The component reads the `ui` store for expanded
  // state, so the WithStores decorator (preset 'empty') seeds a clean store.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import CollapsibleText from './CollapsibleText.svelte';

  const { Story } = defineMeta({
    title: 'Components/CollapsibleText',
    component: CollapsibleText,
    tags: ['autodocs'],
    args: { preset: 'empty' },
    parameters: {
      docs: {
        description: {
          component:
            'Clamped text block with a "... see more" / "show less" toggle. Collapse triggers above COLLAPSIBLE_TEXT_LENGTH or COLLAPSIBLE_TEXT_LINE_COUNT. Expanded state survives re-renders via the `ui` store (keyed by storageKey).',
        },
      },
    },
  });

  const longText =
    'Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.';
</script>

<Story name="Collapsed" args={{ preset: 'empty', text: longText, storageKey: 'sb-collapsed' }} />

<Story name="Short" args={{ preset: 'empty', text: 'A short description with no toggle.', storageKey: 'sb-short' }} />

<Story name="Expanded" args={{ preset: 'empty', text: longText, storageKey: 'sb-expanded' }} />
