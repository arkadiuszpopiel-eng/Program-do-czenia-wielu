// Atrapa „Naucz Alfę” (P5): podział jak w rdzeniu (`logic/memory-teach`), tekst i plik jako treść
// niezaufana (tylko zakres sesji), rozmowa jako wiedza użytkownika (tury bez ukrytych); wybór
// pliku zwraca notatkę. Rozmowa bez wiadomości → błąd „Nie ma treści do nauki.” (jak rdzeń).
import type { MemoryItem, MemoryScopeRef, MemoryTeachApi, TeachFile } from '../types-memory';
import { chunkText, isExternal, oneLine } from '../../logic/memory-teach';
import type { FakeCore } from './core';

/** Plik zwracany przez „Wybierz plik…” w atrapie. */
export const FAKE_TEACH_FILE: TeachFile = {
  name: 'wyjazd-zakopane.md',
  text: '# Wyjazd\n\nHotel Pod Giewontem: zameldowanie od 15:00, parking w cenie.\n\nPociąg z Krakowa o 7:12, peron 3.',
  chars: 105,
  truncated: false,
};

/** Granica akapitów (każda tura rozmowy to osobny akapit). */
const PARAGRAPH = '\n\n';

/** Wpis atrapy pamięci (bez pól łańcucha wersji — dokłada je `api-memory`). */
export type TaughtItem = MemoryItem;

const hash = (text: string): string => {
  let h = 2166136261;
  for (const c of text) h = Math.imul(h ^ (c.codePointAt(0) ?? 0), 16777619) >>> 0;
  return h.toString(16).padStart(8, '0').repeat(2);
};

export function teachApi(
  core: FakeCore,
  add: (items: readonly TaughtItem[]) => void,
): MemoryTeachApi {
  return {
    teachPickFile: () => core.reply(FAKE_TEACH_FILE),
    teach: (request) => {
      const { source } = request;
      const external = isExternal(source.kind);
      if (external && request.scope !== 'session')
        return Promise.reject(
          new Error(
            'Tekst i pliki to treść z zewnątrz (niezaufana) — trafiają tylko do pamięci tej rozmowy.',
          ),
        );
      const session = core.session(request.session_id);
      const text =
        source.kind === 'conversation'
          ? core
              .turnsOf(request.session_id)
              .filter((t) => !core.annotations[t.id]?.hidden && t.text.trim())
              .map((t) => `${t.author === 'user' ? 'Użytkownik' : 'Agentka'}: ${t.text}`)
              .join(PARAGRAPH)
          : source.text;
      const chunks = chunkText(text);
      if (chunks.pieces.length === 0) return Promise.reject(new Error('Nie ma treści do nauki.'));
      const fallback =
        source.kind === 'file'
          ? source.name
          : source.kind === 'text'
            ? 'Wklejony tekst'
            : `Rozmowa „${session?.title ?? ''}”`;
      const title = oneLine(request.title ?? '').replace(/[[\]]/g, '') || fallback;
      const project = session?.project?.id ?? null;
      if (request.scope === 'project' && !project)
        return Promise.reject(new Error('Ta sesja nie należy do projektu — wybierz inny zakres.'));
      if (request.scope === 'agent')
        return Promise.reject(
          new Error(
            'Naukę zapisuję w pamięci rozmowy, projektu albo globalnej — wybierz jeden z tych zakresów.',
          ),
        );
      const scope: MemoryScopeRef =
        request.scope === 'global'
          ? { kind: 'global', id: null }
          : request.scope === 'project'
            ? { kind: 'project', id: project }
            : { kind: 'session', id: request.session_id };
      const key = scope.id === null ? scope.kind : `${scope.kind}:${scope.id}`;
      const label = source.kind === 'file' ? `plik „${source.name}”` : 'wklejony tekst';
      const origin = external
        ? `naucz: ${label} · sha256:${hash(text)}`
        : `rozmowa:${request.session_id}`;
      const n = chunks.pieces.length;
      const items: TaughtItem[] = chunks.pieces.map((piece, i) => ({
        id: `${key}#${core.nextId('mem')}`,
        scope,
        scope_key: key,
        layer: 'semantic',
        state: 'active',
        text: n === 1 ? `[${title}] ${piece}` : `[${title} · ${i + 1}/${n}] ${piece}`,
        subject: null,
        entities: [title],
        source: external ? 'untrusted' : 'user',
        source_detail: external ? origin : null,
        trusted: !external,
        confidence: external ? 0.7 : 1,
        pinned: false,
        version: 1,
        created_at: core.isoNow(),
        expires_at: null,
        session_id: request.session_id,
        turn: null,
        derivation: null,
        score: null,
      }));
      add(items);
      return core.reply({
        facts: n,
        ids: items.map((x) => x.id),
        scope_key: key,
        source: origin,
        trusted: !external,
        truncated: chunks.truncated,
        duplicates: chunks.duplicates,
      });
    },
  };
}
