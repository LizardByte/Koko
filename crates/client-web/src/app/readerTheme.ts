/** Keeps reflowable ebook text readable in Koko's dark appearance without changing text anchors or artwork. */

/** Override publisher text paints, including inline priorities, while preserving typography and SVG illustrations. */
export function applyReaderTextTheme(doc: Document): void {
  const foreground = '#f4f7fb', background = '#0c111d';
  doc.documentElement.style.setProperty('background-color', background, 'important');
  doc.documentElement.style.setProperty('color', foreground, 'important');
  doc.documentElement.style.setProperty('color-scheme', 'dark', 'important');
  doc.documentElement.style.setProperty('--theme-bg-color', background);
  for (const element of doc.querySelectorAll<HTMLElement>('body, body *')) {
    if (element.namespaceURI !== 'http://www.w3.org/1999/xhtml') continue;
    element.style.setProperty('color', foreground, 'important');
    element.style.setProperty('-webkit-text-fill-color', 'currentColor', 'important');
    element.style.setProperty('background-color', 'transparent', 'important');
  }
}
