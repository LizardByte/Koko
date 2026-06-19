import type { Preview } from '@storybook/sveltekit';
import '../src/app.css'; // design tokens + shared rules — components depend on these
import { withStores } from './decorators/withStores';

const preview: Preview = {
  parameters: {
    controls: {
      matchers: {
        color: /(background|color)$/i,
        date: /Date$/i,
      },
    },
    backgrounds: {
      // The Koko client is dark-only (color-scheme: dark hardcoded in app.css).
      // Force the Storybook canvas to match so stories render on the right bg.
      default: 'dark',
      values: [{ name: 'dark', value: '#0c111d' }],
    },
    a11y: {
      // 'todo' - show a11y violations in the test UI only
      // 'error' - fail CI on a11y violations
      // 'off' - skip a11y checks entirely
      test: 'todo',
    },
  },
  // Seed stores + mock $app/state around every story. Stories select a fixture
  // preset via args.preset (default 'empty'); see decorators/withStores.ts.
  decorators: [withStores],
};

export default preview;
