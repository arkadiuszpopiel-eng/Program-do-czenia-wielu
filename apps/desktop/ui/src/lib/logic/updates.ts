// Logika widoku aktualizacji (bez Svelte — testowana w vitest): komunikat etapu, postęp,
// dostępne akcje, rozpoznanie przywrócenia starszej wersji, filtr licencji „O programie".
import type { LicenseEntry, UpdatesView } from '../api/types-updates';

/** Porównanie wersji semver (`1.2.0` > `1.2.0-beta.1` > `1.1.9`); -1 / 0 / 1. */
export function compareVersions(a: string, b: string): number {
  const split = (v: string) => {
    const [core = '', pre = ''] = v.split('+')[0]?.split(/-(.*)/s) ?? [];
    return { nums: core.split('.').map((n) => Number.parseInt(n, 10) || 0), pre };
  };
  const x = split(a);
  const y = split(b);
  for (let i = 0; i < 3; i++) {
    const d = (x.nums[i] ?? 0) - (y.nums[i] ?? 0);
    if (d !== 0) return Math.sign(d);
  }
  if (x.pre === y.pre) return 0;
  if (!x.pre) return 1;
  if (!y.pre) return -1;
  const xs = x.pre.split('.');
  const ys = y.pre.split('.');
  for (let i = 0; i < Math.max(xs.length, ys.length); i++) {
    const p = xs[i];
    const q = ys[i];
    if (p === undefined) return -1;
    if (q === undefined) return 1;
    const pn = /^\d+$/.test(p);
    const qn = /^\d+$/.test(q);
    if (pn && qn && Number(p) !== Number(q)) return Math.sign(Number(p) - Number(q));
    if (pn !== qn) return pn ? -1 : 1;
    if (p !== q) return p < q ? -1 : 1;
  }
  return 0;
}

/** Przygotowana wersja jest starsza od bieżącej (przywrócenie, nie aktualizacja). */
export function isRollback(view: UpdatesView): boolean {
  return view.ready !== null && compareVersions(view.ready, view.current) < 0;
}

/** Klucz i parametry komunikatu etapu (`updates.phase.*`). */
export function phaseMessage(view: UpdatesView): {
  readonly key: string;
  readonly params: Readonly<Record<string, string>>;
} {
  const version = view.ready ?? view.available?.version ?? view.current;
  if (view.phase === 'ready' && isRollback(view)) {
    return { key: 'updates.phase.readyRollback', params: { version } };
  }
  return { key: `updates.phase.${view.phase}`, params: { version } };
}

/** Procent pobrania (0–100) albo `null`, gdy rozmiar nieznany. */
export function progressPercent(view: UpdatesView): number | null {
  const p = view.progress;
  if (!p || !p.total) return null;
  return Math.max(0, Math.min(100, Math.floor((p.downloaded / p.total) * 100)));
}

/** Akcje dostępne na stronie „Aktualizacje". */
export function updateActions(view: UpdatesView): {
  readonly check: boolean;
  readonly download: boolean;
  readonly resume: boolean;
  readonly cancel: boolean;
  readonly restart: boolean;
  readonly rollback: boolean;
} {
  const busy = ['checking', 'downloading', 'verifying', 'installing'].includes(view.phase);
  const partial = (view.progress?.downloaded ?? 0) > 0;
  const resume =
    view.available !== null && partial && (view.phase === 'failed' || view.phase === 'available');
  return {
    check: view.phase !== 'disabled' && !busy,
    download: view.phase === 'available' && !partial,
    resume,
    cancel: view.phase === 'downloading',
    restart: view.phase === 'ready' && view.ready !== null,
    rollback: !busy && view.previous !== null && view.ready === null,
  };
}

/** Filtr listy licencji (nazwa albo licencja, bez rozróżniania wielkości liter). */
export function filterLicenses(
  entries: readonly LicenseEntry[],
  query: string,
): readonly LicenseEntry[] {
  const q = query.trim().toLowerCase();
  if (!q) return entries;
  return entries.filter(
    (e) => e.name.toLowerCase().includes(q) || e.license.toLowerCase().includes(q),
  );
}
