<script lang="ts">
  // PersonCredits — replaces renderPersonCreditGroup() + bindPersonCreditTrays()
  // (../client-web/src/app/itemPersonView.ts:418-488, 649-710). The credit grid
  // with expandable season/episode trays. The vanilla client positions trays
  // via CSS `order` + `is-active`, measuring offsetTop with an 8px row
  // tolerance. Here we use reactive state: the active group id determines
  // which tray is open, and a bound callback computes the order so the tray
  // lands right after the hovered card's visual row.
  import MediaCard from './MediaCard.svelte';
  import Icon from './Icon.svelte';
  import { countLabel } from '$lib';
  import type { MetadataPersonItemCredit, MediaItemSummary } from '$lib/api';

  type Props = { credits: MetadataPersonItemCredit[] };
  let { credits }: Props = $props();

  interface SeasonGroup {
    season: MediaItemSummary;
    episodes: MetadataPersonItemCredit[];
  }
  interface CreditGroup {
    root: MediaItemSummary;
    seasons: SeasonGroup[];
    directEpisodes: MetadataPersonItemCredit[];
  }

  // Bucket credits by root show, then by season — mirrors personCreditGroups.
  const groups = $derived<CreditGroup[]>(buildGroups(credits));

  /** Bucket one root's credits into seasons + direct episodes. */
  function bucketRootCredits(bucket: MetadataPersonItemCredit[]): {
    seasons: SeasonGroup[];
    directEpisodes: MetadataPersonItemCredit[];
  } {
    const seasons: SeasonGroup[] = [];
    const directEpisodes: MetadataPersonItemCredit[] = [];
    const bySeason = new Map<number, MetadataPersonItemCredit[]>();
    for (const credit of bucket) {
      if (credit.item.item_type === 'season') {
        // A season-level credit: becomes its own group with no episodes yet.
        bySeason.set(credit.item.id, []);
      } else if (credit.item.item_type === 'episode' && credit.item.parent_id) {
        const seasonBucket = bySeason.get(credit.item.parent_id) ?? [];
        seasonBucket.push(credit);
        bySeason.set(credit.item.parent_id, seasonBucket);
      } else {
        directEpisodes.push(credit);
      }
    }
    for (const [seasonId, episodes] of bySeason) {
      const firstEpisode = episodes[0];
      const season = firstEpisode?.item.hierarchy?.find((entry) => entry.id === seasonId)
        ?? bucket.find((credit) => credit.item.id === seasonId)?.item;
      if (season) seasons.push({ season, episodes });
    }
    seasons.sort((a, b) => (a.season.season_number ?? 0) - (b.season.season_number ?? 0));
    return { seasons, directEpisodes };
  }

  function buildGroups(allCredits: MetadataPersonItemCredit[]): CreditGroup[] {
    const byRoot = new Map<number, MetadataPersonItemCredit[]>();
    for (const credit of allCredits) {
      const rootId = rootIdFor(credit);
      const bucket = byRoot.get(rootId) ?? [];
      bucket.push(credit);
      byRoot.set(rootId, bucket);
    }
    const result: CreditGroup[] = [];
    for (const [rootId, bucket] of byRoot) {
      const root = bucket[0].item.hierarchy?.find((entry) => entry.id === rootId) ?? bucket[0].item;
      const { seasons, directEpisodes } = bucketRootCredits(bucket);
      result.push({ root, seasons, directEpisodes });
    }
    result.sort((a, b) => a.root.display_title.localeCompare(b.root.display_title));
    return result;
  }

  function rootIdFor(credit: MetadataPersonItemCredit): number {
    if (credit.item.item_type === 'show') return credit.item.id;
    if (credit.hierarchy.length) return credit.hierarchy[0].id;
    return credit.item.id;
  }

  // Active group + active season for tray expansion.
  let activeGroupId = $state<number | undefined>(undefined);
  let activeSeasonId = $state<number | undefined>(undefined);

  // Grid element ref — used to query cards by data attribute for tray order.
  let grid: HTMLElement | undefined = $state();
  // The CSS order value assigned to the active group's tray, computed on
  // activation so the tray lands after the hovered card's visual row.
  let activeTrayOrder = $state(0);

  function activateGroup(groupId: number) {
    activeGroupId = groupId;
    activeSeasonId = undefined;
    activeTrayOrder = computeTrayOrder(groupId);
  }

  function deactivateGroup() {
    activeGroupId = undefined;
    activeSeasonId = undefined;
  }

  function activateSeason(seasonId: number) {
    activeSeasonId = seasonId;
  }

  function deactivateSeason() {
    activeSeasonId = undefined;
  }

  // Compute the CSS order for a tray so it lands right after the hovered
  // card's visual row (the 8px-tolerance offsetTop match from the vanilla
  // client). Cards in the same row share ~offsetTop; the tray takes that row's
  // offsetTop as its order key, which grid auto-flow places in the right gap.
  function computeTrayOrder(groupId: number): number {
    if (!grid) return 0;
    const card = grid.querySelector<HTMLElement>(`[data-person-credit-id="${groupId}"]`);
    return card ? card.offsetTop : 0;
  }
</script>

<section class="panel page-panel item-section">
  <div class="section-heading section-heading-actions">
    <div><h3>Credits</h3></div>
    <span class="muted">{countLabel(groups.length, 'title')}</span>
  </div>
  {#if groups.length === 0}
    <div class="empty-state tight">No linked items are stored for this person yet.</div>
  {:else}
    <div class="person-credit-grid" bind:this={grid}>
      {#each groups as group (group.root.id)}
        <div
          class="person-credit-card"
          class:is-active={activeGroupId === group.root.id}
          data-person-credit-card
          data-person-credit-id={group.root.id}
          role="button"
          tabindex="0"
          aria-label="Show seasons for {group.root.display_title}"
          onmouseover={() => activateGroup(group.root.id)}
          onfocus={() => activateGroup(group.root.id)}
          onkeydown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              activateGroup(group.root.id);
            }
          }}
        >
          <MediaCard item={group.root} />
        </div>
        {#if group.seasons.length}
          <div
            class="person-credit-tray person-season-tray"
            class:is-active={activeGroupId === group.root.id}
            style="order: {activeGroupId === group.root.id ? activeTrayOrder : 0}"
            data-person-credit-tray
            data-person-credit-id={group.root.id}
          >
            <div class="person-credit-tray-heading">
              <span>
                {[countLabel(group.seasons.length, 'season'), countLabel(group.seasons.reduce((sum, season) => sum + season.episodes.length, 0), 'episode')].filter(Boolean).join(' · ') || 'Credits'}
              </span>
              <button class="person-credit-tray-close" type="button" title="Collapse row" aria-label="Collapse row" onclick={deactivateGroup}>
                <Icon name="x" size={16} />
              </button>
            </div>
            <div class="person-season-credit-grid">
              {#each group.seasons as seasonGroup (seasonGroup.season.id)}
                <div
                  class="person-season-credit-card"
                  class:is-active={activeSeasonId === seasonGroup.season.id}
                  data-person-season-credit-card
                  data-person-season-credit-id={seasonGroup.season.id}
                  role="button"
                  tabindex="0"
                  aria-label="Show episodes for {seasonGroup.season.display_title}"
                  onmouseover={() => activateSeason(seasonGroup.season.id)}
                  onfocus={() => activateSeason(seasonGroup.season.id)}
                  onkeydown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault();
                      activateSeason(seasonGroup.season.id);
                    }
                  }}
                >
                  <MediaCard item={seasonGroup.season} />
                </div>
                {#if seasonGroup.episodes.length}
                  <div
                    class="person-credit-tray person-episode-tray"
                    class:is-active={activeSeasonId === seasonGroup.season.id}
                    data-person-season-credit-tray
                    data-person-season-credit-id={seasonGroup.season.id}
                  >
                    <div class="person-credit-tray-heading">
                      <span>{countLabel(seasonGroup.episodes.length, 'episode')}</span>
                      <button class="person-credit-tray-close" type="button" title="Collapse" aria-label="Collapse" onclick={deactivateSeason}>
                        <Icon name="x" size={16} />
                      </button>
                    </div>
                    <div class="person-episode-credit-grid">
                      {#each seasonGroup.episodes as episodeCredit (episodeCredit.item.id)}
                        <MediaCard item={episodeCredit.item} />
                      {/each}
                    </div>
                  </div>
                {/if}
              {/each}
            </div>
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</section>

<style>
  /*
   * Component-owned (PersonCredits-only). Values mirror vanilla style.css
   * :1618-1693. .item-section is shared (app.css).
   */
  .section-heading {
    margin-bottom: 0.8rem;
  }

  .person-credit-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 1rem;
    align-items: start;
  }

  .person-credit-card,
  .person-season-credit-card {
    display: grid;
    align-items: start;
  }

  .person-credit-card.is-active,
  .person-season-credit-card.is-active {
    z-index: 1;
  }

  .person-credit-tray {
    display: none;
    grid-column: 1 / -1;
    padding: 0.9rem;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.055);
    border: 1px solid rgba(255, 255, 255, 0.09);
  }

  .person-credit-tray.is-active {
    display: grid;
    gap: 0.75rem;
  }

  .person-credit-tray-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    font-size: 0.78rem;
    color: var(--muted);
  }

  .person-credit-tray-close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.8rem;
    height: 1.8rem;
    padding: 0;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
    color: #dfe9ff;
    box-shadow: none;
  }

  .person-credit-tray-close:hover,
  .person-credit-tray-close:focus-visible {
    background: #d8ffe9;
    color: #061018;
  }

  .person-credit-tray-close :global(svg) {
    width: 0.95rem;
    height: 0.95rem;
  }

  .person-season-credit-grid,
  .person-episode-credit-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 0.8rem;
    align-items: start;
  }

  .person-episode-credit-grid {
    grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
  }
</style>
