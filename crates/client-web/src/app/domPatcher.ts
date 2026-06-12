/** Applies DOM patches while preserving focused controls and scroll state. */
interface ScrollPosition {
  top: number;
  left: number;
}

interface RenderSnapshot {
  activeSelector?: string;
  activeSelection?: { start: number; end: number };
  scrollPositions: Map<string, ScrollPosition>;
}

interface SetElementHtmlOptions {
  preserveDom?: boolean;
  beforePatch?: (root: ParentNode) => void;
  abortEvents?: () => void;
}

const DOM_PATCH_KEY_ATTRIBUTES = [
  'id',
  'data-shelf-row',
  'data-lazy-shelf-id',
  'data-shelf-scroll',
  'data-item-id',
  'data-preview-item-id',
  'data-preview-collection-id',
  'data-person-id',
  'data-home-tab',
  'data-settings-section-path',
  'data-nav-library-id',
  'data-category-filter',
  'data-playlist-filter',
  'data-collection-filter',
  'data-link-metadata',
  'data-provider-settings',
  'data-provider-move',
  'data-run-scheduled-task',
  'data-refresh-library-id',
  'data-scan-library-id',
  'data-delete-missing-library-id',
  'data-remove-library-index',
  'data-update-user-id',
  'data-play-trailer-index',
  'data-play-extra-index',
  'data-play-selected-item-start-ms',
  'data-player-seek',
  'data-player-audio-track-index',
  'data-trailer-seek',
] as const;

function elementSelectorForFocus(element: Element | null): string | undefined {
  if (!element) {
    return undefined;
  }
  if (element.id) {
    return `#${CSS.escape(element.id)}`;
  }
  const namedControl = element instanceof HTMLInputElement
    || element instanceof HTMLSelectElement
    || element instanceof HTMLTextAreaElement
    ? element
    : undefined;
  const name = namedControl?.name;
  const formId = namedControl?.form?.id;
  return name && formId
    ? `#${CSS.escape(formId)} [name="${CSS.escape(name)}"]`
    : undefined;
}

function domPatchKey(node: Node): string | undefined {
  if (!(node instanceof Element)) {
    return undefined;
  }
  const tagName = node.tagName.toLowerCase();
  const keyAttribute = DOM_PATCH_KEY_ATTRIBUTES.find((attribute) => {
    const value = node.getAttribute(attribute);
    return value !== null && value !== '';
  });
  return keyAttribute ? `${tagName}:${keyAttribute}:${node.getAttribute(keyAttribute)}` : undefined;
}

function scrollPositionKey(element: Element, index: number): string | undefined {
  const patchKey = domPatchKey(element);
  if (patchKey) {
    return patchKey;
  }
  if (element.classList.contains('main-shell')) {
    return 'main-shell';
  }
  if (element.classList.contains('rail-nav')) {
    return 'rail-nav';
  }
  if (element.classList.contains('table-shell')) {
    const rootId = element.closest<HTMLElement>('[id]')?.id ?? 'page';
    return `table-shell:${rootId}:${index}`;
  }
  return undefined;
}

function captureRenderSnapshot(): RenderSnapshot {
  const activeElement = document.activeElement as HTMLInputElement | HTMLTextAreaElement | null;
  const activeSelection = activeElement
    && typeof activeElement.selectionStart === 'number'
    && typeof activeElement.selectionEnd === 'number'
      ? { start: activeElement.selectionStart, end: activeElement.selectionEnd }
      : undefined;
  const scrollPositions = new Map<string, ScrollPosition>();
  document.querySelectorAll<HTMLElement>('.main-shell, .rail-nav, [data-shelf-row], .table-shell').forEach((element, index) => {
    const key = scrollPositionKey(element, index);
    if (key) {
      scrollPositions.set(key, { top: element.scrollTop, left: element.scrollLeft });
    }
  });

  return {
    activeSelector: elementSelectorForFocus(document.activeElement instanceof Element ? document.activeElement : null),
    activeSelection,
    scrollPositions,
  };
}

function restoreRenderSnapshot(snapshot: RenderSnapshot): void {
  window.requestAnimationFrame(() => {
    document.querySelectorAll<HTMLElement>('.main-shell, .rail-nav, [data-shelf-row], .table-shell').forEach((element, index) => {
      const key = scrollPositionKey(element, index);
      const position = key ? snapshot.scrollPositions.get(key) : undefined;
      if (position) {
        element.scrollTop = position.top;
        element.scrollLeft = position.left;
      }
    });

    const activeElement = snapshot.activeSelector
      ? document.querySelector<HTMLInputElement | HTMLTextAreaElement>(snapshot.activeSelector)
      : undefined;
    activeElement?.focus({ preventScroll: true });
    if (snapshot.activeSelection && activeElement?.setSelectionRange) {
      activeElement.setSelectionRange(snapshot.activeSelection.start, snapshot.activeSelection.end);
    }
  });
}

function nodesCanPatch(current: Node, next: Node): boolean {
  if (current.nodeType !== next.nodeType) {
    return false;
  }
  if (current instanceof Element && next instanceof Element) {
    if (current.tagName !== next.tagName) {
      return false;
    }
    const currentKey = domPatchKey(current);
    const nextKey = domPatchKey(next);
    if (currentKey || nextKey) {
      return currentKey === nextKey;
    }
    if (
      current instanceof HTMLMediaElement
      && next instanceof HTMLMediaElement
      && current.getAttribute('src') !== next.getAttribute('src')
    ) {
      return false;
    }
  }
  return true;
}

function patchAttributes(current: Element, next: Element): void {
  current.getAttributeNames().forEach((name) => {
    if (!next.hasAttribute(name)) {
      current.removeAttribute(name);
    }
  });
  next.getAttributeNames().forEach((name) => {
    const value = next.getAttribute(name);
    if (value !== null && current.getAttribute(name) !== value) {
      current.setAttribute(name, value);
    }
  });
}

function syncFormControlState(current: Element, next: Element): void {
  const isActive = current === document.activeElement;
  if (current instanceof HTMLInputElement && next instanceof HTMLInputElement) {
    current.defaultValue = next.defaultValue;
    current.defaultChecked = next.defaultChecked;
    if (!isActive) {
      current.value = next.value;
      current.checked = next.checked;
    }
    return;
  }

  if (current instanceof HTMLTextAreaElement && next instanceof HTMLTextAreaElement) {
    current.defaultValue = next.defaultValue;
    if (!isActive) {
      current.value = next.value;
    }
    return;
  }

  if (current instanceof HTMLSelectElement && next instanceof HTMLSelectElement && !isActive) {
    current.value = next.value;
  }
}

function patchNode(current: Node, next: Node): Node {
  if (!nodesCanPatch(current, next)) {
    current.parentNode?.replaceChild(next, current);
    return next;
  }

  if (current.nodeType === Node.TEXT_NODE) {
    if (current.nodeValue !== next.nodeValue) {
      current.nodeValue = next.nodeValue;
    }
    return current;
  }

  if (current instanceof Element && next instanceof Element) {
    patchAttributes(current, next);
    patchChildren(current, next);
    syncFormControlState(current, next);
  }

  return current;
}

function findKeyedPatchCandidate(parent: Node, startIndex: number, next: Node): Node | undefined {
  const nextKey = domPatchKey(next);
  if (!nextKey) {
    return undefined;
  }
  return Array.from(parent.childNodes)
    .slice(startIndex)
    .find((candidate) => domPatchKey(candidate) === nextKey && nodesCanPatch(candidate, next));
}

function patchChildren(parent: Node, nextParent: ParentNode): void {
  const nextChildren = Array.from(nextParent.childNodes);
  let index = 0;
  while (index < nextChildren.length || index < parent.childNodes.length) {
    const currentChild = parent.childNodes[index];
    const nextChild = nextChildren[index];

    if (!nextChild) {
      currentChild?.remove();
      continue;
    }

    if (!currentChild) {
      parent.appendChild(nextChild);
      index += 1;
      continue;
    }

    if (nodesCanPatch(currentChild, nextChild)) {
      patchNode(currentChild, nextChild);
      index += 1;
      continue;
    }

    const keyedCandidate = findKeyedPatchCandidate(parent, index + 1, nextChild);
    if (keyedCandidate) {
      parent.insertBefore(keyedCandidate, currentChild);
      patchNode(keyedCandidate, nextChild);
      index += 1;
      continue;
    }

    parent.replaceChild(nextChild, currentChild);
    index += 1;
  }
}

/** Updates a root element with keyed DOM patching while preserving focus and scroll state. */
export function setElementHtml(root: HTMLElement, html: string, options: SetElementHtmlOptions = {}): void {
  const preserveDom = options.preserveDom ?? true;
  if (!preserveDom || !root.childNodes.length) {
    options.abortEvents?.();
    root.innerHTML = html;
    return;
  }

  const snapshot = captureRenderSnapshot();
  const template = document.createElement('template');
  template.innerHTML = html;
  options.beforePatch?.(template.content);
  patchChildren(root, template.content);
  restoreRenderSnapshot(snapshot);
}
