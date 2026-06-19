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

    // Make app.css (design tokens + shared rules) available in every story.
    const sveltePlugin = config.plugins?.flat().find((p) => p && typeof p === 'object' && 'name' in p && (p as { name: string }).name === 'vite-plugin-svelte');
    if (sveltePlugin && 'api' in sveltePlugin) {
      // handled via preview-body instead (more reliable across versions)
    }

    return config;
  },
};

export default config;
