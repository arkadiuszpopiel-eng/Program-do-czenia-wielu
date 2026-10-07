// Akcje na sesjach z panelu Sesje, paska tytułu, palety i composera (wydzielone z AppState).
// Każda akcja sama obsługuje błąd komendy rdzenia: toast z komunikatem, cofnięcie zmiany
// optymistycznej i wynik `true` tylko po sukcesie — `void app.newSession()` w przycisku nie gubi
// błędu po cichu (test na laptopie 2026-10-07: odrzucona komenda = „przycisk nic nie robi").
import type { SessionTemplate, UndoTicket } from '../api/types';
import type { AppState } from './app.svelte';
import { attempt, showError } from './attempt';

/** Nowa rozmowa (szablon domyślny z ustawień) i przejście do niej. */
export function createSession(app: AppState, template?: SessionTemplate): Promise<boolean> {
  const chosen = template ?? (app.str('sessions.default_template', 'empty') as SessionTemplate);
  return attempt(app.toasts, async () => {
    const created = await app.client.sessions.create(chosen);
    app.sessions.upsert(created);
    app.view = 'chat';
    await app.openSession(created.id);
  });
}

/** Zmiana nazwy: nowa nazwa od razu; po błędzie wraca poprzednia. */
export async function renameSession(app: AppState, id: string, title: string): Promise<boolean> {
  const trimmed = title.trim();
  app.sessions.renamingId = null;
  const current = app.sessions.list.find((s) => s.id === id);
  if (!trimmed || !current || current.title === trimmed) return true;
  const previous = current.title;
  app.sessions.upsert({ ...current, title: trimmed });
  if (await attempt(app.toasts, () => app.client.sessions.rename(id, trimmed))) return true;
  // Poprzednia nazwa wraca, chyba że w międzyczasie zmieniło ją co innego (zdarzenie rdzenia).
  const now = app.sessions.list.find((s) => s.id === id);
  if (now?.title === trimmed) app.sessions.upsert({ ...now, title: previous });
  return false;
}

export function setPinned(app: AppState, id: string, pinned: boolean): Promise<boolean> {
  return attempt(app.toasts, () => app.client.sessions.setPinned(id, pinned));
}

export function setArchived(app: AppState, id: string, archived: boolean): Promise<boolean> {
  return attempt(app.toasts, () => app.client.sessions.setArchived(id, archived));
}

/** Usunięcie z cofnięciem przez 10 s (toast „Cofnij"). */
export async function deleteSession(app: AppState, id: string): Promise<boolean> {
  const session = app.sessions.list.find((s) => s.id === id);
  let ticket: UndoTicket;
  try {
    ticket = await app.client.sessions.remove(id);
  } catch (error) {
    showError(app.toasts, error);
    return false;
  }
  app.sessions.remove(id);
  if (app.activeId === id) {
    const next = app.sessions.list.find((s) => !s.archived);
    if (next) await app.openSession(next.id);
    else {
      app.sessions.activeId = null;
      app.conversation = null;
    }
  }
  app.toasts.show({
    kind: 'info',
    message: app.i18n.t('sessions.deleted', { title: session?.title ?? '' }),
    actionLabel: app.i18n.t('common.undo'),
    timeoutMs: 10_000,
    onAction: () => void attempt(app.toasts, () => app.client.sessions.undoRemove(ticket.token)),
  });
  return true;
}

/** Eksport sesji do `.alfa` (natywny dialog zapisu; anulowanie to nie błąd). */
export function exportSession(app: AppState, id: string): Promise<boolean> {
  return attempt(app.toasts, async () => {
    const result = await app.client.sessions.exportSession(id);
    if (result.status === 'saved') {
      app.toasts.show({
        kind: 'success',
        message: app.i18n.t('sessions.exported', { path: result.path }),
      });
    }
  });
}

/** Duplikat jako szablon — nowa sesja przychodzi zdarzeniem `SessionUpdated`. */
export function duplicateSession(app: AppState, id: string): Promise<boolean> {
  return attempt(app.toasts, () => app.client.sessions.duplicateAsTemplate(id));
}
