/** Delay pointer previews; keyboard and controller focus update immediately. */
export function bindHoverPreview(element: HTMLElement, activate: () => void, delay = 220): void {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const cancel = () => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
  };
  element.addEventListener('mouseenter', () => {
    cancel();
    timer = setTimeout(() => {
      timer = undefined;
      if (element.isConnected && element.matches(':hover')) activate();
    }, delay);
  });
  element.addEventListener('mouseleave', cancel);
  element.addEventListener('focus', () => { cancel(); activate(); });
  element.addEventListener('click', cancel);
}
