// SvelteKit app type declarations.

declare global {
  namespace App {
    // Page state for shallow routing. When the player overlay opens, we
    // pushState with { player: {...} } so the browser back-button closes the
    // player instead of leaving the page. See playback store + layout
    // beforeNavigate hook.
    interface PageState {
      player?: {
        itemId: number;
        startMs: number;
        kind: 'media' | 'trailer';
      };
    }
  }
}

export {};
