// Stan atrapy backendu: dane w pamięci, zdarzenia do subskrybentów, scenariusze błędów.
// Wszystko, co wychodzi do UI, jest klonowane (jak serializacja IPC) — UI nie dzieli obiektów z atrapą.
import type { AgentId } from '@alfa/ui-kit';
import type {
  AgentState,
  ArtifactInfo,
  SessionSummary,
  TimelineEvent,
  Turn,
  TurnAnnotation,
} from '../types';
import type { Account } from '../types-hub';
import type { AlfaEvent, LayoutPrefs, SettingValue, SystemStatus } from '../types-system';
import {
  seedAccounts,
  seedArtifacts,
  seedLong,
  seedQ3,
  seedSessions,
  seedSmall,
  seedTimeline,
} from './fixtures';
import { FakeRuns } from './api-agents';
import { realScheduler, type Scheduler } from './scheduler';
import { defaultValues } from './settings-schema';

export type FakeScenario =
  | 'default'
  | 'empty'
  | 'first-run'
  | 'offline'
  | 'rate-limited'
  | 'no-keys'
  | 'no-mic'
  | 'disk-low'
  /** Rdzeń odrzuca `turns_send` (np. błąd zapisu historii) — szkic nie może zginąć. */
  | 'send-error'
  /** Broker: tryb przenośny, zerwanie łącza, kill-switch bez watchdoga, Broker w procesie. */
  | 'broker-portable'
  | 'broker-lost'
  | 'broker-no-watchdog'
  | 'broker-dev';

export const FAKE_SCENARIOS: readonly FakeScenario[] = [
  'default',
  'empty',
  'first-run',
  'offline',
  'rate-limited',
  'no-keys',
  'no-mic',
  'disk-low',
  'send-error',
  'broker-portable',
  'broker-lost',
  'broker-no-watchdog',
  'broker-dev',
];

export interface FakeOptions {
  readonly scenario?: FakeScenario;
  readonly scheduler?: Scheduler;
  /** Prędkość strumienia (domyślnie ~100 tokenów/s, PLAN §14.7). */
  readonly tokensPerSecond?: number;
  /** Sztuczne opóźnienie odpowiedzi komend (np. do pokazania szkieletów > 300 ms). */
  readonly latencyMs?: number;
}

export const STANDARD_CAST: Readonly<Record<AgentId, readonly string[]>> = {
  alfa: ['conductor', 'speaker'],
  beta: ['keeper', 'writer'],
  gama: ['researcher', 'critic', 'thinker'],
  delta: ['operator', 'coder'],
};

const SMALL_TOPICS: Readonly<Record<string, string>> = {
  's-api': 'Dodaj endpoint eksportu zamówień do CSV.',
  's-shop': 'Zrób listę zakupów na weekend dla 4 osób.',
  's-trip': 'Zaplanuj trzy dni w Gdańsku w październiku.',
  's-db': 'Przygotuj plan migracji bazy na PostgreSQL 18.',
  's-old': 'Notatki ze spotkania z 2 września.',
};

export const clone = <T>(value: T): T => structuredClone(value);

export class FakeCore {
  readonly scheduler: Scheduler;
  readonly scenario: FakeScenario;
  readonly tokensPerSecond: number;
  readonly latencyMs: number;
  private readonly listeners = new Set<(batch: readonly AlfaEvent[]) => void>();
  private counter = 0;

  sessions: SessionSummary[];
  readonly turns = new Map<string, Turn[]>();
  readonly annotations: Record<string, TurnAnnotation> = {};
  readonly drafts: Record<string, string> = {};
  readonly agents: Record<string, AgentState[]> = {};
  readonly sessionCost: Record<string, number> = { 's-q3': 108 };
  timeline: TimelineEvent[];
  artifacts: ArtifactInfo[];
  accounts: Account[];
  settings: Record<string, SettingValue>;
  shortcutOverrides: Record<string, string> = {};
  layout: LayoutPrefs | null = null;
  activeSession: string | null;
  onboardingDone: boolean;
  dayCost = 214;
  monthCost = 4_870;
  limit = { enabled: true, monthly: { minor: 30_000, currency: 'PLN' as const } };
  status: SystemStatus;
  /** Przebiegi agentek (Replay) i katalogi robocze sesji. */
  readonly runs: FakeRuns;
  /** Tryb głosowy: rozmowa włączona, wyciszenie. */
  voice = { active: false, muted: false, mode: 'toggle' as 'toggle' | 'ptt' };

  constructor(options: FakeOptions = {}) {
    this.scheduler = options.scheduler ?? realScheduler;
    this.scenario = options.scenario ?? 'default';
    this.tokensPerSecond = options.tokensPerSecond ?? 100;
    this.latencyMs = options.latencyMs ?? 0;
    const now = this.scheduler.now();
    const empty = this.scenario === 'empty' || this.scenario === 'first-run';
    this.sessions = empty ? [] : seedSessions(now);
    this.timeline = empty ? [] : seedTimeline(now);
    this.artifacts = empty ? [] : seedArtifacts(now);
    const noKeys = this.scenario === 'no-keys' || this.scenario === 'first-run';
    this.accounts = noKeys ? [] : seedAccounts(now);
    this.settings = defaultValues();
    this.activeSession = empty ? null : 's-q3';
    this.onboardingDone = this.scenario !== 'first-run';
    this.status = {
      online: this.scenario !== 'offline',
      queued_messages: 0,
      rate_limit:
        this.scenario === 'rate-limited'
          ? { provider: 'Anthropic', resets_at: new Date(now + 3 * 60_000).toISOString() }
          : null,
      keys_configured: !noKeys,
      profile: noKeys ? 'local' : 'hybrid',
      mic: this.scenario === 'no-mic' ? 'denied' : 'ok',
      disk:
        this.scenario === 'disk-low'
          ? { free_bytes: 1_800_000_000, low: true }
          : { free_bytes: 212_000_000_000, low: false },
    };
    if (this.status.profile === 'local') this.settings['models.default_profile'] = 'local';
    this.runs = new FakeRuns(this);
  }

  nextId(prefix: string): string {
    this.counter++;
    return `${prefix}-${this.scheduler.now().toString(36)}-${this.counter}`;
  }

  isoNow(): string {
    return new Date(this.scheduler.now()).toISOString();
  }

  reply<T>(value: T): Promise<T> {
    const copy = clone(value);
    if (this.latencyMs <= 0) return Promise.resolve(copy);
    return new Promise((resolve) => this.scheduler.setTimeout(() => resolve(copy), this.latencyMs));
  }

  subscribe(listener: (batch: readonly AlfaEvent[]) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  emit(events: readonly AlfaEvent[]): void {
    if (events.length === 0 || this.listeners.size === 0) return;
    const batch = clone(events);
    queueMicrotask(() => {
      for (const listener of this.listeners) listener(batch);
    });
  }

  dispose(): void {
    this.listeners.clear();
  }

  session(id: string): SessionSummary | undefined {
    return this.sessions.find((s) => s.id === id);
  }

  updateSession(id: string, patch: Partial<SessionSummary>): SessionSummary | undefined {
    const index = this.sessions.findIndex((s) => s.id === id);
    const current = this.sessions[index];
    if (!current) return undefined;
    const next = { ...current, ...patch };
    this.sessions[index] = next;
    this.emit([{ type: 'SessionUpdated', session: next }]);
    return next;
  }

  turnsOf(sessionId: string): Turn[] {
    let list = this.turns.get(sessionId);
    if (!list) {
      const now = this.scheduler.now();
      const topic = SMALL_TOPICS[sessionId];
      list =
        sessionId === 's-q3'
          ? seedQ3(now)
          : sessionId === 's-long'
            ? seedLong(now)
            : topic
              ? seedSmall(sessionId, now, topic)
              : [];
      this.turns.set(sessionId, list);
    }
    return list;
  }

  findTurn(turnId: string): Turn | undefined {
    for (const list of this.turns.values()) {
      const found = list.find((t) => t.id === turnId);
      if (found) return found;
    }
    return undefined;
  }

  agentsOf(sessionId: string): AgentState[] {
    let list = this.agents[sessionId];
    if (!list) {
      list = (Object.keys(STANDARD_CAST) as AgentId[]).map((id) => ({
        id,
        role_ids: STANDARD_CAST[id],
        status: 'idle',
        activity: null,
      }));
      this.agents[sessionId] = list;
    }
    return list;
  }

  setAgent(sessionId: string, agent: AgentId, patch: Partial<AgentState>): void {
    const list = this.agentsOf(sessionId).map((a) => (a.id === agent ? { ...a, ...patch } : a));
    this.agents[sessionId] = list;
    this.emit([{ type: 'AgentsChanged', session_id: sessionId, agents: list }]);
  }

  setStatus(patch: Partial<SystemStatus>): void {
    this.status = { ...this.status, ...patch };
    this.emit([{ type: 'SystemStatusChanged', status: this.status }]);
  }
}
