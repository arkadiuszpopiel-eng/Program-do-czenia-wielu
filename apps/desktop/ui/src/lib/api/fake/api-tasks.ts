// Atrapa: panel Zadania (DAG schedulera z postępem na wirtualnym zegarze, sterowanie, anulowanie
// z poddrzewem, ponowienie z tym samym pochodzeniem) i Wyzwalacze (czasowe z podglądem cron,
// plikowe bez obserwacji na tej platformie, ręczne; dziennik uruchomień).
import type { AlfaClient } from '../client';
import type {
  NewTaskInput,
  TaskInfo,
  TaskOriginKind,
  TriggerInfo,
  TriggerKindView,
  TriggerRunInfo,
} from '../types-tasks';
import type { FakeCore } from './core';
import { cronNext } from './cron';

const STEP_MS = 400;
const MIN = 60_000;

export class FakeTasks {
  tasks: TaskInfo[] = [];
  triggers: TriggerInfo[] = [];
  readonly runs: TriggerRunInfo[] = [];
  private retries = 0;

  constructor(private readonly core: FakeCore) {
    if (core.scenario === 'empty' || core.scenario === 'first-run') return;
    const now = core.scheduler.now();
    const iso = (t: number): string => new Date(t).toISOString();
    const task = (id: string, title: string, patch: Partial<TaskInfo>): TaskInfo => ({
      id,
      title,
      parent_id: null,
      deps: [],
      assignee: 'any',
      agent: null,
      class: 'user',
      origin: 'user',
      origin_detail: null,
      executor: 'agent',
      state: 'pending',
      result: null,
      blocked: null,
      error: null,
      summary: null,
      attempt: 1,
      max_attempts: 3,
      steps: 0,
      max_steps: 40,
      cost: { minor: 0, currency: 'PLN' },
      session_id: 's-q3',
      tainted: false,
      submitted_at: iso(now - 20 * MIN),
      deadline_at: iso(now + 60 * MIN),
      ...patch,
    });
    this.tasks = [
      task('t-dane', 'Zbierz dane sprzedaży Q3', {
        agent: 'beta',
        assignee: 'beta',
        state: 'done',
        result: 'succeeded',
        steps: 6,
        summary: 'Zebrano 3 arkusze (lipiec–wrzesień).',
        cost: { minor: 12, currency: 'PLN' },
      }),
      task('t-marze', 'Policz marże i trendy', {
        agent: 'gama',
        assignee: 'gama',
        deps: [{ task_id: 't-dane', condition: 'succeeded' }],
        state: 'running',
        steps: 3,
        cost: { minor: 8, currency: 'PLN' },
      }),
      task('t-raport', 'Złóż raport dla zarządu', {
        assignee: 'alfa',
        deps: [{ task_id: 't-marze', condition: 'succeeded' }],
        blocked: 'czeka na zadanie „Policz marże i trendy”',
      }),
      task('t-testy', 'Popraw testy modułu płatności', {
        executor: 'bridge:claude_code',
        agent: 'delta',
        session_id: 's-api',
        state: 'done',
        result: 'failed',
        error: 'Most odmówił: wersja CLI nie jest przypięta.',
        max_attempts: 1,
      }),
    ];
    this.triggers = [
      this.trigger('poranek', 'Poranny przegląd', { kind: 'cron', expr: '0 8 * * 1-5' }),
      this.trigger('pobrane', 'Nowy plik w Pobranych', {
        kind: 'file_in_dir',
        dir: 'C:\\Users\\Ty\\Downloads',
        pattern: '*.pdf',
      }),
      this.trigger('porzadki', 'Sprzątanie Pobranych', { kind: 'manual' }),
    ];
  }

  private trigger(id: string, name: string, kind: TriggerKindView): TriggerInfo {
    return {
      id,
      name,
      kind,
      enabled: true,
      owner: 'user',
      title: name,
      goal: `${name}: przygotuj krótkie podsumowanie.`,
      agent: 'alfa',
      bridge: null,
      tz: 'Europe/Warsaw',
      next_fire_at: this.nextFire(kind),
      last_fire_at: null,
      fired: 0,
      suppressed: 0,
      deferred_until: null,
      respect_dnd: true,
      watch_unavailable: kind.kind === 'file_in_dir',
    };
  }

  nextFire(kind: TriggerKindView): string | null {
    const now = this.core.scheduler.now();
    if (kind.kind === 'cron') {
      const r = cronNext(kind.expr, now, 1);
      return 'next' in r && r.next[0] !== undefined ? new Date(r.next[0]).toISOString() : null;
    }
    if (kind.kind === 'interval') return new Date(now + kind.every_minutes * MIN).toISOString();
    if (kind.kind === 'once') return kind.at;
    return null;
  }

  private update(id: string, patch: Partial<TaskInfo>): TaskInfo | undefined {
    const index = this.tasks.findIndex((t) => t.id === id);
    const current = this.tasks[index];
    if (!current) return undefined;
    const next = { ...current, ...patch };
    this.tasks[index] = next;
    this.core.emit([{ type: 'TaskUpdated', task: next }]);
    return next;
  }

  /** Zgłoszenie i przebieg na wirtualnym zegarze (gotowe → w toku → 3 kroki → sukces). */
  submit(task: TaskInfo): TaskInfo {
    this.tasks.push(task);
    this.core.emit([{ type: 'TaskUpdated', task }]);
    const tick = (): void => {
      const t = this.tasks.find((x) => x.id === task.id);
      if (!t || t.state === 'done' || t.state === 'paused') return;
      if (t.steps >= 3) {
        this.update(t.id, { state: 'done', result: 'succeeded', summary: 'Gotowe (atrapa).' });
        return;
      }
      this.update(t.id, { state: 'running', steps: t.steps + 1, agent: t.agent ?? 'alfa' });
      this.core.scheduler.setTimeout(tick, STEP_MS);
    };
    if (task.deps.length === 0) this.core.scheduler.setTimeout(tick, STEP_MS);
    return task;
  }

  /** Nowe zadanie (pochodzenie: użytkownik albo wyzwalacz). */
  build(input: NewTaskInput, origin: TaskOriginKind, detail: string | null): TaskInfo {
    const now = this.core.scheduler.now();
    const goal = input.goal.trim();
    return {
      id: this.core.nextId(origin === 'user' ? 'u' : 'tr'),
      title: input.title.trim() || goal.slice(0, 60),
      parent_id: input.parent_id,
      deps: input.after.map((task_id) => ({ task_id, condition: 'succeeded' })),
      assignee: input.agent ?? 'any',
      agent: null,
      class: origin === 'user' ? 'user' : 'background',
      origin,
      origin_detail: detail,
      executor: 'agent',
      state: input.after.length ? 'pending' : 'ready',
      result: null,
      blocked: null,
      error: null,
      summary: null,
      attempt: 1,
      max_attempts: 3,
      steps: 0,
      max_steps: 40,
      cost: { minor: 0, currency: 'PLN' },
      session_id: input.session_id,
      tainted: false,
      submitted_at: new Date(now).toISOString(),
      deadline_at: new Date(now + 60 * MIN).toISOString(),
    };
  }

  api(): AlfaClient['tasks'] {
    const core = this.core;
    const get = (id: string): TaskInfo | undefined => this.tasks.find((t) => t.id === id);
    const missing = (id: string) => Promise.reject(new Error(`Zadanie „${id}” nie istnieje.`));
    return {
      list: () => core.reply(this.tasks),
      create: (input) => {
        if (!input.goal.trim()) {
          return Promise.reject(new Error('Cel zadania musi mieć od 1 do 4000 znaków.'));
        }
        return core.reply(this.submit(this.build(input, 'user', null)));
      },
      cancel: (id) => {
        if (!get(id)) return missing(id);
        const ids = [id];
        for (let i = 0; i < ids.length; i++) {
          for (const t of this.tasks) if (t.parent_id === ids[i]) ids.push(t.id);
        }
        const live = ids.filter((t) => get(t)?.state !== 'done');
        for (const t of live) this.update(t, { state: 'done', result: 'cancelled' });
        return core.reply(live);
      },
      retry: (id) => {
        const t = get(id);
        if (!t) return missing(id);
        if (t.state !== 'done')
          return Promise.reject(new Error('Ponowić można tylko zakończone zadanie.'));
        this.retries++;
        const base = id.split('.retry')[0] ?? id;
        const copy = this.submit({
          ...t,
          id: `${base}.retry${this.retries}`,
          deps: [],
          parent_id: null,
          state: 'ready',
          result: null,
          error: null,
          summary: null,
          steps: 0,
          submitted_at: core.isoNow(),
        });
        return core.reply(copy);
      },
      steer: (id, text) => {
        const t = get(id);
        if (!t) return missing(id);
        if (t.state === 'done' || !text.trim()) {
          return Promise.reject(new Error('Zadanie zakończone — nie da się nim sterować.'));
        }
        return core.reply(undefined);
      },
      pause: (id) => core.reply(void this.update(id, { state: 'paused' })),
      resume: (id) => {
        const t = this.update(id, { state: 'ready' });
        if (t) this.core.scheduler.setTimeout(() => this.submitTick(id), STEP_MS);
        return core.reply(undefined);
      },
    };
  }

  private submitTick(id: string): void {
    const t = this.tasks.find((x) => x.id === id);
    if (!t) return;
    this.tasks = this.tasks.filter((x) => x.id !== id);
    this.submit(t);
  }

  triggersApi(): AlfaClient['triggers'] {
    const core = this.core;
    const get = (id: string): TriggerInfo | undefined => this.triggers.find((t) => t.id === id);
    const missing = (id: string) => Promise.reject(new Error(`Wyzwalacz „${id}” nie istnieje.`));
    const set = (id: string, patch: Partial<TriggerInfo>): void => {
      this.triggers = this.triggers.map((t) => (t.id === id ? { ...t, ...patch } : t));
    };
    return {
      list: () => core.reply(this.triggers),
      create: (draft) => {
        if (!draft.name.trim() || !draft.goal.trim()) {
          return Promise.reject(new Error('Podaj nazwę i cel wyzwalacza.'));
        }
        if (draft.kind.kind === 'cron' && 'error' in cronNext(draft.kind.expr, 0, 1)) {
          return Promise.reject(new Error('Nieprawidłowe wyrażenie cron.'));
        }
        const id = `${draft.name.toLowerCase().replace(/[^a-z0-9]+/g, '-')}-${this.triggers.length}`;
        const created: TriggerInfo = {
          ...this.trigger(id, draft.name.trim(), draft.kind),
          title: draft.title.trim() || draft.name.trim(),
          goal: draft.goal.trim(),
          agent: draft.agent,
          bridge: draft.bridge,
          respect_dnd: draft.respect_dnd,
        };
        this.triggers.push(created);
        return core.reply(created);
      },
      remove: (id) => {
        this.triggers = this.triggers.filter((t) => t.id !== id);
        return core.reply(undefined);
      },
      setEnabled: (id, enabled) => core.reply(set(id, { enabled })),
      fireNow: (id) => {
        const t = get(id);
        if (!t) return missing(id);
        const input = {
          session_id: null,
          title: t.title,
          goal: t.goal,
          agent: t.agent,
          after: [],
          parent_id: null,
        };
        const task = this.submit(this.build(input, 'trigger', id));
        const run: TriggerRunInfo = {
          at: core.isoNow(),
          trigger_id: id,
          cause: 'ręcznie',
          outcome: 'submitted',
          task_id: task.id,
          detail: null,
        };
        this.runs.push(run);
        set(id, { fired: t.fired + 1, last_fire_at: run.at });
        core.emit([{ type: 'TriggerFired', run }]);
        return core.reply(run);
      },
      log: (id) => core.reply(this.runs.filter((r) => id === null || r.trigger_id === id)),
      previewCron: (expr) => {
        const r = cronNext(expr, core.scheduler.now(), 5);
        return core.reply(
          'error' in r
            ? { valid: false, error: r.error, next: [] }
            : { valid: true, error: null, next: r.next.map((t) => new Date(t).toISOString()) },
        );
      },
    };
  }
}
