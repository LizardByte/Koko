// Local CC0 artwork registry for Storybook mock mode.
//
// In the real app, artwork is served by the Rust media server at
// /api/v1/items/:id/artwork?kind=poster. In Storybook there's no server, so
// `getArtworkUrl` would return a fake `mock://...` URL that 404s (relying on
// gradient fallbacks). To make stories show real imagery, we bundle a small
// set of CC0 photos (from picsum.photos / Unsplash, deterministic by seed) and
// map fixture item ids to them here.
//
// Add new artworks: drop a file in src/lib/assets/artworks/, import it below,
// and register it for the matching fixture item id + kind. Unknown ids fall
// through to the mock:// placeholder (gradient fallback) as before.
import moviePoster from '$lib/assets/artworks/movie-101.jpg';
import showPoster from '$lib/assets/artworks/show-201.jpg';
import seasonPoster from '$lib/assets/artworks/season-202.jpg';
import episodeBackdrop from '$lib/assets/artworks/episode-203.jpg';
import trackPoster from '$lib/assets/artworks/track-103.jpg';

export type ArtworkKind = 'poster' | 'backdrop' | 'logo';

const REGISTRY: Record<number, Partial<Record<ArtworkKind, string>>> = {
  // movieSummary — id 101
  101: { poster: moviePoster, backdrop: moviePoster },
  // showSummary — id 201
  201: { poster: showPoster, backdrop: showPoster },
  // seasonSummary — id 202
  202: { poster: seasonPoster },
  // episodeSummary — id 203 (16:9 backdrop)
  203: { backdrop: episodeBackdrop },
  // trackSummary — id 103
  103: { poster: trackPoster },
};

/**
 * Resolve a fixture item's artwork to a bundled asset URL, or undefined if no
 * artwork is registered for this id/kind (caller falls back to mock:// placeholder).
 */
export function lookupArtwork(itemId: number, kind: ArtworkKind): string | undefined {
  return REGISTRY[itemId]?.[kind];
}
