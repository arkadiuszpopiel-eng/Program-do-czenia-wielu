// Przebiegi okna głównego wydzielone z `AppState` (limit 400 linii na plik): start, odświeżenie
// po zgubionych zdarzeniach, otwieranie i sprzątanie sesji, „Wstecz / Dalej”. Klasa woła je
// jednym wierszem; tu mają dostęp do jej pól `@internal`.
import type { SessionSummary } from '../api/types';
import type { AppBootstrap } from '../api/types-system';
import { watchBoot } from './boot.svelte';
import { errorText } from '../api/command-error';
import type { AppState } from './app.svelte';
import { attempt } from './attempt';
import { logFailure } from './background';

export async function start(app: AppState): Promise<void> {
  app.view = 'loading';
  app.fatal = null;
  app.bootStep = '';
  let boot: AppBootstrap;
  let list: readonly SessionSummary[];
  let status: Promise<unknown> = Promise.resolve();
  try {
    boot = await app.client.app.bootstrap();
    app.appVersion = boot.app_version;
    app.settings = { ...boot.settings };
    app.shortcutOverrides = { ...boot.shortcut_overrides };
    app.layout.load(boot.layout);
    await app.i18n.setLocale(boot.locale);
    app.bootStep = app.i18n.t('boot.sessions');
    app.unsubscribe?.();
    app.unsubscribe = app.client.subscribe(
      (batch) => app.batcher.push(...batch),
      () => void app.resync(),
    );
    status = app.client.system.status().then(
      (value) => (app.system = value),
      (error: unknown) => app.loadFailed(error),
    );
    list = await app.client.sessions.list();
  } catch (error) {
    app.fatal = errorText(error);
    app.view = 'error';
    return;
  }
  app.sessions.list = [...list];
  void app.client.voice.status().then((v) => app.voice.applyStatus(v), logFailure('voice'));
  void app.client.gui.status().then((g) => app.work.applyGui(g), logFailure('gui'));
  void app.client.cameras.state().then((c) => (app.cameras = c), logFailure('cameras'));
  void app.updates.load(app.client.updates);
  void app.broker.load(app.client.broker);
  void watchBoot(app);
  const first = boot.active_session_id ?? list.find((s) => !s.archived)?.id ?? null;
  if (first) await app.openSession(first);
  else await app.refreshCosts().catch((error: unknown) => app.loadFailed(error));
  await status;
  app.view = boot.onboarding_done ? 'chat' : 'onboarding';
}

export async function resync(app: AppState): Promise<void> {
  app.batcher.flushNow();
  const active = app.activeId;
  for (const id of Object.keys(app.cache)) if (id !== active) delete app.cache[id];
  app.cacheOrder = app.cacheOrder.filter((id) => id === active);
  const failed = (
    await Promise.allSettled([
      app.client.sessions.list().then((list) => (app.sessions.list = [...list])),
      app.client.system.status().then((value) => (app.system = value)),
      app.client.voice.status().then((v) => app.voice.applyStatus(v)),
      app.client.cameras.state().then((c) => (app.cameras = c)),
      app.conversation?.load(),
      app.refreshCosts(),
      active ? app.loadAgents(active) : null,
    ])
  ).find((r) => r.status === 'rejected');
  if (failed) app.loadFailed(failed.reason);
}

export async function forgetSession(app: AppState, id: string): Promise<void> {
  app.sessions.remove(id);
  delete app.cache[id];
  app.cacheOrder = app.cacheOrder.filter((x) => x !== id);
  delete app.agents[id];
  delete app.activity[id];
  if (app.activeId !== id) return;
  const next = app.sessions.list.find((s) => !s.archived);
  if (next) await app.openSession(next.id);
  else {
    app.sessions.activeId = null;
    app.conversation = null;
  }
}

export async function openSession(app: AppState, id: string): Promise<void> {
  app.batcher.flushNow();
  app.announcer.reset();
  const conv = app.conversationFor(id);
  app.sessions.activeId = id;
  app.sessions.touch(id);
  app.conversation = conv;
  app.findOpen = false;
  app.timelineTurn = null;
  const loads: Promise<unknown>[] = [app.refreshCosts(), app.loadAgents(id)];
  if (!conv.loaded || conv.loadError) loads.push(conv.load());
  if (app.sessions.drafts[id] === undefined) {
    loads.push(
      app.client.sessions.getDraft(id).then((d) => {
        // Tekst wpisany, zanim przyszła odpowiedź, jest nowszy niż zapisany szkic.
        if (app.sessions.drafts[id] === undefined) app.sessions.drafts[id] = d;
      }),
    );
  }
  const failed = (await Promise.allSettled(loads)).find((r) => r.status === 'rejected');
  // Szybkie przełączanie: spóźnione A nie może nadpisać aktywnej B w rdzeniu.
  if (app.activeId !== id) return;
  if (failed) app.loadFailed(failed.reason);
  app.client.app.setActiveSession(id).catch(logFailure('setActiveSession'));
  if (app.sessions.active?.unread) app.client.sessions.markRead(id).catch(logFailure('markRead'));
}

export async function focusSession(app: AppState, id: string): Promise<void> {
  if (!app.sessions.list.some((s) => s.id === id)) {
    try {
      app.sessions.list = [...(await app.client.sessions.list())];
    } catch (error) {
      app.loadFailed(error);
      return;
    }
  }
  if (app.view === 'settings') app.view = 'chat';
  await app.openSession(id);
}

export async function navigate(app: AppState, direction: -1 | 1): Promise<void> {
  if (app.navBusy || app.place() === null) return;
  // Sesje usunięte w międzyczasie nie są celem powrotu.
  app.nav.prune((p) => p.session === null || app.sessions.list.some((s) => s.id === p.session));
  const target = direction < 0 ? app.nav.back() : app.nav.forward();
  if (!target) {
    app.announcement = app.i18n.t(direction < 0 ? 'nav.noBack' : 'nav.noForward');
    return;
  }
  app.navBusy = true;
  try {
    if (target.session && target.session !== app.activeId) await app.openSession(target.session);
    if (target.view === 'settings') app.openSettings(target.page ?? undefined);
    else app.closeSettings();
  } finally {
    app.navBusy = false;
  }
  app.announcement = app.i18n.t(direction < 0 ? 'nav.wentBack' : 'nav.wentForward');
}

export async function sessionRemoved(app: AppState, id: string): Promise<void> {
  // Otwarcie następnej sesji opróżnia kolejkę zdarzeń, w której może już czekać zdarzenie
  // o tym samym usunięciu — drugie wejście otwierało sesję jeszcze raz.
  if (app.removing.includes(id)) return;
  app.removing.push(id);
  try {
    await app.forgetSession(id);
  } finally {
    app.removing = app.removing.filter((x) => x !== id);
  }
}

export async function undoStep(app: AppState, token: string, label: string): Promise<boolean> {
  const ok = await attempt(app.toasts, () => app.client.turns.undoStep(token));
  if (ok) {
    app.runs.markUndone(token);
    app.toasts.show({ kind: 'success', message: app.i18n.t('conv.undone', { label }) });
  }
  return ok;
}
