// Storybook presets — named fixture bundles applied to the store singletons
// by the WithStores decorator. A story picks a preset via args.preset.
//
// Reset between stories is handled by resetStores() to prevent state bleed.

import { catalog } from '$lib/stores/catalog.svelte';
import { item } from '$lib/stores/item.svelte';
import { libraries } from '$lib/stores/libraries.svelte';
import { auth } from '$lib/stores/auth.svelte';
import { ui } from '$lib/stores/ui.svelte';
import {
  mockHome,
  mockLibraries,
  mockUser,
  movieDetail,
  movieSummary,
} from './fixtures';

export type Preset =
  | 'empty'
  | 'home'
  | 'item-movie'
  | 'item-show'
  | 'item-missing'
  | 'item-watched'
  | 'auth-logged-in'
  | 'requires-login'
  | 'requires-setup';

// Single source of truth for the preset values — consumed by the Storybook
// global argTypes.preset control (see .storybook/preview.ts) so the controls
// panel renders a dropdown instead of a free-text field. `satisfies` keeps it
// in sync with the union: adding a preset above without adding it here is a
// type error.
export const PRESETS = [
  'empty',
  'home',
  'item-movie',
  'item-show',
  'item-missing',
  'item-watched',
  'auth-logged-in',
  'requires-login',
  'requires-setup',
] as const satisfies readonly Preset[];

/** Reset all store singletons to a clean baseline (call between stories). */
export function resetStores(): void {
  catalog.home = undefined;
  catalog.libraryItems = [];
  catalog.libraryItemsLoading = false;
  catalog.searchQuery = '';
  catalog.searchResults = [];
  catalog.homeTab = 'recommended';
  catalog.activeLibraryId = undefined;
  catalog.homePreviewItemId = undefined;
  catalog.homePreviewCollectionId = undefined;
  item.item = undefined;
  item.metadata = undefined;
  item.person = undefined;
  item.playback = undefined;
  item.loading = false;
  libraries.libraries = [];
  libraries.loading = false;
  // auth.currentUser / isLoggedIn / requiresLogin are $derived getters over
  // bootstrap; reset by clearing bootstrap.
  auth.bootstrap = undefined;
  auth.users = [];
  auth.loading = false;
  ui.error = undefined;
}

/** Apply a named preset to the store singletons. */
export function applyPreset(preset: Preset): void {
  resetStores();
  const loggedInBootstrap = {
    has_users: true,
    current_user: mockUser(),
  };
  switch (preset) {
    case 'empty':
      return;
    case 'home': {
      const home = mockHome();
      libraries.libraries = mockLibraries();
      catalog.home = home;
      // libraryItems is a unique-id list in the real app (one row per item
      // from /libraries/:id/items). Home shelves can legitimately reference the
      // same item across rows (e.g. a show in both 'recently_added' and
      // 'recommended'), so dedupe by id when flattening to avoid duplicate
      // each-block keys in BrowseListing/category grids.
      const seen = new Set<number>();
      catalog.libraryItems = home.shelves
        .flatMap((s) => s.items)
        .filter((item) => {
          if (seen.has(item.id)) return false;
          seen.add(item.id);
          return true;
        });
      auth.bootstrap = loggedInBootstrap;
      return;
    }
    case 'item-movie':
      libraries.libraries = mockLibraries();
      item.item = movieDetail();
      auth.bootstrap = loggedInBootstrap;
      return;
    case 'item-show':
      libraries.libraries = mockLibraries();
      item.item = movieDetail({
        ...movieSummary({ id: 201, item_type: 'show', display_title: 'Mock Show' }),
        child_count: 2,
      });
      auth.bootstrap = loggedInBootstrap;
      return;
    case 'item-missing':
      libraries.libraries = mockLibraries();
      item.item = movieDetail({
        missing_since: 1_760_000_000,
        display_title: 'Missing Movie',
      });
      auth.bootstrap = loggedInBootstrap;
      return;
    case 'item-watched':
      libraries.libraries = mockLibraries();
      item.item = movieDetail({
        playback_completed: true,
        watch_count: 3,
        last_watched_at: 1_760_900_000,
        display_title: 'Watched Movie',
      });
      auth.bootstrap = loggedInBootstrap;
      return;
    case 'auth-logged-in':
      auth.bootstrap = loggedInBootstrap;
      return;
    case 'requires-login':
      // has_users true, no current_user → LoginScreen shows.
      auth.bootstrap = { has_users: true };
      return;
    case 'requires-setup':
      // no users yet → WelcomeScreen shows.
      auth.bootstrap = { has_users: false };
      return;
  }
}
