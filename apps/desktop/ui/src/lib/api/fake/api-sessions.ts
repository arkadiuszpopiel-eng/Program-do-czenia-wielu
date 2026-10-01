// Atrapa: sesje (lista, tworzenie, zmiana nazwy, przypięcie, archiwum, usunięcie z cofnięciem
// przez 10 s, duplikat, eksport, wyszukiwanie, szkice).
import type { AlfaClient } from '../client';
import type { SessionSearchHit, SessionSummary, SessionTemplate } from '../types';
import type { FakeCore } from './core';
import { session as makeSession } from './fixtures';

export const TEMPLATE_TITLES: Readonly<Record<SessionTemplate, string>> = {
  empty: 'Nowa rozmowa',
  coding: 'Kodowanie',
  research: 'Research',
  voice: 'Asystent głosowy',
  admin: 'Administracja PC',
};

export function sessionsApi(core: FakeCore): AlfaClient['sessions'] {
  const trash = new Map<string, { session: SessionSummary; timer: number }>();
  return {
    list: () => core.reply(core.sessions),
    create: (template) => {
      const created = makeSession(
        core.nextId('s'),
        TEMPLATE_TITLES[template],
        core.scheduler.now(),
        {
          profile: core.status.profile,
        },
      );
      core.sessions.unshift(created);
      core.emit([{ type: 'SessionUpdated', session: created }]);
      return core.reply(created);
    },
    rename: (id, title) => core.reply(void core.updateSession(id, { title })),
    setPinned: (id, pinned) => core.reply(void core.updateSession(id, { pinned })),
    setArchived: (id, archived) => core.reply(void core.updateSession(id, { archived })),
    remove: (id) => {
      const found = core.session(id);
      const token = core.nextId('undo');
      const expires = core.scheduler.now() + 10_000;
      if (found) {
        core.sessions = core.sessions.filter((s) => s.id !== id);
        const timer = core.scheduler.setTimeout(() => trash.delete(token), 10_000);
        trash.set(token, { session: found, timer });
        core.emit([{ type: 'SessionRemoved', session_id: id }]);
      }
      return core.reply({ token, expires_at: new Date(expires).toISOString() });
    },
    undoRemove: (token) => {
      const entry = trash.get(token);
      if (entry) {
        core.scheduler.clearTimeout(entry.timer);
        trash.delete(token);
        core.sessions.push(entry.session);
        core.emit([{ type: 'SessionUpdated', session: entry.session }]);
      }
      return core.reply(undefined);
    },
    duplicateAsTemplate: (id) => {
      const source = core.session(id);
      const copy = makeSession(
        core.nextId('s'),
        `${source?.title ?? 'Sesja'} (szablon)`,
        core.scheduler.now(),
        {
          project: source?.project ?? null,
        },
      );
      core.sessions.unshift(copy);
      core.emit([{ type: 'SessionUpdated', session: copy }]);
      return core.reply(copy);
    },
    exportSession: (id) =>
      core.reply({
        status: 'saved' as const,
        path: `C:\\Users\\Ty\\Documents\\${id}.alfa`,
        files: 4,
        bytes: 18_422,
      }),
    search: (query) => {
      const q = query.trim().toLowerCase();
      const hits = q
        ? core.sessions.flatMap((s): SessionSearchHit[] => {
            const turn = (core.turns.get(s.id) ?? []).find((t) => t.text.toLowerCase().includes(q));
            if (turn)
              return [
                {
                  session_id: s.id,
                  title: s.title,
                  snippet: turn.text.slice(0, 120),
                  turn_id: turn.id,
                },
              ];
            if (s.title.toLowerCase().includes(q))
              return [{ session_id: s.id, title: s.title, snippet: '', turn_id: null }];
            return [];
          })
        : [];
      return core.reply(hits);
    },
    markRead: (id) => core.reply(void core.updateSession(id, { unread: false })),
    getDraft: (id) => core.reply(core.drafts[id] ?? ''),
    saveDraft: (id, text) => {
      core.drafts[id] = text;
      return core.reply(undefined);
    },
    workdir: (id) => core.reply(core.runs.workdir(id)),
    chooseWorkdir: (id, choice) => core.reply(core.runs.chooseWorkdir(id, choice)),
  };
}
