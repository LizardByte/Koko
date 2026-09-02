import type { StorybookConfig } from '@storybook/sveltekit';

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
  // Dark theming is applied via the proper Storybook theme API, not CSS:
  //  - manager.ts sets `addons.setConfig({ theme: themes.dark })` for the
  //    sidebar + toolbar (Storybook 10 uses Emotion CSS-in-JS, so overriding
  //    CSS variables does nothing — the theme API generates the right styles).
  //  - preview.ts sets `parameters.docs.theme = themes.dark` for the docs page.
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
