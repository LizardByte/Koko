import type { StorybookConfig } from '@storybook/sveltekit';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const darkCss = readFileSync(fileURLToPath(new URL('./storybook-dark.css', import.meta.url)), 'utf8');

const config: StorybookConfig = {
  stories: ['../src/**/*.mdx', '../src/**/*.stories.@(js|ts|svelte)'],
  addons: [
    '@storybook/addon-svelte-csf',
    '@chromatic-com/storybook',
    '@storybook/addon-vitest',
    '@storybook/addon-a11y',
    '@storybook/addon-docs',
  ],
  framework: {
    name: '@storybook/sveltekit',
    options: {},
  },
  docs: {
    autodocs: 'tag',
  },
  // The Koko client is dark-only, so force the Storybook chrome (manager
  // sidebar + docs page) to dark too. The story canvas already renders dark
  // via app.css (color-scheme: dark) + backgrounds.default in preview.ts.
  managerHead: `<style>${darkCss}</style>`,
  previewHead: `<style>${darkCss}</style>`,
  viteFinal: async (config) => {
    // Stub the SvelteKit $app modules — Storybook isn't a router, so components
    // that read $app/state (page) or $app/navigation (goto) get a mutable mock.
    // See src/lib/storybook/mockAppState.svelte.ts + mockAppNavigation.ts.
    config.resolve ??= {};
    config.resolve.alias = {
      ...(config.resolve.alias as Record<string, string> | undefined),
      '$app/state': new URL('../src/lib/storybook/mockAppState.svelte.ts', import.meta.url)
        .pathname,
      '$app/navigation': new URL('../src/lib/storybook/mockAppNavigation.ts', import.meta.url)
        .pathname,
    };

    // Force mock API mode in Storybook. Stories rely on the mock dispatch layer
    // (src/lib/mockApi.ts) + store presets — without this, components try to
    // hit a real backend and the artwork resolver / fixture seeding never runs.
    // `import.meta.env.VITE_USE_MOCK_API` is read as a Vite env var (see
    // api.ts:606), so we define the full property access on import.meta.env.
    config.define ??= {};
    const define = config.define as Record<string, string>;
    define['import.meta.env.VITE_USE_MOCK_API'] = JSON.stringify('true');

    return config;
  },
};

export default config;
