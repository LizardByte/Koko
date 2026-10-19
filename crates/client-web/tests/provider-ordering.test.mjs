import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { bindProviderOrdering, syncProviderDependencyOptions } = await server.ssrLoadModule('/src/app/providerOrdering.ts');

function fixture() {
  const options = [];
  const list = {
    querySelectorAll: () => options.filter((option) => !option.hidden),
    insertBefore(option, reference) {
      options.splice(options.indexOf(option), 1);
      options.splice(reference ? options.indexOf(reference) : options.length, 0, option);
    },
  };
  for (const [id, role, dependencies] of [['musicbrainz', 'primary', ''], ['local_nfo', 'primary', ''], ['fanart', 'secondary', 'musicbrainz']]) {
    const input = new EventTarget();
    Object.assign(input, { value: id, checked: id !== 'local_nfo', disabled: false, closest: () => list });
    const label = { textContent: '' };
    const option = {
      dataset: { providerRole: role, extendsProviderIds: dependencies }, input, label, hidden: false,
      closest: () => list, classList: { toggle() {} },
      get nextSibling() { return options[options.indexOf(option) + 1] ?? null; },
      querySelector: (selector) => ({
        'input[type="checkbox"]': input, '.provider-option-main .muted': label,
        '[data-provider-move="up"]': option.up, '[data-provider-move="down"]': option.down,
      })[selector],
    };
    for (const direction of ['up', 'down']) {
      const button = new EventTarget();
      Object.assign(button, { dataset: { providerMove: direction }, closest: () => option });
      option[direction] = button;
    }
    options.push(option);
  }
  const root = { querySelectorAll: (selector) => selector === '[data-provider-move]'
    ? options.flatMap((option) => [option.up, option.down]) : options.map((option) => option.input) };
  bindProviderOrdering(root);
  syncProviderDependencyOptions(list);
  return { options, list, primary: options[0], local: options[1], fanart: options[2] };
}

test('clicking Fanart.tv arrows moves it across primary providers and updates priorities', () => {
  const { options, primary, local, fanart } = fixture();
  fanart.up.dispatchEvent(new Event('click'));
  fanart.up.dispatchEvent(new Event('click'));
  assert.deepEqual(options, [fanart, primary, local]);
  assert.equal(fanart.label.textContent, 'Priority 1 · Secondary');
  assert.equal(fanart.up.disabled, true);
  assert.equal(primary.label.textContent, 'Priority 2');
  assert.equal(local.down.disabled, true);
  assert.equal(fanart.input.checked, true);
  assert.equal(fanart.input.disabled, false);
  fanart.down.dispatchEvent(new Event('click'));
  assert.deepEqual(options, [primary, fanart, local]);
  assert.equal(fanart.label.textContent, 'Priority 2 · Secondary');
});

test('secondary dependencies still apply above their primary and hidden options are skipped', () => {
  const { options, primary, local, fanart, list } = fixture();
  local.hidden = true;
  fanart.up.dispatchEvent(new Event('click'));
  assert.equal(options[0], fanart);
  assert.equal(fanart.label.textContent, 'Priority 1 · Secondary');
  primary.input.checked = false;
  primary.input.dispatchEvent(new Event('change'));
  assert.equal(fanart.input.disabled, true);
  assert.equal(fanart.input.checked, false);
  assert.equal(fanart.label.textContent, 'Requires primary provider');
  primary.input.checked = true;
  primary.input.dispatchEvent(new Event('change'));
  assert.equal(fanart.input.disabled, false);
  primary.hidden = true;
  syncProviderDependencyOptions(list);
  assert.equal(fanart.input.disabled, true);
});
