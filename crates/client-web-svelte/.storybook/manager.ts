// Storybook manager — runs in the Storybook UI shell (sidebar + toolbar), not
// in the story canvas. The Koko client is dark-only, so we force the Storybook
// UI to the built-in `dark` theme.
//
// Why the theme API and not CSS overrides?
//   Storybook 10 renders its entire UI with Emotion CSS-in-JS (hashed class
//   names like `css-1z0jwx`). There are no stable CSS variables or class names
//   to override — a `<style>` block in managerHead/previewHead gets ignored by
//   the Emotion-generated styles. `addons.setConfig({ theme })` is the
//   supported way to theme the UI; it feeds the theme into Emotion so the
//   correct dark styles are generated.
import { addons } from 'storybook/manager-api';
import { themes } from 'storybook/theming';

addons.setConfig({
  theme: themes.dark,
});
