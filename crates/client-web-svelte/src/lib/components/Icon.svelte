<script lang="ts" module>
  // Icon — thin wrapper over @lucide/svelte giving a string-name API matching
  // the vanilla client's `<i data-lucide="name">` usage. The vanilla client
  // re-hydrates icons after every render via createIcons(); here each icon is
  // a native Svelte 5 component rendered inline, so no hydration step exists.
  //
  // Only the names the client actually uses are mapped. Adding one = add a
  // key here and the corresponding `import` below.
  import LayoutGrid from '@lucide/svelte/icons/layout-grid';
  import Clapperboard from '@lucide/svelte/icons/clapperboard';
  import Tv from '@lucide/svelte/icons/tv';
  import Music from '@lucide/svelte/icons/music';
  import Image from '@lucide/svelte/icons/image';
  import Book from '@lucide/svelte/icons/book';
  import Film from '@lucide/svelte/icons/film';
  import FolderSync from '@lucide/svelte/icons/folder-sync';
  import DatabaseZap from '@lucide/svelte/icons/database-zap';
  import Volume2 from '@lucide/svelte/icons/volume-2';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import ChevronLeft from '@lucide/svelte/icons/chevron-left';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import X from '@lucide/svelte/icons/x';
  import UserPlus from '@lucide/svelte/icons/user-plus';
  import Save from '@lucide/svelte/icons/save';
  import Search from '@lucide/svelte/icons/search';
  import Play from '@lucide/svelte/icons/play';
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import LogIn from '@lucide/svelte/icons/log-in';
  import LogOut from '@lucide/svelte/icons/log-out';
  import Layers from '@lucide/svelte/icons/layers';
  import Settings from '@lucide/svelte/icons/settings';
  import Star from '@lucide/svelte/icons/star';
  // Icons used by views not yet ported (Phase 5 settings, playback spike).
  // Added now so a missing key never silently renders nothing — see
  // PORTING_GUIDELINES.md "Icons".
  import House from '@lucide/svelte/icons/house';
  import Link2 from '@lucide/svelte/icons/link-2';
  import Plus from '@lucide/svelte/icons/plus';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import Languages from '@lucide/svelte/icons/languages';
  import Maximize from '@lucide/svelte/icons/maximize';
  import Pause from '@lucide/svelte/icons/pause';
  import PictureInPicture from '@lucide/svelte/icons/picture-in-picture';
  import SkipBack from '@lucide/svelte/icons/skip-back';
  import SkipForward from '@lucide/svelte/icons/skip-forward';
  import VolumeX from '@lucide/svelte/icons/volume-x';
  import type { Component } from 'svelte';

  export const ICONS: Record<string, Component> = {
    'layout-grid': LayoutGrid,
    clapperboard: Clapperboard,
    tv: Tv,
    music: Music,
    image: Image,
    book: Book,
    film: Film,
    'folder-sync': FolderSync,
    'database-zap': DatabaseZap,
    'volume-2': Volume2,
    'arrow-right': ArrowRight,
    'arrow-left': ArrowLeft,
    'chevron-left': ChevronLeft,
    'chevron-right': ChevronRight,
    x: X,
    'user-plus': UserPlus,
    save: Save,
    search: Search,
    play: Play,
    'circle-check': CircleCheck,
    'triangle-alert': TriangleAlert,
    'refresh-cw': RefreshCw,
    'log-in': LogIn,
    'log-out': LogOut,
    layers: Layers,
    settings: Settings,
    star: Star,
    // Phase 5 / playback-spike icons (pre-registered — see header comment).
    house: House,
    'link-2': Link2,
    plus: Plus,
    'trash-2': Trash2,
    languages: Languages,
    maximize: Maximize,
    pause: Pause,
    'picture-in-picture': PictureInPicture,
    'skip-back': SkipBack,
    'skip-forward': SkipForward,
    'volume-x': VolumeX,
  };
</script>

<script lang="ts">
  type Props = {
    name: string;
    size?: number | string;
    strokeWidth?: number;
    class?: string;
  };
  let { name, size = 24, strokeWidth = 2, class: className = '' }: Props = $props();

  const IconComponent = $derived(ICONS[name]);
</script>

{#if IconComponent}
  <IconComponent {size} {strokeWidth} class={className} />
{/if}
