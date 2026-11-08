// Atrapa: pakiety 1–6. Skład (warianty silników), wymagania, dopasowanie, zalecenie i uwagi
// o jakości dla maszyny atrapy (`seedDevice`: AMD Radeon 780M 4 GB, 32 GB RAM, 8 rdzeni) pochodzą
// z Rust — `bundles.json` zapisuje `crates/app-models/tests/bundles.rs` (`ALFA_UPDATE_FIXTURES=1`),
// nie edytuj go ręcznie. Stan, rozmiary i liczniki atrapa liczy ze swoich pozycji tak jak
// `app_models::bundles::view` (pozycje spoza katalogu pomija).
import type {
  BundleItemView,
  BundleState,
  ModelBundle,
  ModelItem,
  ModelItemState,
} from '../types-models';
import GENERATED from './bundles.json';

const DEFS = GENERATED as readonly ModelBundle[];

const BUSY: readonly ModelItemState[] = ['queued', 'downloading', 'installing'];
const PENDING: readonly ModelItemState[] = [
  'missing',
  'paused',
  'failed',
  'corrupt',
  'queued',
  'downloading',
];

const isInstalled = (s: ModelItemState) => s === 'installed' || s === 'external';

/** Stan pakietu ze stanów pozycji (kolejność jak `app_models::bundles::state_of`). */
export function bundleState(items: readonly BundleItemView[]): BundleState {
  const any = (f: (s: ModelItemState) => boolean) => items.some((i) => f(i.state));
  if (any((s) => BUSY.includes(s))) return 'downloading';
  if (any((s) => s === 'corrupt' || s === 'failed')) return 'corrupt';
  if (any((s) => s === 'needs_trust')) return 'needs_trust';
  if (items.length > 0 && items.every((i) => isInstalled(i.state))) return 'installed';
  if (any((s) => isInstalled(s) || s === 'paused')) return 'partial';
  return 'not_installed';
}

/** Bajty do pobrania pozycji (bez pobranej części bieżącego pliku). */
function toDownload(item: ModelItem): number {
  if (!item.downloadable || !PENDING.includes(item.state)) return 0;
  return Math.max(0, item.size_bytes - (item.progress?.done ?? 0));
}

function view(def: ModelBundle, items: ReadonlyMap<string, ModelItem>): ModelBundle {
  const found = def.items.flatMap((row) => {
    const item = items.get(row.id);
    return item ? [{ item, fallback: row.fallback }] : [];
  });
  const rows: BundleItemView[] = found.map(({ item, fallback }) => ({
    id: item.id,
    name: item.name,
    kind: item.kind,
    state: item.state,
    size_bytes: item.size_bytes,
    downloadable: item.downloadable,
    fallback,
  }));
  return {
    ...def,
    items: rows,
    size_bytes: found.reduce((sum, { item }) => sum + item.size_bytes, 0),
    missing_bytes: found.reduce((sum, { item }) => sum + toDownload(item), 0),
    installed: rows.filter((r) => isInstalled(r.state)).length,
    total: rows.length,
    state: bundleState(rows),
  };
}

/** Pakiety od 6 do 1 ze stanem pozycji atrapy. */
export function bundleViews(items: ReadonlyMap<string, ModelItem>): ModelBundle[] {
  return DEFS.map((def) => view(def, items));
}

/** Pakiet po identyfikatorze (błąd jak w rdzeniu, gdy nie istnieje). */
export function bundleView(bundleId: string, items: ReadonlyMap<string, ModelItem>): ModelBundle {
  const def = DEFS.find((d) => d.id === bundleId);
  if (!def) throw new Error(`Nie ma pakietu „${bundleId}” — wybierz jeden z pakietów 1–6 z listy.`);
  return view(def, items);
}

/** Identyfikatory pozycji pakietu (warianty silników dla maszyny atrapy). */
export function bundleMembers(bundleId: string): readonly string[] {
  return DEFS.find((d) => d.id === bundleId)?.items.map((i) => i.id) ?? [];
}
