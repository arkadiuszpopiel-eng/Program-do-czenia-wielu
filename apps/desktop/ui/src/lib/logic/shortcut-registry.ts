// Rejestr skrótów z PLAN §14.8 (etykiety: klucze i18n `shortcut.<id>`). Skróty `global` rejestruje
// rdzeń (RegisterHotKey) — UI tylko je pokazuje i sprawdza konflikty. Composer obsługuje własne
// klawisze (Enter, ↑, Ctrl+↑/↓) lokalnie; są tu dla ściągawki.
import type { ShortcutDef } from './shortcuts';

const app = (
  id: string,
  group: ShortcutDef['group'],
  defaults: readonly string[],
  extra: Partial<Pick<ShortcutDef, 'customizable' | 'inInput'>> = {},
): ShortcutDef => ({
  id,
  group,
  defaults,
  scope: 'app',
  customizable: extra.customizable ?? true,
  inInput: extra.inInput ?? false,
});

const global = (id: string, defaults: readonly string[], customizable: boolean): ShortcutDef => ({
  id,
  group: 'system',
  defaults,
  scope: 'global',
  customizable,
  inInput: true,
});

export const SHORTCUTS: readonly ShortcutDef[] = [
  app('session.new', 'sessions', ['Ctrl+N']),
  app('session.switch', 'sessions', ['Ctrl+P']),
  app('session.close', 'sessions', ['Ctrl+W']),
  app('session.reopen', 'sessions', ['Ctrl+Shift+T']),
  app('session.next', 'sessions', ['Ctrl+Tab']),
  app('session.prev', 'sessions', ['Ctrl+Shift+Tab']),
  ...Array.from({ length: 9 }, (_, i) =>
    app(`session.goto${i + 1}`, 'sessions', [`Ctrl+${i + 1}`]),
  ),
  app('session.rename', 'sessions', ['F2']),
  app('search.everywhere', 'sessions', ['Ctrl+Shift+F']),
  app('search.conversation', 'conversation', ['Ctrl+F']),
  app('palette.open', 'view', ['Ctrl+K']),
  app('cheatsheet.open', 'view', ['Ctrl+/']),
  app('settings.open', 'view', ['Ctrl+,']),
  app('view.focus', 'view', ['F11', 'Ctrl+Shift+Enter']),
  app('view.zoomIn', 'view', ['Ctrl+=']),
  app('view.zoomOut', 'view', ['Ctrl+-']),
  app('view.zoomReset', 'view', ['Ctrl+0']),
  app('panel.sessions', 'panels', ['Ctrl+B']),
  app('panel.right', 'panels', ['Ctrl+\\']),
  app('panel.agents', 'panels', ['Alt+1']),
  app('panel.timeline', 'panels', ['Alt+2']),
  app('panel.files', 'panels', ['Alt+3']),
  app('panel.memory', 'panels', ['Alt+4']),
  app('panel.screen', 'panels', ['Alt+5']),
  app('panel.voice', 'panels', ['Alt+6']),
  app('panel.tasks', 'panels', ['Alt+7']),
  app('voice.mic', 'voice', ['Ctrl+Shift+M']),
  app('voice.ptt', 'voice', ['Space'], { customizable: false }),
  app('conversation.escape', 'conversation', ['Esc'], { customizable: false, inInput: true }),
  app('composer.send', 'conversation', ['Enter'], { customizable: false, inInput: true }),
  app('composer.newline', 'conversation', ['Shift+Enter'], { customizable: false, inInput: true }),
  app('composer.editLast', 'conversation', ['Up'], { customizable: false, inInput: true }),
  app('composer.historyPrev', 'conversation', ['Ctrl+Up'], { customizable: false, inInput: true }),
  app('composer.historyNext', 'conversation', ['Ctrl+Down'], {
    customizable: false,
    inInput: true,
  }),
  app('composer.pastePlain', 'conversation', ['Ctrl+Shift+V'], {
    customizable: false,
    inInput: true,
  }),
  global('system.quickAsk', ['Ctrl+Alt+Space'], true),
  global('system.killSwitch', ['Ctrl+Shift+F12'], false),
  // F5: rejestruje powłoka (shortcuts.rs) — D i R nie są literami polskimi (reguła AltGr).
  global('voice.dictation', ['Ctrl+Alt+D'], false),
  global('voice.readSelection', ['Ctrl+Alt+R'], false),
];

/** Akcje obsługiwane przez composer lokalnie — nie trafiają do globalnej mapy klawiszy okna. */
export const COMPOSER_LOCAL = new Set([
  'voice.ptt',
  'conversation.escape',
  'composer.send',
  'composer.newline',
  'composer.editLast',
  'composer.historyPrev',
  'composer.historyNext',
  'composer.pastePlain',
]);

export const SHORTCUT_GROUPS: readonly ShortcutDef['group'][] = [
  'sessions',
  'conversation',
  'view',
  'panels',
  'voice',
  'system',
];
