/** Library provider order controls preference; secondary providers still need a matching primary. */
function visibleProviderOptions(list: HTMLElement): HTMLElement[] {
  return Array.from(list.querySelectorAll<HTMLElement>('.metadata-provider-option:not(.is-hidden)'));
}

export function syncProviderDependencyOptions(list: HTMLElement): void {
  const options = visibleProviderOptions(list);
  const selectedPrimaryIds = new Set(
    options.filter((option) => option.dataset.providerRole === 'primary')
      .flatMap((option) => {
        const input = option.querySelector<HTMLInputElement>('input[type="checkbox"]');
        return input?.checked ? [input.value] : [];
      }),
  );
  options.forEach((option, index) => {
    const up = option.querySelector<HTMLButtonElement>('[data-provider-move="up"]');
    const down = option.querySelector<HTMLButtonElement>('[data-provider-move="down"]');
    if (up) up.disabled = index === 0;
    if (down) down.disabled = index === options.length - 1;
    const input = option.querySelector<HTMLInputElement>('input[type="checkbox"]');
    const label = option.querySelector<HTMLElement>('.provider-option-main .muted');
    const secondary = option.dataset.providerRole === 'secondary';
    const dependencies = (option.dataset.extendsProviderIds ?? '').split(',').map((id) => id.trim()).filter(Boolean);
    const available = !secondary || dependencies.some((id) => selectedPrimaryIds.has(id));
    if (input && secondary) {
      input.disabled = !available;
      if (!available) input.checked = false;
    }
    option.classList.toggle('is-disabled', !available);
    if (label) label.textContent = available
      ? `Priority ${index + 1}${secondary ? ' · Secondary' : ''}`
      : 'Requires primary provider';
  });
}

export function bindProviderOrdering(root: ParentNode = document): void {
  root.querySelectorAll<HTMLButtonElement>('[data-provider-move]').forEach((button) => {
    button.addEventListener('click', () => {
      const option = button.closest<HTMLElement>('.metadata-provider-option');
      const list = option?.closest<HTMLElement>('.metadata-provider-list');
      if (!option || !list) return;
      const options = visibleProviderOptions(list);
      const index = options.indexOf(option);
      if (button.dataset.providerMove === 'up' && index > 0) {
        list.insertBefore(option, options[index - 1]);
      }
      if (button.dataset.providerMove === 'down' && index >= 0 && index < options.length - 1) {
        list.insertBefore(option, options[index + 1].nextSibling);
      }
      syncProviderDependencyOptions(list);
    });
  });
  root.querySelectorAll<HTMLInputElement>('.metadata-provider-list input[type="checkbox"]').forEach((input) => {
    input.addEventListener('change', () => {
      const list = input.closest<HTMLElement>('.metadata-provider-list');
      if (list) syncProviderDependencyOptions(list);
    });
  });
}
