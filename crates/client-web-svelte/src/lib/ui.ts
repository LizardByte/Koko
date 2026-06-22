// Pure UI helpers — port of the pure (non-rendering) functions from
// ../client-web/src/app/ui.ts. The string-rendering helpers (renderIcon,
// renderButtonContent, renderCollapsibleText, renderUserAvatar) become Svelte
// components instead — see ./components/. Only the dispatch/format functions
// that have no better Svelte parallel stay here.
import type { MediaItemSummary } from "./api";
import { formatDuration } from "./format";

export type AppIconName = string;

export function selectedLibraryIcon(kind?: string): AppIconName {
  switch (kind) {
    case "mixed":
      return "layout-grid";
    case "movies":
      return "clapperboard";
    case "shows":
      return "tv";
    case "music":
      return "music";
    case "photos":
      return "image";
    case "books":
      return "book";
    case "home_videos":
      return "film";
    default:
      return "layout-grid";
  }
}

export function humanizeItemType(itemType: string): string {
  switch (itemType) {
    case "show":
      return "Show";
    case "season":
      return "Season";
    case "episode":
      return "Episode";
    case "movie":
      return "Movie";
    case "track":
      return "Track";
    case "photo":
      return "Photo";
    case "book":
      return "Book";
    default:
      return itemType
        .replace(/_/g, " ")
        .replace(/\b\w/g, (character) => character.toUpperCase());
  }
}

export function formatChildCount(item: MediaItemSummary): string {
  if (!item.child_count) {
    return formatDuration(item.duration_ms);
  }
  if (item.item_type === "show") {
    const seasonCount = item.available_season_count ?? item.child_count;
    return `${seasonCount} season${seasonCount === 1 ? "" : "s"}`;
  }
  if (item.item_type === "season") {
    return `${item.child_count} episode${item.child_count === 1 ? "" : "s"}`;
  }
  return `${item.child_count} item${item.child_count === 1 ? "" : "s"}`;
}

export function libraryStatusLabel(status: string): string {
  switch (status) {
    case "never_scanned":
      return "Pending first scan";
    case "available":
      return "Ready";
    case "missing_path":
      return "Missing path";
    case "not_directory":
      return "Invalid folder";
    case "unreadable":
      return "Unreadable";
    case "empty_path":
      return "No folder";
    default:
      return status.replace(/_/g, " ");
  }
}

/** Count label, e.g. "3 items" / "1 item" / '' for 0. */
export function countLabel(count: number, noun: string): string {
  if (count === 0) {
    return "";
  }
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}

/** Subtitle for a media card: episode "Episode N", season "Season N", else display_subtitle. */
export function itemCardSubtitle(item: MediaItemSummary): string | undefined {
  if (item.display_subtitle) {
    return item.display_subtitle;
  }
  if (item.item_type === "episode" && typeof item.episode_number === "number") {
    return `Episode ${item.episode_number}`;
  }
  if (item.item_type === "season" && typeof item.season_number === "number") {
    return `Season ${item.season_number}`;
  }
  return undefined;
}
