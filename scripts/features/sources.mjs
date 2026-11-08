// Źródła katalogu funkcji (docs/plan/45-testy-ai.md §2): każda funkcja programu wyliczona
// z kodu, nie z pamięci — nowa funkcja pojawia się w katalogu sama, gdy trafi do źródła.
//
//   polecenie   — polecenia rdzenia (`packages/schemas/ipc.v1.json`, generowane z kontraktu łącza)
//   akcja       — polecenia palety i skróty (`shortcut-registry.ts` + `EXTRA_COMMANDS`)
//   ustawienie  — klucze ustawień (`crates/app-core/data/settings-pages.json`)
//   strona      — strony Ustawień (ten sam plik)
//   narzedzie   — narzędzia agentek (`docs/features/narzedzia.txt`, pilnowany testem
//                 `crates/app-core/tests/feature_catalog.rs` na rdzeniu złożonym jak w aplikacji)
//   zdarzenie   — zdarzenia rdzenia dla okien (`AlfaEvent` w `packages/schemas/ipc.v1.json`)
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

const json = (root, rel) => JSON.parse(readFileSync(join(root, rel), 'utf8'));

function ipcCommands(root) {
  const schema = json(root, 'packages/schemas/ipc.v1.json');
  return Object.keys(schema.commands).map((id) => ({ kind: 'polecenie', id }));
}

function events(root) {
  const schema = json(root, 'packages/schemas/ipc.v1.json');
  const ids = new Set();
  for (const v of schema.$defs.AlfaEvent?.oneOf ?? []) {
    const value = v.properties?.type?.const;
    if (typeof value === 'string') ids.add(value);
  }
  return [...ids].map((id) => ({ kind: 'zdarzenie', id }));
}

async function actions(root) {
  const ui = join(root, 'apps/desktop/ui/src/lib');
  const registry = await import(pathToFileURL(join(ui, 'logic/shortcut-registry.ts')).href);
  const local = registry.COMPOSER_LOCAL;
  const ids = registry.SHORTCUTS.map((d) => d.id).filter((id) => !/^session\.goto[2-9]$/.test(id));
  // `EXTRA_COMMANDS` leży w module stanu (Svelte) — czytamy samą listę z tekstu.
  const commands = readFileSync(join(ui, 'state/commands.ts'), 'utf8');
  const block = /EXTRA_COMMANDS = \[([\s\S]*?)\]/.exec(commands)?.[1] ?? '';
  const extra = [...block.matchAll(/'([^']+)'/g)].map((m) => m[1]);
  const scope = new Map(registry.SHORTCUTS.map((d) => [d.id, d.scope]));
  return [...ids, ...extra].map((id) => {
    const note = local.has(id)
      ? 'klawisz pola pisania'
      : scope.get(id) === 'global'
        ? 'skrót globalny'
        : undefined;
    // `sweep` — grupa przeglądu ogólnego: w palecie są polecenia aplikacji i akcje dodatkowe.
    return { kind: 'akcja', id, note, sweep: note ? undefined : 'paleta' };
  });
}

function settings(root) {
  const pages = json(root, 'crates/app-core/data/settings-pages.json');
  const out = [];
  for (const page of pages) {
    out.push({ kind: 'strona', id: page.id, label: page.label.pl, soon: page.wave > 1 });
    for (const s of page.settings ?? []) {
      out.push({ kind: 'ustawienie', id: s.key, label: s.label?.pl, page: page.id });
    }
  }
  return out;
}

function tools(root) {
  const file = join(root, 'docs/features/narzedzia.txt');
  if (!existsSync(file)) return [];
  return readFileSync(file, 'utf8')
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean)
    .map((id) => ({ kind: 'narzedzie', id }));
}

/** Wszystkie funkcje programu: `{ kind, id, label?, note?, soon?, sweep? }`. */
export async function collectFeatures(root) {
  return [
    ...ipcCommands(root),
    ...(await actions(root)),
    ...settings(root),
    ...tools(root),
    ...events(root),
  ];
}

export const KINDS = {
  polecenie: 'Polecenia rdzenia',
  akcja: 'Polecenia palety i skróty',
  strona: 'Strony Ustawień',
  ustawienie: 'Ustawienia',
  narzedzie: 'Narzędzia agentek',
  zdarzenie: 'Zdarzenia rdzenia',
};
