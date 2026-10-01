// Atrapa: przebiegi agentek z narzędziami (Replay krok po kroku), wiadomość w trakcie zadania,
// „uruchom w terminalu", katalog roboczy sesji. Kroki emitowane przez strumień (`stream.ts`).
import type { AgentId } from '@alfa/ui-kit';
import type {
  AgentRun,
  AgentRunDetail,
  ReplayStep,
  RunState,
  SessionWorkdir,
  ToolIntent,
  WorkdirChoice,
} from '../types-agents';
import type { FakeCore } from './core';

const DEFAULT_BUDGET = { max_steps: 40, max_minutes: 15, max_cost: null } as const;

export function replayStep(
  runId: string,
  n: number,
  patch: Partial<ReplayStep> & Pick<ReplayStep, 'title'>,
): ReplayStep {
  return {
    id: `${runId}:s${n}`,
    n,
    kind: 'tool',
    tool: null,
    input: '',
    output: '',
    status: 'running',
    at_ms: 0,
    duration_ms: null,
    undo_token: null,
    undone: false,
    untrusted: false,
    intent: null,
    approval_id: null,
    ...patch,
  };
}

export function terminalIntent(command: string, cwd: string): ToolIntent {
  return {
    kind: 'open_in_terminal',
    title: 'Uruchom w terminalu',
    command,
    cwd,
    shell: 'pwsh',
    paths: [],
  };
}

function seedQ3Run(at: number): AgentRunDetail {
  const id = 's-q3:r1';
  const started = new Date(at).toISOString();
  return {
    run: {
      id,
      session_id: 's-q3',
      turn_id: 'q6',
      agent: 'delta',
      goal: 'Przygotuj szkic raportu Q3 z arkuszy przychodów.',
      workdir: 'C:\\Users\\Ty\\Alfa\\Sesje\\Raport Q3',
      state: 'waiting_approval',
      started_at: started,
      finished_at: null,
      summary: null,
      usage: {
        steps: 4,
        tool_calls: 2,
        input_tokens: 4200,
        output_tokens: 410,
        cost: { minor: 12, currency: 'PLN' },
        elapsed_ms: 5400,
      },
      budget: DEFAULT_BUDGET,
    },
    steps: [
      replayStep(id, 1, {
        kind: 'plan',
        title: 'Plan',
        input: 'Zadanie od właściciela: przygotuj szkic raportu Q3',
        output: '1. Odczytam arkusze. 2. Utworzę szkic. 3. Poproszę o zgodę na szablon.',
        status: 'ok',
        duration_ms: 900,
      }),
      replayStep(id, 2, {
        tool: 'fs_read',
        title: 'Odczyt pliku',
        input: '{"path":"Finanse/2026-Q3/przychody.xlsx"}',
        output: 'Odczytano 3 arkusze przychodów',
        status: 'ok',
        at_ms: 900,
        duration_ms: 1200,
        untrusted: true,
      }),
      replayStep(id, 3, {
        tool: 'fs_write',
        title: 'Zapis pliku',
        input: '{"path":"raport-Q3.docx","mode":"create"}',
        output: 'Delta: zapisano 1 plik',
        status: 'ok',
        at_ms: 2100,
        duration_ms: 2100,
        undo_token: 's-q3:u5',
      }),
      replayStep(id, 4, {
        tool: 'fs_write',
        title: 'Zapis pliku',
        input: '{"path":"Raporty/zarząd.dotx","mode":"overwrite"}',
        status: 'waiting_approval',
        at_ms: 4200,
        approval_id: 'ap-1',
      }),
    ],
  };
}

export class FakeRuns {
  private readonly bySession = new Map<string, AgentRunDetail[]>();
  private readonly workdirs = new Map<string, string | null>();
  private counter = 0;

  constructor(private readonly core: FakeCore) {}

  list(sessionId: string): AgentRunDetail[] {
    let list = this.bySession.get(sessionId);
    if (!list) {
      list =
        sessionId === 's-q3' && this.core.scenario !== 'empty'
          ? [seedQ3Run(this.core.scheduler.now() - 30 * 60_000)]
          : [];
      this.bySession.set(sessionId, list);
    }
    return list;
  }

  private find(runId: string): AgentRunDetail | undefined {
    for (const list of this.bySession.values()) {
      const run = list.find((r) => r.run.id === runId);
      if (run) return run;
    }
    return undefined;
  }

  private replaceRun(detail: AgentRunDetail, run: AgentRun): void {
    const list = this.list(run.session_id);
    const index = list.indexOf(detail);
    const next = { run, steps: detail.steps };
    if (index >= 0) list[index] = next;
    this.core.emit([{ type: 'AgentRunUpdated', session_id: run.session_id, run }]);
  }

  begin(sessionId: string, turnId: string, agent: AgentId, goal: string): string {
    this.counter++;
    const id = `${sessionId}:r${this.counter + 1}`;
    const run: AgentRun = {
      id,
      session_id: sessionId,
      turn_id: turnId,
      agent,
      goal,
      workdir: this.workdir(sessionId).path,
      state: 'running',
      started_at: this.core.isoNow(),
      finished_at: null,
      summary: null,
      usage: {
        steps: 0,
        tool_calls: 0,
        input_tokens: 0,
        output_tokens: 0,
        cost: { minor: 0, currency: 'PLN' },
        elapsed_ms: 0,
      },
      budget: DEFAULT_BUDGET,
    };
    this.list(sessionId).push({ run, steps: [] });
    this.core.emit([{ type: 'AgentRunUpdated', session_id: sessionId, run }]);
    return id;
  }

  step(runId: string, step: ReplayStep): void {
    const detail = this.find(runId);
    if (!detail) return;
    const steps = detail.steps.filter((s) => s.id !== step.id);
    steps.push(step);
    steps.sort((a, b) => a.n - b.n || a.at_ms - b.at_ms);
    const list = this.list(detail.run.session_id);
    const index = list.indexOf(detail);
    const tools = steps.filter((s) => s.kind === 'tool').length;
    const run = {
      ...detail.run,
      usage: { ...detail.run.usage, steps: steps.length, tool_calls: tools },
    };
    if (index >= 0) list[index] = { run, steps };
    this.core.emit([{ type: 'AgentStep', session_id: detail.run.session_id, run_id: runId, step }]);
  }

  setState(runId: string, state: RunState, summary: string | null = null): void {
    const detail = this.find(runId);
    if (!detail) return;
    const done = !['running', 'waiting_approval', 'paused'].includes(state);
    this.replaceRun(detail, {
      ...detail.run,
      state,
      summary: summary ?? detail.run.summary,
      finished_at: done ? this.core.isoNow() : null,
    });
  }

  active(sessionId: string): AgentRunDetail | undefined {
    return [...this.list(sessionId)]
      .reverse()
      .find((r) => ['running', 'waiting_approval', 'paused'].includes(r.run.state));
  }

  steer(sessionId: string, text: string): void {
    const active = this.active(sessionId);
    if (!active) throw new Error('Agentka nie wykonuje teraz zadania w tej sesji.');
    const n = active.steps.reduce((max, s) => Math.max(max, s.n), 0);
    this.step(active.run.id, {
      ...replayStep(active.run.id, n, {
        kind: 'steer',
        title: 'Wiadomość w trakcie zadania',
        input: text,
        status: 'ok',
      }),
      id: `${active.run.id}:m${active.steps.length + 1}`,
    });
  }

  markUndone(token: string): void {
    for (const [sid, list] of this.bySession) {
      for (const detail of list) {
        const step = detail.steps.find((s) => s.undo_token === token);
        if (step && !step.undone) {
          this.step(detail.run.id, { ...step, undone: true });
          void sid;
        }
      }
    }
  }

  stepById(stepId: string): ReplayStep | undefined {
    for (const list of this.bySession.values()) {
      for (const detail of list) {
        const found = detail.steps.find((s) => s.id === stepId);
        if (found) return found;
      }
    }
    return undefined;
  }

  workdir(sessionId: string): SessionWorkdir {
    const fallback = sessionId === 's-q3' ? 'C:\\Users\\Ty\\Alfa\\Sesje\\Raport Q3' : null;
    const path = this.workdirs.has(sessionId) ? (this.workdirs.get(sessionId) ?? null) : fallback;
    const title = this.core.session(sessionId)?.title ?? 'Sesja';
    return { path, default_path: `C:\\Users\\Ty\\Alfa\\Sesje\\${title}` };
  }

  chooseWorkdir(sessionId: string, choice: WorkdirChoice): SessionWorkdir {
    const current = this.workdir(sessionId);
    const next =
      choice === 'none'
        ? null
        : choice === 'default'
          ? current.default_path
          : 'C:\\Users\\Ty\\Dokumenty\\Projekt';
    this.workdirs.set(sessionId, next);
    return this.workdir(sessionId);
  }
}
