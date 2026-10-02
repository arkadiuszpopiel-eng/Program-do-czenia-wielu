// Atrapa: „Zdrowie systemu" — Diagnosta (moduły, incydent 401 z propozycją naprawy, naprawa
// z „Cofnij"), Ulepszacz (propozycja R0 z diffem i digestem, propozycja zablokowana przez bramkę
// evali; cykl tylko ręczny — brak portu bezczynności) i zamrożone zestawy evali.
import type { AlfaClient } from '../client';
import type {
  EvalsView,
  HealthOverall,
  HealthProposal,
  HealthRepair,
  HealthView,
  ImproverProposalView,
  ImproverView,
} from '../types-work';
import { fakeHash } from './api-skills';
import type { FakeCore } from './core';

const MODULES = [
  ['core-bus', 'healthy'],
  ['router', 'healthy'],
  ['memory', 'healthy'],
  ['scheduler', 'healthy'],
  ['voice-pipeline', 'degraded'],
  ['tools-window', 'healthy'],
  ['ui-terminal', 'healthy'],
] as const;

export class FakeHealth {
  private pending: HealthProposal[] = [];
  private repaired: HealthRepair[] = [];
  private proposals: ImproverProposalView[] = [];
  private lastCycle: string | null = null;
  private seq = 10;

  constructor(private readonly core: FakeCore) {
    if (core.scenario === 'empty' || core.scenario === 'first-run') return;
    const at = (h: number) => new Date(core.scheduler.now() - h * 3_600_000).toISOString();
    this.pending = [
      {
        id: 1,
        title: 'Wyłącz trasę „anthropic" do czasu ponownego zalogowania',
        diff: ['router.routes.anthropic.enabled: true → false'],
        rationale: 'Dostawca odpowiada 401 od 3 prób — klucz wygasł albo został odwołany.',
        risk: 'low',
        rollback_plan: ['router.routes.anthropic.enabled: false → true'],
        kernel: false,
      },
      {
        id: 3,
        title: 'Przywróć poprzednią wersję modułu „router"',
        diff: ['router 1.0.1 → 1.0.0'],
        rationale: 'Po aktualizacji moduł nie przechodzi kontroli zdrowia (3 restarty w 10 min).',
        risk: 'medium',
        rollback_plan: ['router 1.0.0 → 1.0.1'],
        kernel: true,
      },
    ];
    this.repaired = [
      {
        id: 2,
        title: 'Restart modułu „voice-pipeline"',
        diff: ['restart voice-pipeline'],
        at: at(2),
        undoable: false,
      },
    ];
    const changes = [
      {
        key: 'router.local.max_tokens',
        old: 1024,
        new: 768,
        ring: 'R0',
        safety: 'neutral',
      },
    ];
    this.proposals = [
      this.proposal(1, 'Krótsze odpowiedzi modelu lokalnego', 'awaiting_approval', changes, at(5)),
      {
        ...this.proposal(2, 'Wyższa temperatura Krytyczki', 'sandbox_failed', [], at(30)),
        note: 'Bramka evali: brak wyników replay — propozycja nie przeszła (fail-closed).',
        can_approve: false,
      },
    ];
    this.lastCycle = at(5);
  }

  private proposal(
    id: number,
    title: string,
    stage: string,
    changes: ImproverProposalView['changes'],
    created: string,
  ): ImproverProposalView {
    return {
      id,
      title,
      rationale: 'Średnia długość odpowiedzi przekracza potrzeby w 70% tur (dziennik 7 dni).',
      source: 'local-model',
      ring: 'R0',
      safety: 'neutral',
      stage,
      note: null,
      digest: fakeHash(`${id}:${title}:${JSON.stringify(changes)}`),
      changes,
      created_at: created,
      needs_signature: false,
      can_approve: stage === 'awaiting_approval',
      can_rollback: stage === 'deployed',
    };
  }

  private overall(): HealthOverall {
    if (this.core.scenario === 'empty' || this.core.scenario === 'first-run') return 'ok';
    return this.pending.length > 0 ? 'degraded' : 'ok';
  }

  private view(): HealthView {
    const empty = this.core.scenario === 'empty' || this.core.scenario === 'first-run';
    const at = this.core.isoNow();
    return {
      overall: this.overall(),
      safe_mode: null,
      generated_at: at,
      modules: MODULES.map(([module, health]) => ({
        module,
        version: '1.0.0',
        lifecycle: 'running',
        health,
        detail: health === 'degraded' ? 'Mikrofon zajęty przez inną aplikację.' : null,
      })),
      incidents: empty
        ? []
        : [
            {
              id: 1,
              kind: 'http_auth',
              title: 'Dostawca odrzuca klucz (401)',
              target: 'anthropic',
              count: 3,
              last_at: at,
              status: this.pending.some((p) => p.id === 1) ? 'awaiting_approval' : 'resolved',
            },
          ],
      repaired: this.repaired,
      needs_human: [],
      pending: this.pending,
      problems: [],
    };
  }

  private changed(): HealthView {
    const view = this.view();
    this.core.emit([
      { type: 'HealthChanged', overall: view.overall, pending: view.pending.length },
    ]);
    return view;
  }

  private improverView(): ImproverView {
    return {
      proposals: this.proposals,
      blocked: [],
      issues: [],
      idle_cycle: false,
      last_cycle: this.lastCycle,
    };
  }

  private setStage(id: number, stage: string): ImproverView {
    const found = this.proposals.find((p) => p.id === id);
    if (!found) throw new Error(`Nieznana propozycja ${id}.`);
    this.proposals = this.proposals.map((p) =>
      p.id === id
        ? {
            ...p,
            stage,
            can_approve: false,
            can_rollback: stage === 'deployed',
          }
        : p,
    );
    this.changed();
    return this.improverView();
  }

  api(): AlfaClient['health'] {
    const core = this.core;
    const fail = (message: string) => Promise.reject(new Error(message));
    const evals = (): EvalsView => ({
      available: true,
      reason: null,
      suites: [
        {
          id: 'F4-agents',
          wave: 'F4',
          version: '1.0.0',
          status: 'frozen',
          integrity_ok: true,
          problems: [],
          thresholds: 6,
        },
        {
          id: 'F5-voice',
          wave: 'F5',
          version: '1.0.0',
          status: 'frozen',
          integrity_ok: true,
          problems: [],
          thresholds: 4,
        },
      ],
      holdout_suites: 1,
      verdicts: [],
    });
    return {
      report: () => core.reply(this.view()),
      scan: () => core.reply(this.changed()),
      approve: (repairId) => {
        const p = this.pending.find((x) => x.id === repairId);
        if (!p) return fail(`Nieznana propozycja naprawy ${repairId}.`);
        if (p.kernel) return fail('Naprawę Jądra zatwierdza wyłącznie okno Brokera.');
        this.pending = this.pending.filter((x) => x.id !== repairId);
        this.repaired = [
          { id: ++this.seq, title: p.title, diff: p.diff, at: core.isoNow(), undoable: true },
          ...this.repaired,
        ];
        return core.reply(this.changed());
      },
      reject: (repairId) => {
        if (!this.pending.some((x) => x.id === repairId))
          return fail(`Nieznana propozycja naprawy ${repairId}.`);
        this.pending = this.pending.filter((x) => x.id !== repairId);
        return core.reply(this.changed());
      },
      undo: (repairId) => {
        const r = this.repaired.find((x) => x.id === repairId);
        if (!r?.undoable) return fail('Tej naprawy nie da się cofnąć.');
        this.repaired = this.repaired.map((x) =>
          x.id === repairId ? { ...x, undoable: false, title: `${x.title} — cofnięto` } : x,
        );
        return core.reply(this.changed());
      },
      improver: () => core.reply(this.improverView()),
      improverCycle: () => {
        this.lastCycle = core.isoNow();
        return core.reply(this.improverView());
      },
      improverApprove: (id, digest) => {
        const p = this.proposals.find((x) => x.id === id);
        if (!p?.can_approve) return fail('Tej propozycji nie można zatwierdzić.');
        if (p.digest !== digest) return fail('Diff zmienił się od podglądu — przejrzyj ponownie.');
        return core.reply(this.setStage(id, 'deployed'));
      },
      improverReject: (id) => {
        if (!this.proposals.some((x) => x.id === id)) return fail(`Nieznana propozycja ${id}.`);
        return core.reply(this.setStage(id, 'rejected'));
      },
      improverRollback: (id) => {
        const p = this.proposals.find((x) => x.id === id);
        if (!p?.can_rollback) return fail('Tej propozycji nie można wycofać.');
        return core.reply(this.setStage(id, 'rolled_back'));
      },
      evals: () => core.reply(evals()),
      evalsVerify: (suiteId) => {
        const suite = evals().suites.find((s) => s.id === suiteId);
        return suite ? core.reply(suite) : fail(`Nieznany zestaw ${suiteId}.`);
      },
    };
  }
}
