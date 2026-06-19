import type { Preview } from '@storybook/sveltekit';
import { themes, ensure } from 'storybook/theming';
import '../src/app.css'; // design tokens + shared rules — components depend on these
import { withStores } from './decorators/withStores';

// The Koko client is dark-only. Force the docs page to the dark theme too —
// the manager's `addons.setConfig({ theme: themes.dark })` (see manager.ts)
// doesn't reliably propagate to docs pages (storybookjs/storybook#28664), so
// we set it explicitly here. `ensure` makes the theme resolve synchronously.
ensure(themes.dark);

const preview: Preview = {
  parameters: {
    controls: {
      matchers: {
        color: /(background|color)$/i,
        date: /Date$/i,
      },
    },
    docs: {
      // Force the docs page chrome to dark (sidebar is handled by manager.ts).
      theme: themes.dark,
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
