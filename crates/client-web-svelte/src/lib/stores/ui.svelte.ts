// UI store — cross-cutting UI state: global error, expanded-text keys (for the
// collapsible overview/biography), and the trailer picker menu.
class UiStore {
  error = $state<string | undefined>(undefined);
  expandedTextKeys = $state<Set<string>>(new Set());
  isTrailerMenuOpen = $state(false);
  controlsHelpOpen = $state(false);

  setError(message: string | undefined) {
    this.error = message;
  }

  clearError() {
    this.error = undefined;
  }

  toggleText(key: string) {
    const next = new Set(this.expandedTextKeys);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    this.expandedTextKeys = next;
  }

  isExpanded(key: string): boolean {
    return this.expandedTextKeys.has(key);
  }

  openTrailerMenu() {
    this.isTrailerMenuOpen = true;
  }

  closeTrailerMenu() {
    this.isTrailerMenuOpen = false;
  }

  toggleControlsHelp() {
    this.controlsHelpOpen = !this.controlsHelpOpen;
  }
}

export const ui = new UiStore();
