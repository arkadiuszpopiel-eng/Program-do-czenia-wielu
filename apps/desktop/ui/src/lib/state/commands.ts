// Komendy okna głównego: jedno miejsce dla skrótów klawiszowych i palety poleceń.
// Skróty globalne (Szybkie pytanie, STOP WSZYSTKIEGO) obsługuje rdzeń — nie ma ich tutaj.
import { errorText } from '../api/command-error';
import type { PanelId } from '../api/types-system';
import { stepZoom } from '../logic/layout';
import { COMPOSER_LOCAL } from '../logic/shortcut-registry';
import { RESERVED, allowedInInput, chordFromEvent } from '../logic/shortcuts';
import type { AppState } from './app.svelte';
import { attempt } from './attempt';
import { exportConversation } from './exports';

const PANEL_COMMANDS: Readonly<Record<string, PanelId>> = {
  'panel.agents': 'agents',
  'panel.timeline': 'timeline',
  'panel.files': 'files',
  'panel.memory': 'memory',
  'panel.screen': 'screen',
  'panel.voice': 'voice',
  'panel.tasks': 'tasks',
};

/** Dodatkowe polecenia palety (bez domyślnego skrótu). */
export const EXTRA_COMMANDS = [
  'action.theme',
  'action.language',
  'action.column',
  'action.transfer',
  'action.accounts',
  'action.autonomy',
  'action.cast',
  'action.stopGeneration',
  'action.onboarding',
  'action.exportMarkdown',
  'action.exportHtml',
] as const;

const ONBOARDING_ALLOWED = new Set([
  'view.zoomIn',
  'view.zoomOut',
  'view.zoomReset',
  'cheatsheet.open',
]);

function focusById(id: string): void {
  requestAnimationFrame(() => document.getElementById(id)?.focus());
}

async function open(app: AppState, id: string | undefined): Promise<void> {
  if (id) await app.openSession(id);
}

export function runCommand(app: AppState, id: string): void {
  const sid = app.activeId;
  const tab = PANEL_COMMANDS[id];
  if (tab) {
    app.openPanel(tab);
    return;
  }
  const goto = /^session\.goto(\d)$/.exec(id);
  if (goto?.[1]) {
    void open(app, app.sessions.tab(Number(goto[1])));
    return;
  }
  switch (id) {
    case 'session.new':
      void app.newSession();
      break;
    case 'session.switch':
      // Pola ustawiane osobno: zmiana samego `open` nie przelicza listy pozycji palety.
      app.palette.mode = 'sessions';
      app.palette.open = true;
      break;
    case 'session.close':
      if (sid) void open(app, app.sessions.close(sid));
      break;
    case 'session.reopen':
      void open(app, app.sessions.reopen());
      break;
    case 'session.next':
      void open(app, app.sessions.cycle(1));
      break;
    case 'session.prev':
      void open(app, app.sessions.cycle(-1));
      break;
    case 'session.rename':
      if (sid) app.sessions.renamingId = sid;
      break;
    case 'search.everywhere':
      app.view = 'chat';
      app.layout.setLeftCollapsed(false);
      app.layout.update(sid, { left_open: true });
      focusById('alfa-sessions-search');
      break;
    case 'search.conversation':
      app.findOpen = true;
      focusById('alfa-find-input');
      break;
    case 'palette.open':
      app.palette.mode = 'all';
      app.palette.open = !app.palette.open;
      break;
    case 'cheatsheet.open':
      app.cheatsheetOpen = !app.cheatsheetOpen;
      break;
    case 'settings.open':
      if (app.view === 'settings') app.closeSettings();
      else app.openSettings();
      break;
    case 'view.focus':
      app.layout.focus = !app.layout.focus;
      break;
    case 'view.zoomIn':
    case 'view.zoomOut':
    case 'view.zoomReset': {
      const dir = id === 'view.zoomIn' ? 1 : id === 'view.zoomOut' ? -1 : 0;
      const zoom = stepZoom(app.num('ui.zoom', 100), dir);
      void app.setSetting('ui.zoom', zoom);
      app.announcement = app.i18n.t('zoom.level', { pct: `${zoom}%` });
      break;
    }
    case 'panel.sessions':
      if (app.layout.leftCollapsed) app.layout.setLeftCollapsed(false);
      else app.layout.toggleLeft(sid);
      break;
    case 'panel.right':
      app.layout.toggleRight(sid);
      break;
    case 'voice.mic': {
      const on = app.micState === 'off' || app.micState === 'muted';
      const before = app.micState;
      app.micState = on ? 'listening' : 'off';
      app.client.voice.setMicEnabled(on).catch((error: unknown) => {
        app.micState = before;
        app.toasts.show({ kind: 'warning', message: errorText(error) });
      });
      break;
    }
    case 'action.theme': {
      const theme = app.str('ui.theme', 'auto');
      const dark =
        theme === 'dark' ||
        (theme === 'auto' && matchMedia('(prefers-color-scheme: dark)').matches);
      void app.setSetting('ui.theme', dark ? 'light' : 'dark');
      break;
    }
    case 'action.language':
      void app.setSetting('ui.locale', app.i18n.locale === 'pl' ? 'en' : 'pl');
      break;
    case 'action.column':
      void app.setSetting(
        'ui.column',
        app.str('ui.column', 'narrow') === 'wide' ? 'narrow' : 'wide',
      );
      break;
    case 'action.transfer':
      app.openSettings('transfer');
      break;
    case 'action.accounts':
      app.openSettings('providers');
      break;
    case 'action.autonomy':
      app.openSettings('permissions');
      break;
    case 'action.cast':
      app.openPanel('agents');
      break;
    case 'action.stopGeneration':
      void app.stopGeneration();
      break;
    case 'action.onboarding':
      app.view = 'onboarding';
      break;
    case 'action.exportMarkdown':
    case 'action.exportHtml':
      void exportConversation(app, sid, id === 'action.exportHtml' ? 'html' : 'markdown');
      break;
  }
}

function inEditable(target: EventTarget | null): boolean {
  const element = target as Partial<Element> | null;
  return (
    typeof element?.closest === 'function' &&
    element.closest('input, textarea, select, [contenteditable="true"]') !== null
  );
}

/** Esc: kolejno zamknij menu/dialog → szufladę/widok → stop mowy → stop generowania. */
export function escapeChain(app: AppState): boolean {
  if (app.palette.open || app.cheatsheetOpen) return false;
  if (app.findOpen) {
    app.findOpen = false;
    return true;
  }
  if (app.view === 'settings') {
    app.closeSettings();
    return true;
  }
  const sid = app.activeId;
  const place = app.layout.placement(sid);
  if (place.right === 'drawer' || place.right === 'sheet') {
    app.layout.update(sid, { right_open: false });
    return true;
  }
  if (place.left === 'drawer' || place.left === 'sheet') {
    app.layout.update(sid, { left_open: false });
    return true;
  }
  if (app.layout.focus) {
    app.layout.focus = false;
    return true;
  }
  if (app.micState === 'speaking') {
    void attempt(app.toasts, () => app.client.voice.stopSpeech());
    return true;
  }
  if (app.voice.reading) {
    app.voice.stopReading(app.client);
    return true;
  }
  if (app.conversation?.streaming) {
    void app.stopGeneration();
    return true;
  }
  return false;
}

export function handleKeydown(app: AppState, event: KeyboardEvent): void {
  if (event.defaultPrevented || event.isComposing) return;
  const chord = chordFromEvent(event);
  if (!chord) return;
  if (RESERVED[chord] === 'reload') {
    event.preventDefault();
    return;
  }
  if (chord === 'Esc') {
    if (app.view !== 'onboarding' && escapeChain(app)) event.preventDefault();
    return;
  }
  const def = app.keymap.get(chord);
  if (!def || COMPOSER_LOCAL.has(def.id)) return;
  if (app.view === 'onboarding' && !ONBOARDING_ALLOWED.has(def.id)) return;
  if (inEditable(event.target) && !allowedInInput(chord, def)) return;
  event.preventDefault();
  runCommand(app, def.id);
}
