/** Renders the standard browse hero shared by library previews and feature pages. */
import { escapeHtml } from './format';

interface FeatureHeroOptions {
  id?: string;
  label?: string;
  image?: string;
  className?: string;
  /** Escaped, application-rendered markup placed inside the standard copy column. */
  copy: string;
  actions?: string;
  decoration?: string;
  /** Reserve the artwork area for an existing player, without mounting another video. */
  media?: { id: string; active: boolean };
}

/** Share hero backgrounds while keeping publisher URLs inside their CSS string. */
export function featureArtworkStyle(backdropUrl?: string): string {
  if (!backdropUrl) return '';
  const cssUrl = backdropUrl.replace(/['"\\\r\n\f]/g, character => '%' + character.codePointAt(0)!.toString(16).padStart(2, '0'));
  return `style="--home-feature-image: url('${escapeHtml(cssUrl)}');"`;
}

/** Use the ordinary full-width hero, typography, backdrop, and action layout. */
export function renderFeatureHero(options: FeatureHeroOptions): string {
  const id = options.id ? `id="${escapeHtml(options.id)}"` : '';
  const label = options.label ? `aria-label="${escapeHtml(options.label)}"` : '';
  const mediaClass = options.media?.active ? ' is-active' : '';
  const media = options.media ? `<div id="${escapeHtml(options.media.id)}" class="home-feature-media${mediaClass}" aria-hidden="true"></div>` : '';
  const classes = ['home-feature',options.className,options.image && 'has-artwork',options.media?.active && 'has-media-preview'].filter(Boolean).join(' ');
  return `<section ${id} class="${escapeHtml(classes)}" data-nav-section="featured" data-nav-label="Featured" ${label} ${featureArtworkStyle(options.image)}>
    ${options.decoration ?? ''}<div class="home-feature-copy">${options.copy}</div>${options.actions ?? ''}${media}</section>`;
}
