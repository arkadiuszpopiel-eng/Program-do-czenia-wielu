// Atrapa: Inspektor pamięci (F7) — wpisy w czterech zakresach z proweniencją, wersje (edycja =
// nowa wersja), propozycje do zatwierdzenia, zapomnienie kaskadowe z podglądem, dziennik
// z cofaniem i porządkowanie „teraz". Zdarzenie `MemoryChanged` po każdej zmianie.
import type { AlfaClient } from '../client';
import type {
  MemoryCascadeItem,
  MemoryForgetTarget,
  MemoryItem,
  MemoryJournalEntry,
  MemoryScopeInfo,
  MemoryScopeRef,
} from '../types-memory';
import type { FakeCore } from './core';

interface Entry extends MemoryItem {
  /** Pierwsza wersja (łańcuch wersji). */
  readonly chain: string;
  readonly derivedFrom: readonly string[];
}

const HOUR = 3_600_000;

const scopeKey = (s: MemoryScopeRef): string => (s.id === null ? s.kind : `${s.kind}:${s.id}`);

function seed(now: number): Entry[] {
  const at = (h: number): string => new Date(now - h * HOUR).toISOString();
  const base = (key: string, scope: MemoryScopeRef, id: string, patch: Partial<Entry>): Entry => ({
    id: `${key}#${id}`,
    scope,
    scope_key: key,
    layer: 'semantic',
    state: 'active',
    text: '',
    subject: null,
    entities: [],
    source: 'user',
    source_detail: null,
    trusted: true,
    confidence: 0.9,
    pinned: false,
    version: 1,
    created_at: at(24),
    expires_at: null,
    session_id: null,
    turn: null,
    derivation: null,
    score: null,
    chain: `${key}#${id}`,
    derivedFrom: [],
    ...patch,
  });
  const session = { kind: 'session' as const, id: 's-q3' };
  const project = { kind: 'project' as const, id: 'p-x' };
  return [
    base('session:s-q3', session, 'm1', {
      layer: 'episodic',
      state: 'superseded',
      text: 'Raport Q3 ma trafić do zarządu do czwartku.',
      session_id: 's-q3',
      turn: 3,
      created_at: at(30),
    }),
    base('session:s-q3', session, 'm2', {
      layer: 'episodic',
      text: 'Raport Q3 ma trafić do zarządu do piątku.',
      subject: 'termin raportu',
      session_id: 's-q3',
      turn: 5,
      version: 2,
      derivation: 'edited',
      chain: 'session:s-q3#m1',
      created_at: at(20),
    }),
    base('project:p-x', project, 'm3', {
      text: 'Waluta raportów: PLN, kwoty bez groszy w tabelach.',
      subject: 'format raportów',
      entities: ['PLN'],
      derivation: 'extracted',
      derivedFrom: ['session:s-q3#m2'],
    }),
    base('agent:beta', { kind: 'agent', id: 'beta' }, 'm4', {
      layer: 'procedural',
      state: 'pending',
      text: 'Użytkownik woli krótkie podsumowanie na początku odpowiedzi.',
      source: 'agent',
      source_detail: 'beta',
      confidence: 0.7,
      derivation: 'summary',
    }),
    base('global', { kind: 'global', id: null }, 'm5', {
      layer: 'working',
      text: 'Komentarze w kodzie piszę po polsku.',
      pinned: true,
    }),
    base('global', { kind: 'global', id: null }, 'm6', {
      text: 'Strona dostawcy podaje czas dostawy 14 dni.',
      source: 'untrusted',
      source_detail: 'https://dostawca.example/warunki',
      trusted: false,
      confidence: 0.5,
      expires_at: new Date(now + 30 * 24 * HOUR).toISOString(),
    }),
  ];
}

export function memoryApi(core: FakeCore): AlfaClient['memory'] {
  let entries = seed(core.scheduler.now());
  const journal: MemoryJournalEntry[] = [];
  let last: Awaited<ReturnType<AlfaClient['memory']['consolidateNow']>> | null = null;
  const fail = (message: string): Promise<never> => Promise.reject(new Error(message));
  const find = (id: string): Entry | undefined => entries.find((e) => e.id === id);
  const plain = (e: Entry): MemoryItem => {
    const { chain: _chain, derivedFrom: _from, ...item } = e;
    return item;
  };
  const changed = (scope: string | null): void =>
    core.emit([{ type: 'MemoryChanged', scope_key: scope }]);
  const log = (scope: string, kind: string, note: string, ids: string[], undoable = true) =>
    journal.push({
      id: core.nextId('chg'),
      scope_key: scope,
      run: null,
      at: core.isoNow(),
      kind,
      note,
      entries: ids,
      undone: false,
      undoable,
    });
  const add = (from: Entry, patch: Partial<Entry>): Entry => {
    const id = `${patch.scope_key ?? from.scope_key}#${core.nextId('m')}`;
    const next: Entry = { ...from, created_at: core.isoNow(), ...patch, id };
    entries.push(next);
    return next;
  };
  const cascade = (target: MemoryForgetTarget): MemoryCascadeItem[] => {
    const item = (e: Entry, reason: string): MemoryCascadeItem => ({
      id: e.id,
      scope_key: e.scope_key,
      text: e.text.slice(0, 120),
      reason,
    });
    if (target.target === 'scope')
      return entries.filter((e) => e.scope_key === target.scope).map((e) => item(e, 'target'));
    const root = find(target.id);
    if (!root) return [];
    const versions = entries.filter((e) => e.chain === root.chain && e.id !== root.id);
    const ids = new Set([root.id, ...versions.map((v) => v.id)]);
    const derived = entries.filter((e) => e.derivedFrom.some((d) => ids.has(d)));
    return [
      item(root, 'target'),
      ...versions.map((v) => item(v, 'version')),
      ...derived.map((d) => item(d, 'derived')),
    ];
  };

  return {
    status: () =>
      core.reply({
        consolidation_enabled: core.settings['memory.consolidation_enabled'] !== false,
        idle_available: false,
        model_available: core.status.profile !== 'local',
        window: String(core.settings['memory.consolidation_window'] ?? '02:00-05:00'),
        pending: entries.filter((e) => e.state === 'pending').length,
        last,
      }),
    scopes: () => {
      const map = new Map<string, MemoryScopeInfo>();
      for (const e of entries) {
        const prev = map.get(e.scope_key);
        const label =
          e.scope.kind === 'global'
            ? 'Globalna'
            : e.scope.kind === 'session'
              ? (core.session(e.scope.id ?? '')?.title ?? e.scope_key)
              : e.scope.kind === 'project'
                ? 'Projekt X'
                : `Agentka ${e.scope.id ?? ''}`;
        map.set(e.scope_key, {
          key: e.scope_key,
          scope: e.scope,
          label,
          entries: (prev?.entries ?? 0) + 1,
          active: (prev?.active ?? 0) + (e.state === 'active' ? 1 : 0),
          pending: (prev?.pending ?? 0) + (e.state === 'pending' ? 1 : 0),
          document: `memory/${e.scope_key.replace(':', '-')}`,
        });
      }
      return core.reply([...map.values()]);
    },
    inspect: (q) => {
      const text = q.text?.trim().toLowerCase() ?? '';
      const hits = entries.filter(
        (e) =>
          (q.scopes.length === 0 || q.scopes.includes(e.scope_key)) &&
          (q.layers.length === 0 || q.layers.includes(e.layer)) &&
          (q.states.length === 0 || q.states.includes(e.state)) &&
          (q.trusted === null || e.trusted === q.trusted) &&
          (q.pinned === null || e.pinned === q.pinned) &&
          (!text || e.text.toLowerCase().includes(text) || e.subject?.includes(text)),
      );
      const page = hits.slice(q.offset, q.offset + (q.limit || 50)).map(plain);
      return core.reply({ items: page, total: hits.length });
    },
    explain: (id) => {
      const e = find(id);
      if (!e) return fail(`Nie ma wpisu „${id}”.`);
      const reasons = [
        e.source === 'user'
          ? 'Zapisane na Twoje polecenie.'
          : e.source === 'agent'
            ? `Zaproponowała agentka ${e.source_detail ?? ''}.`
            : `Z treści niezaufanej: ${e.source_detail ?? 'nieznane źródło'}.`,
        ...(e.session_id ? [`Z rozmowy ${e.session_id}, tura ${e.turn ?? '?'}.`] : []),
        ...(e.pinned ? ['Przypięte — zawsze w kontekście.'] : []),
      ];
      return core.reply({
        item: plain(e),
        reasons,
        sources: e.derivedFrom.map((s) => ({
          id: s,
          exists: !!find(s),
          state: find(s)?.state ?? null,
        })),
        versions: entries
          .filter((v) => v.chain === e.chain)
          .sort((a, b) => a.version - b.version)
          .map(plain),
        merged: [],
        derived: entries.filter((d) => d.derivedFrom.includes(e.id)).map((d) => d.id),
        journal: journal.filter((j) => j.entries.includes(e.id)),
      });
    },
    edit: (id, edit) => {
      const e = find(id);
      if (!e) return fail(`Nie ma wpisu „${id}”.`);
      const next = add(e, {
        text: edit.text ?? e.text,
        subject: edit.subject === null ? e.subject : edit.subject || null,
        confidence: edit.confidence ?? e.confidence,
        version: e.version + 1,
        derivation: 'edited',
        state: 'active',
      });
      entries = entries.map((x) => (x.id === e.id ? { ...x, state: 'superseded' } : x));
      log(e.scope_key, 'edit', 'Edycja przez użytkownika (nowa wersja).', [e.id, next.id]);
      changed(e.scope_key);
      return core.reply(plain(next));
    },
    setPinned: (id, pinned) => {
      const e = find(id);
      if (!e) return fail(`Nie ma wpisu „${id}”.`);
      const next = { ...e, pinned };
      entries = entries.map((x) => (x.id === id ? next : x));
      changed(e.scope_key);
      return core.reply(plain(next));
    },
    approve: (id) => {
      const e = find(id);
      if (!e || e.state !== 'pending') return fail('Ten wpis nie czeka na zatwierdzenie.');
      const next: Entry = { ...e, state: 'active' };
      entries = entries.map((x) => (x.id === id ? next : x));
      log(e.scope_key, 'create', 'Propozycja zatwierdzona przez użytkownika.', [id], false);
      changed(e.scope_key);
      return core.reply(plain(next));
    },
    promote: (id, to) => {
      const e = find(id);
      if (!e) return fail(`Nie ma wpisu „${id}”.`);
      const key = scopeKey(to);
      const copy = add(e, {
        scope: to,
        scope_key: key,
        derivation: 'promoted',
        version: 1,
        pinned: false,
      });
      entries = entries.map((x) =>
        x.id === copy.id ? { ...copy, chain: copy.id, derivedFrom: [id] } : x,
      );
      log(key, 'promote', `Awans z zakresu ${e.scope_key}.`, [copy.id]);
      changed(key);
      return core.reply(plain(copy));
    },
    forgetPreview: (target) =>
      core.reply({ target, remove: cascade(target), revive: [], shred: target.target === 'scope' }),
    forget: (target) => {
      const remove = cascade(target);
      const ids = new Set(remove.map((r) => r.id));
      entries = entries.filter((e) => !ids.has(e.id));
      changed(target.target === 'scope' ? target.scope : null);
      return core.reply({
        removed: remove.filter((r) => r.reason === 'target').length,
        derived: remove.filter((r) => r.reason === 'derived').length,
        versions: remove.filter((r) => r.reason === 'version').length,
        revived: 0,
        fts_rows: ids.size,
        vectors: ids.size,
        journal_records: 0,
        shredded: target.target === 'scope' ? [target.scope] : [],
        stale_exports: [],
      });
    },
    journal: (scope) => core.reply(journal.filter((j) => j.scope_key === scope).reverse()),
    undo: (scope, changeId) => {
      const index = journal.findIndex((j) => j.id === changeId && j.scope_key === scope);
      const record = journal[index];
      if (!record || record.undone || !record.undoable)
        return fail('Tej zmiany nie da się cofnąć.');
      const [previous, created] =
        record.kind === 'edit' ? record.entries : [null, record.entries[0]];
      entries = entries
        .filter((e) => e.id !== created)
        .map((e) => (e.id === previous ? { ...e, state: 'active' } : e));
      journal[index] = { ...record, undone: true };
      changed(scope);
      return core.reply({ removed: 1, restored: previous ? 1 : 0, skipped: 0 });
    },
    consolidateNow: () => {
      last = {
        run: core.nextId('run'),
        manual: true,
        started_at: core.isoNow(),
        skipped: null,
        interrupted: null,
        scopes: new Set(entries.map((e) => e.scope_key)).size,
        created: 0,
        merged: 0,
        resolved: 0,
        expired: 0,
        conflicts: 0,
        proposals: entries.filter((e) => e.state === 'pending').length,
        llm_calls: 0,
        budget_denied: false,
        errors: [],
      };
      changed(null);
      return core.reply(last);
    },
  };
}
