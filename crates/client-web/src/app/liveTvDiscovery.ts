/** Presents discovered TV servers and local tuners as selectable connection cards. */
import * as api from '../liveTvApi';
import { escapeHtml as escape } from './format';

interface Choice { name: string; address: string; details: string; capacity?: number }
interface DiscoveryForm {
  form: HTMLFormElement; input: HTMLInputElement; results: HTMLElement;
  status: HTMLElement; kind: string; signal: AbortSignal; suggestedName?: string; viewKey?: string; choices?: Choice[];
}

const forms = new WeakMap<HTMLFormElement, DiscoveryForm>();

async function findChoices(kind: string): Promise<Choice[]> {
  if (kind === 'native') {
    return (await api.discoverTvTuners()).map(device => ({
      name: device.name, address: device.id, details: `${device.info || 'Local tuner'} · 1 simultaneous stream`,
    }));
  }
  const serverLabel = kind === 'nextpvr' ? 'NextPVR server' : 'Tvheadend server';
  return (await api.discoverTvSources(kind)).map(source => ({
    name: source.name, address: source.url, capacity: source.capacity,
    details: kind === 'hdhomerun' ? `Tuners: ${source.capacity}` : serverLabel,
  }));
}

function useChoice(context: DiscoveryForm, choice: Choice): void {
  const name = context.form.querySelector<HTMLInputElement>('[name="name"]');
  const capacity = context.form.querySelector<HTMLInputElement>('[name="capacity"]');
  context.input.value = choice.address;
  if (name && (!name.value.trim() || name.value === context.suggestedName)) name.value = choice.name;
  if (capacity && choice.capacity !== undefined) capacity.value = String(choice.capacity);
  context.suggestedName = choice.name;
  context.status.textContent = `${choice.name} selected. You can review its connection settings below.`;
}

function showChoices(context: DiscoveryForm, choices: Choice[]): void {
  context.choices = choices;
  context.results.innerHTML = choices.map(choice => `<label class="live-tv-discovery-card">
    <input type="radio" name="tv_discovery_choice" value="${escape(choice.address)}" ${choice.address === context.input.value ? 'checked' : ''} />
    <span class="live-tv-discovery-card-content"><span class="live-tv-discovery-card-heading"><strong>${escape(choice.name)}</strong><span class="live-tv-discovery-selected" aria-hidden="true">Selected</span></span>
      <span class="live-tv-discovery-card-address">${escape(choice.address)}</span><span class="live-tv-discovery-card-details">${escape(choice.details)}</span></span></label>`).join('');
  context.results.hidden = choices.length === 0;
}

async function search(context: DiscoveryForm, button: HTMLButtonElement): Promise<void> {
  button.disabled = true;
  context.results.hidden = true;
  context.results.replaceChildren();
  context.status.textContent = 'Searching for devices…';
  try {
    const choices = await findChoices(context.kind);
    if (context.signal.aborted) return;
    showChoices(context, choices);
    const noun = context.kind === 'native' ? 'tuner' : 'source';
    const foundLabel = choices.length === 1 ? noun : `${noun}s`;
    context.status.textContent = choices.length
      ? `${choices.length} ${foundLabel} found. Select a card to use that device.`
      : `No ${noun}s found. Check the device and connection, or enter its location manually below.`;
  } catch (error) {
    if (!context.signal.aborted) context.status.textContent = error instanceof Error ? error.message : 'Discovery failed. Enter the location manually.';
  } finally {
    button.disabled = false;
  }
}

function bindForm(form: HTMLFormElement, signal: AbortSignal): void {
  const kind = form.dataset.tvSourceKind ?? '';
  const button = form.querySelector<HTMLButtonElement>('[data-tv-discover-source], [data-tv-discover-tuners]');
  const results = form.querySelector<HTMLElement>('[data-tv-discovery-results]');
  const status = form.querySelector<HTMLElement>('[data-tv-discovery-status], [data-tv-tuner-status]');
  const input = form.querySelector<HTMLInputElement>(kind === 'native' ? '[name="native_device"]' : '[name="url"]');
  if (!button || !results || !status || !input) return;
  // Refresh the signal on a retained form so pending discovery can populate newly bound controls.
  const fields = { form, input, results, status, kind, signal, viewKey: form.dataset.tvFormKey };
  const previous = forms.get(form);
  const context: DiscoveryForm = previous?.kind === kind && previous.viewKey === fields.viewKey ? Object.assign(previous, fields) : fields;
  forms.set(form, context);
  results.addEventListener('change', event => {
    const radio = (event.target as Element | null)?.closest<HTMLInputElement>('input[type="radio"]');
    const choice = radio?.checked ? context.choices?.find(item => item.address === radio.value) : undefined;
    if (choice) useChoice(context, choice);
  }, { signal });
  button.addEventListener('click', () => { void search(context, button); }, { signal });
  input.addEventListener('input', () => {
    const checked = results.querySelector<HTMLInputElement>('input:checked');
    if (!checked) return;
    checked.checked = false;
    status.textContent = 'Using the location entered manually below. Select a card to use a discovered device again.';
  }, { signal });
}

/** Bind card selection and discovery while retaining editable manual connection fields. */
export function bindTvDiscovery(signal: AbortSignal): void {
  document.querySelectorAll<HTMLFormElement>('[data-tv-source-kind]').forEach(form => bindForm(form, signal));
}
