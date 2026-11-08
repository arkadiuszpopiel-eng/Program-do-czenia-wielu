// Zmiana trybu pracy — wspólna dla menu „Tryb pracy” i skrótu `mode.cycle` (Alt+M; odpowiednik
// Shift+Tab z Claude Code). Zakres: otwarta rozmowa w trybie Plan, przy wyjściu z Planu i przy kłódce
// otwartej „do końca rozmowy” (Z.7); w pozostałych przypadkach — poziom globalny.
import type { AutonomyLevel } from '../api/types';
import { requestMode } from '../logic/work-mode';
import type { AppState } from './app.svelte';
import { showError } from './attempt';

/** Kolejność skrótu jak w Claude Code: zwykły → akceptuj edycje → automatycznie → plan → zwykły.
 * Pełny dostęp (L4) tylko z menu — skrót nigdy go nie włącza. */
export const MODE_CYCLE: readonly AutonomyLevel[] = ['L1', 'L2', 'L3', 'L0'];

/** Stan potrzebny do zmiany trybu (odczytany w chwili zmiany). */
export interface ModeView {
  /** Tryb pokazany w menu jako bieżący. */
  readonly current: AutonomyLevel;
  /** Zmiana dotyczy tej rozmowy (Plan, wyjście z Planu, „do końca rozmowy”). */
  readonly sessionId: string | null;
  readonly planning: boolean;
  /** Kłódka otwarta dla tej rozmowy. */
  readonly here: boolean;
  /** Każda zmiana dotyczy tej rozmowy (skrót przy rozmowie z własnym poziomem). */
  readonly scoped?: boolean;
}

export function modeView(app: AppState, scope: 'global' | 'session' = 'global'): ModeView {
  const session = app.sessions.active;
  const planning = session?.autonomy === 'L0';
  const here = app.autonomy.openHere(session?.id);
  const scoped = planning || here || (scope === 'session' && Boolean(session));
  const current: AutonomyLevel =
    scoped && session ? session.autonomy : (app.autonomy.global ?? session?.autonomy ?? 'L3');
  return { current, sessionId: session?.id ?? null, planning, here, scoped };
}

/**
 * Widok skrótu: tryb z żetonu w pasku pisania (poziom tej rozmowy). Rozmowa z własnym poziomem
 * (np. po wyjściu z Planu) zmienia się sama — skrót nie przestawia wtedy po cichu trybu globalnego.
 */
export function cycleView(app: AppState): ModeView {
  const view = modeView(app);
  const session = app.sessions.active;
  if (!session) return view;
  const own = app.autonomy.global !== null && session.autonomy !== app.autonomy.global;
  return { ...view, current: session.autonomy, scoped: view.planning || view.here || own };
}

/** Tryb za kłódką rodzica (profil Dziecko, plan 50 §4.4): Plan i Ręcznie zostają otwarte. */
export function behindLock(app: AppState, view: ModeView, level: AutonomyLevel): boolean {
  return app.autonomy.locked && !view.here && level > 'L1';
}

/** Następny tryb skrótu; przy kłódce tylko Plan ↔ Ręcznie. */
export function nextMode(app: AppState, view: ModeView): AutonomyLevel {
  const cycle = MODE_CYCLE.filter((l) => !behindLock(app, view, l));
  const at = cycle.indexOf(view.current);
  return cycle[(at + 1) % cycle.length] ?? 'L1';
}

/** Poziom w zakresie zmiany: rozmowy albo globalny. */
async function readLevel(app: AppState, scope: string | null): Promise<AutonomyLevel | null> {
  if (!scope) return app.autonomy.load(app.client.permissions);
  const state = await app.client.permissions.get(scope);
  return state.session ?? state.global;
}

/** Komunikat po zmianie — sukces dopiero po odczycie z Brokera, że poziom obowiązuje. */
async function report(
  app: AppState,
  event: 'applied' | 'requested' | 'raised',
  level: AutonomyLevel,
  scope: string | null,
): Promise<void> {
  const { t, tk } = app.i18n;
  const mode = tk(`mode.${level}.name`);
  if (event === 'requested') {
    app.toasts.show({ kind: 'info', message: t('mode.requested', { mode }) });
    return;
  }
  const now = event === 'raised' ? level : await readLevel(app, scope);
  if (now === level || now === null) {
    app.toasts.show({ kind: 'success', message: t(`mode.${event}`, { mode }) });
  } else {
    const current = tk(`mode.${now}.name`);
    app.toasts.show({ kind: 'error', message: t('mode.notApplied', { mode, current }) });
  }
}

/** Zmienia tryb; podniesienie czeka na okno Brokera, obniżenie działa od razu. */
export async function switchMode(
  app: AppState,
  view: ModeView,
  level: AutonomyLevel,
): Promise<void> {
  if (behindLock(app, view, level)) {
    app.toasts.show({ kind: 'info', message: app.i18n.t('child.lockedNote') });
    return;
  }
  if (level === view.current) return;
  const scope =
    view.sessionId && (level === 'L0' || view.planning || view.here || view.scoped)
      ? view.sessionId
      : null;
  try {
    await requestMode(level, {
      request: (l) => app.client.permissions.requestLevel(l, scope),
      readGlobal: () => readLevel(app, scope),
      notify: (event, l) => void report(app, event, l, scope),
    });
  } catch (error) {
    showError(app.toasts, error);
  }
}
