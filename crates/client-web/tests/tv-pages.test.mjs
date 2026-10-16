import assert from 'node:assert/strict';
import { after, beforeEach, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/', origin: 'http://localhost' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const tv = await server.ssrLoadModule('/src/app/tvItemView.ts');
const { renderItemPage } = await server.ssrLoadModule('/src/app/itemPersonView.ts');
const { renderRating } = await server.ssrLoadModule('/src/app/ratingView.ts');
const mock = await server.ssrLoadModule('/src/mockApi.ts');
let show, season, episode, first, next;

beforeEach(() => {
  show = { ...mock.getMockItem(201), poster_url: '/poster', logo_url: '/logo', backdrop_url: '/backdrop', artwork_updated_at: 123, rating: 8.2, rating_provider_id: 'tmdb' };
  season = { ...mock.getMockItem(202), hierarchy: [show], poster_url: '/season-poster', artwork_updated_at: 234 };
  first = { ...mock.getMockItem(203), id: 210, episode_number: 1, display_title: 'First episode', rating: 8.4, rating_provider_id: 'tmdb' };
  episode = { ...mock.getMockItem(203), id: 211, episode_number: 2, display_title: 'The middle <episode>', hierarchy: [show, season], playback_position_ms: 74_000, playback_duration_ms: 2_700_000, playback_completed: false, watch_count: 0 };
  next = { ...episode, id: 212, episode_number: 3, display_title: 'Next episode' };
  season.children = [next, first, episode];
  show.children = [season];
  state.libraryItems = [next, season, first, episode, show];
  state.libraryItemsLoading = false;
  state.metadataProviders = mock.getMockMetadataProviders();
  state.selectedItemMetadata = undefined;
  state.bootstrap = { current_user: { id: 1, admin: true } };
  state.isLoading = false;
  state.itemMoreOpen = state.resumeOptionsOpen = state.itemManagementOpen = state.isTrailerMenuOpen = false;
  state.selectedPlayback = undefined;
});

test('episodes use their own thumbnail as the cover and their own backdrop when available', () => {
  episode.artwork_updated_at = 345;
  episode.backdrop_url = '/episode-backdrop';
  const artwork = tv.itemArtwork(episode);
  assert.match(artwork.poster, /items\/211\/artwork\?kind=poster&rev=345/);
  assert.match(artwork.logo, /items\/201\/artwork\?kind=logo&rev=123/);
  assert.match(artwork.backdrop, /items\/211\/artwork\?kind=backdrop&rev=345/);
  state.selectedItem = episode; state.route = { page: 'item', itemId: episode.id };
  const html = renderItemPage();
  assert.match(html, /data-item-cover-image[^>]*items\/211\/artwork\?kind=poster/);
  assert.match(html, /item-opening-backdrop[^>]*items\/211\/artwork\?kind=backdrop/);
});

test('episode backgrounds fall back to season then show artwork without reusing the thumbnail', () => {
  season.backdrop_url = '/season-backdrop';
  assert.match(tv.itemArtwork(episode).backdrop, /items\/202\/artwork\?kind=backdrop&rev=234/);
  season.backdrop_url = undefined;
  assert.match(tv.itemArtwork(episode).backdrop, /items\/201\/artwork\?kind=backdrop&rev=123/);
  show.backdrop_url = undefined;
  assert.equal(tv.itemArtwork(episode).backdrop, undefined);
  episode.poster_url = undefined;
  assert.equal(tv.itemArtwork(episode).poster, undefined);
});

test('linked episode backdrop metadata takes precedence over inherited artwork', () => {
  season.backdrop_url = '/season-backdrop';
  state.selectedItemMetadata = { item_id: episode.id, matches: [{ cached_backdrop_path: '/cached/episode.jpg' }] };
  assert.match(tv.itemArtwork(episode).backdrop, /items\/211\/artwork\?kind=backdrop/);
  state.selectedItemMetadata.item_id = next.id;
  assert.match(tv.itemArtwork(episode).backdrop, /items\/202\/artwork\?kind=backdrop/);
});

test('season posters can fall back through the backend while season identity stays explicit', () => {
  season.poster_url = undefined;
  assert.match(tv.itemArtwork(season).poster, /items\/202\/artwork\?kind=poster/);
  state.selectedItem = season; state.route = { page: 'item', itemId: season.id };
  const html = renderItemPage();
  assert.match(html, /class="item-title-logo"[^>]*items\/201/);
  assert.match(html, /class="item-context-title">Season 1<\/h2>/);
  assert.ok(html.indexOf('class="season-tabs"') > html.indexOf('class="item-action-dock"'));
  assert.ok(html.indexOf('class="season-tabs"') < html.indexOf('class="item-description-section"'));
  assert.match(html, /data-item-row="episodes"/);
  assert.doesNotMatch(html, /class="episode-list"|class="item-section episode-browser"/);
});

test('episode identity, Back, Previous/Next, and its related row stay within its own season', () => {
  const unrelated = { ...next, id: 999, parent_id: 999 };
  const otherLibrary = { ...next, id: 888, library_id: 99 };
  state.libraryItems.push(unrelated, otherLibrary);
  state.selectedItem = episode; state.route = { page: 'item', itemId: episode.id };
  const html = renderItemPage();
  assert.match(html, /S01E02 · The middle &lt;episode&gt;/);
  assert.match(html, /Back to season/);
  assert.match(html, /episode-navigation-button" data-item-id="210"/);
  assert.match(html, /episode-navigation-button" data-item-id="212"/);
  assert.match(html, /More from this season <span class="item-row-count">· 2/);
  assert.doesNotMatch(html, /data-item-id="999"|data-item-id="888"/);
  assert.doesNotMatch(html, /item-thumbnail/);
  assert.equal(tv.renderEpisodeNavigation(first).includes('Previous episode'), false);
  assert.equal(tv.renderEpisodeNavigation(next).includes('Next episode'), false);
});

test('TV playback identifies the episode inside the button and held options restart that episode', () => {
  season.playback_target = { item_id: episode.id, start_ms: 74_000, resume: true, season_number: 1, episode_number: 2, display_title: episode.display_title, label: 'Resume S01E02' };
  season.restart_playback_target = { ...season.playback_target, item_id: first.id, start_ms: 0, resume: false, display_title: first.display_title };
  state.selectedItem = season; state.route = { page: 'item', itemId: season.id };
  const html = renderItemPage();
  assert.match(html, /Continue S01E02/);
  assert.match(html, /class="button-secondary-label">The middle &lt;episode&gt; · 43:46 remaining/);
  const held = html.slice(html.indexOf('id="resume-playback-menu"'), html.indexOf('</section>', html.indexOf('id="resume-playback-menu"')));
  assert.match(held, /data-playback-target-item-id="211" data-playback-target-start-ms="0"/);
  assert.match(held, /Restart episode/);
  assert.match(html, /id="restart-season"[^>]*data-playback-target-item-id="210"/);
  assert.match(html, /id="restart-show"[^>]*data-playback-target-item-id="210"/);
  assert.ok(html.indexOf('id="restart-season"') > html.indexOf('id="item-more"'));
  assert.doesNotMatch(html, /class="item-playback-context"/);
});

test('restart targets skip missing/virtual episodes and select the first episode across season boundaries', () => {
  const laterSeason = { ...season, id: 302, season_number: 2, children: [] };
  const later = { ...next, id: 303, parent_id: 302, season_number: 2, episode_number: 1, hierarchy: [show, laterSeason] };
  first.missing_since = 123;
  episode.playable = false;
  state.libraryItems.push(laterSeason, later);
  const targets = tv.tvRestartTargets(later);
  assert.equal(targets.find((target) => target.label === 'Restart show').item_id, next.id);
  assert.equal(targets.find((target) => target.label === 'Restart season').item_id, later.id);
  assert.deepEqual(targets.map((target) => target.start_ms), [0, 0]);
});

test('ancestor restart actions wait for the catalog instead of using the current episode', () => {
  state.libraryItemsLoading = true;
  assert.deepEqual(tv.tvRestartTargets(episode), []);
  const target = { item_id: first.id, start_ms: 0, display_title: first.display_title, resume: false, label: 'Start season' };
  season.restart_playback_target = target;
  assert.deepEqual(tv.tvRestartTargets(season), [{ ...target, label: 'Restart season' }]);
  state.libraryItemsLoading = false;
  state.libraryItems = [];
  assert.deepEqual(tv.tvRestartTargets(episode), []);
});

test('ratings use actual local attribution images and label inherited values without a star', () => {
  const own = renderRating({ ...episode, rating: 9.1, rating_provider_id: 'tvdb', rating_source_item_type: 'episode' });
  assert.match(own, /aria-label="Rating from TheTVDB: 9\.1"/);
  assert.match(own, /class="rating-source-logo"[^>]*tvdb-dark\.png/);
  assert.doesNotMatch(own, /Series|data-lucide="star"/);
  const inherited = renderRating({ ...episode, rating: 8.2, rating_provider_id: 'tmdb', rating_source_item_type: 'show' });
  assert.match(inherited, /Series rating from TheMovieDB: 8\.2/);
  assert.match(inherited, /<span>Series<\/span>/);
  assert.match(inherited, /tmdb\.svg/);
  assert.equal(renderRating({ ...episode, rating: null }), '');
  assert.equal(renderRating({ ...episode, rating: Number.NaN }), '');
  assert.match(renderRating({ ...episode, rating: 7, rating_provider_id: 'unknown-provider' }), /rating-source-name">unknown-provider/);
});

test('episode cards retain a circular ring, rating attribution, and escaped title and description', () => {
  const html = tv.renderTvEpisodeCard({ ...episode, overview: '<script>spoiler</script>', rating: 8.1, rating_provider_id: 'tmdb' });
  assert.match(html, /class="media-card-progress"/);
  assert.match(html, /episode-number-badge">S01E02/);
  assert.match(html, /rating-source-logo/);
  assert.match(html, /data-episode-still/);
  assert.match(html, /episode-still-fallback" hidden/);
  assert.match(html, /The middle &lt;episode&gt;/);
  assert.match(html, /&lt;script&gt;spoiler&lt;\/script&gt;/);
  assert.doesNotMatch(html, /<script>/);
});

test('TV items without a show ancestor keep one clear heading', () => {
  assert.equal(tv.renderTvTitle({ ...season, hierarchy: [] }), '<h2 class="item-title-fallback">Season 1</h2>');
  assert.equal(tv.renderTvTitle({ ...episode, hierarchy: [] }), '<h2 class="item-title-fallback">S01E02 · The middle &lt;episode&gt;</h2>');
});

test('completed episodes put the checkmark on artwork and order More restarts from episode to show', () => {
  episode.playback_completed = true;
  episode.watch_count = 1;
  state.selectedItem = episode; state.route = { page: 'item', itemId: episode.id };
  const html = renderItemPage();
  const badges = html.slice(html.indexOf('class="hero-meta-row"'), html.indexOf('class="item-actions-anchor"'));
  assert.doesNotMatch(badges, /Watched|watched-fact/);
  const card = tv.renderTvEpisodeCard(episode);
  assert.match(card, /media-card-progress is-watched/);
  assert.match(card, /data-lucide="check"/);
  assert.equal([...card.matchAll(/class="media-card-progress/g)].length, 1);
  const menu = html.slice(html.indexOf('class="item-more-menu"'));
  assert.ok(menu.includes('Restart episode'));
  assert.ok(menu.indexOf('Restart episode') < menu.indexOf('Restart season'));
  assert.ok(menu.indexOf('Restart season') < menu.indexOf('Restart show'));
  assert.deepEqual(tv.tvRestartTargets(episode).map((target) => target.label), ['Restart season', 'Restart show']);
});
