import type { MediaItemSummary } from '../api';

/** Resolve nearest ancestors without crossing libraries or following malformed cycles. */
export function itemLineage(item: MediaItemSummary, catalog: MediaItemSummary[] = []): MediaItemSummary[] {
  const byId = new Map([...catalog, ...(item.hierarchy ?? [])].map((entry) => [entry.id, entry]));
  const lineage = [item];
  const seen = new Set([item.id]);
  let current = item;
  while (current.parent_id != null) {
    const parent = byId.get(current.parent_id);
    if (!parent || parent.library_id !== item.library_id || seen.has(parent.id)) break;
    lineage.push(parent);
    seen.add(parent.id);
    current = parent;
  }
  return lineage;
}

/** Keep a child's own artwork first, then inherit the nearest available family identity and fanart. */
export function heroPresentation(item: MediaItemSummary, catalog: MediaItemSummary[] = []) {
  const lineage = itemLineage(item, catalog);
  const logo = lineage.find((entry) => entry.logo_url);
  return { identity: logo ?? lineage[lineage.length - 1], logo, backdrop: lineage.find((entry) => entry.backdrop_url) };
}
