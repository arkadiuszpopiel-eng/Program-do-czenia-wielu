// Atrapa: sesje i tury (append-only, gałęzie, strumień, kolejka offline, 429).
import type { AgentId } from '@alfa/ui-kit';
import type { AlfaClient } from '../client';
import type { Turn, TurnError, TurnUsage } from '../types';
import { TEMPLATE_TITLES } from './api-sessions';
import { STANDARD_CAST, type FakeCore } from './core';
import { session as makeSession, turn as makeTurn } from './fixtures';
import { pickResponse, quickResponse, type ResponseScript } from './responses';
import { FakeStreamer } from './stream';
import { takeStaged } from './api-files';
import type { TurnAttachment } from '../types-files';

export class FakeChat {
  readonly streamer: FakeStreamer;

  constructor(private readonly core: FakeCore) {
    this.streamer = new FakeStreamer({
      scheduler: core.scheduler,
      tokensPerSecond: core.tokensPerSecond,
      emit: (events) => core.emit(events),
      runs: core.runs,
      finished: (turn, agent, usage) => this.finished(turn, agent, usage),
    });
  }

  private finished(turn: Turn, agent: AgentId, usage: TurnUsage | null): void {
    const { core } = this;
    const sid = turn.session_id;
    core.setAgent(sid, agent, { status: 'idle', activity: null });
    core.updateSession(sid, { working: false, updated_at: core.isoNow() });
    if (!usage) return;
    core.sessionCost[sid] = (core.sessionCost[sid] ?? 0) + usage.cost.minor;
    core.dayCost += usage.cost.minor;
    core.monthCost += usage.cost.minor;
    const event = {
      id: core.nextId('ev'),
      ts: core.isoNow(),
      session_id: sid,
      kind: 'model_call' as const,
      level: 'info' as const,
      agent,
      title: `${usage.model} · ${usage.input_tokens} → ${usage.output_tokens}`,
      detail: null,
      cost: usage.cost,
      latency_ms: usage.latency_ms,
      turn_id: turn.id,
    };
    core.timeline.push(event);
    core.emit([
      { type: 'TimelineAppended', event },
      { type: 'CostsChanged', session_id: sid, costs: this.costs(sid) },
    ]);
  }

  costs(sessionId: string | null) {
    const { core } = this;
    const turns = sessionId ? core.turnsOf(sessionId) : [];
    const used = turns.reduce((sum, t) => sum + t.text.length / 3.5, 0);
    return {
      session: {
        minor: sessionId ? (core.sessionCost[sessionId] ?? 0) : 0,
        currency: 'PLN' as const,
      },
      day: { minor: core.dayCost, currency: 'PLN' as const },
      month: { minor: core.monthCost, currency: 'PLN' as const },
      limit: core.limit,
      context: {
        used_tokens: Math.round(used) + 2400,
        max_tokens: 200_000,
        compacted: sessionId === 's-long',
      },
      fx: { usd_pln: 3.64, date: core.isoNow().slice(0, 10), stale: false },
    };
  }

  private failure(): TurnError | null {
    const { status } = this.core;
    if (status.rate_limit) {
      return {
        code: 'rate_limited',
        message: `Limit zapytań u dostawcy ${status.rate_limit.provider}.`,
        retry_at: status.rate_limit.resets_at,
        provider: status.rate_limit.provider,
      };
    }
    return null;
  }

  /** Dopisuje turę agentki i uruchamia strumień. */
  respond(
    sessionId: string,
    parentId: string,
    script: ResponseScript,
    continues: string | null = null,
  ): string {
    const { core } = this;
    const turn = makeTurn(
      core.nextId('t'),
      sessionId,
      parentId,
      script.agent,
      '',
      core.scheduler.now(),
      {
        role_id: script.role_id,
        status: 'streaming',
        blocks: [],
        continues,
      },
    );
    core.turnsOf(sessionId).push(turn);
    core.emit([{ type: 'TurnAppended', session_id: sessionId, turn }]);
    core.setAgent(sessionId, script.agent, { status: 'speaking', activity: null });
    core.updateSession(sessionId, { working: true });
    this.streamer.start(turn, script, this.failure());
    return turn.id;
  }

  appendUser(
    sessionId: string,
    parentId: string | null,
    text: string,
    addressed: AgentId | null,
    attachments: readonly TurnAttachment[] = [],
  ): Turn {
    const { core } = this;
    const queued = !core.status.online;
    const turn = makeTurn(
      core.nextId('t'),
      sessionId,
      parentId,
      'user',
      text,
      core.scheduler.now(),
      {
        addressed_to: addressed,
        status: queued ? 'queued' : 'complete',
        ...(attachments.length ? { attachments } : {}),
      },
    );
    core.turnsOf(sessionId).push(turn);
    core.emit([{ type: 'TurnAppended', session_id: sessionId, turn }]);
    if (queued) core.setStatus({ queued_messages: core.status.queued_messages + 1 });
    const current = core.session(sessionId);
    if (current && current.title === TEMPLATE_TITLES.empty) {
      core.updateSession(sessionId, { title: text.slice(0, 40).trim() || current.title });
    }
    return turn;
  }

  variantCount(sessionId: string, parentId: string): number {
    return this.core.turnsOf(sessionId).filter((t) => t.parent_id === parentId).length;
  }

  turnsApi(): AlfaClient['turns'] {
    const { core } = this;
    return {
      list: (sessionId) =>
        core.reply({ turns: core.turnsOf(sessionId), annotations: core.annotations }),
      send: (sessionId, options) => {
        if (core.scenario === 'send-error') {
          return Promise.reject(
            new Error('Nie udało się zapisać wiadomości (atrapa: send-error).'),
          );
        }
        let attachments;
        try {
          attachments = takeStaged(core, sessionId, options.attachments);
        } catch (error) {
          return Promise.reject(error instanceof Error ? error : new Error(String(error)));
        }
        const user = this.appendUser(
          sessionId,
          options.parent_id,
          options.text,
          options.addressed_to,
          attachments,
        );
        const assistant =
          user.status === 'queued'
            ? null
            : this.respond(sessionId, user.id, pickResponse(options.text, options.addressed_to, 0));
        return core.reply({ user_turn_id: user.id, assistant_turn_id: assistant });
      },
      regenerate: (sessionId, turnId) => {
        const old = core.findTurn(turnId);
        const parent = old?.parent_id ? core.findTurn(old.parent_id) : undefined;
        if (!old || !parent) return Promise.reject(new Error('Brak tury do ponowienia'));
        const variant = this.variantCount(sessionId, parent.id);
        const addressed = old.author === 'user' ? null : old.author;
        return core.reply(
          this.respond(sessionId, parent.id, pickResponse(parent.text, addressed, variant)),
        );
      },
      editAndResend: (sessionId, turnId, text) => {
        const old = core.findTurn(turnId);
        const user = this.appendUser(
          sessionId,
          old?.parent_id ?? null,
          text,
          old?.addressed_to ?? null,
        );
        const assistant =
          user.status === 'queued'
            ? null
            : this.respond(sessionId, user.id, pickResponse(text, user.addressed_to, 0));
        return core.reply({ user_turn_id: user.id, assistant_turn_id: assistant });
      },
      continueTurn: (sessionId, turnId) => {
        const old = core.findTurn(turnId);
        const agent = old && old.author !== 'user' ? old.author : 'alfa';
        const script: ResponseScript = {
          agent,
          role_id: old?.role_id ?? 'conductor',
          text: '…co w praktyce oznacza wpływy 30–45 dni później niż w segmencie detalicznym. Dlatego w rekomendacjach proponuję rezerwę płynności na dwa miesiące.',
          thinkingMs: 0,
          tools: [],
          approval: null,
          truncated: false,
        };
        return core.reply(this.respond(sessionId, turnId, script, turnId));
      },
      stop: (sessionId) => core.reply(void this.streamer.cancel(sessionId)),
      rate: (turnId, rating) => {
        core.annotations[turnId] = { hidden: core.annotations[turnId]?.hidden ?? false, rating };
        return core.reply(undefined);
      },
      setHidden: (turnId, hidden) => {
        core.annotations[turnId] = { rating: core.annotations[turnId]?.rating ?? null, hidden };
        return core.reply(undefined);
      },
      remember: () => core.reply(undefined),
      readAloud: () => core.reply(undefined),
      saveCode: () => core.reply(undefined),
      runCode: () =>
        core.reply({ status: 'opened_broker' as const, request_id: core.nextId('br') }),
      undoStep: (token) => {
        core.runs.markUndone(token);
        return core.reply(undefined);
      },
    };
  }

  /** Szybkie pytanie: jedna wspólna sesja „Szybkie pytania". */
  quickApi(): AlfaClient['quick'] {
    const { core } = this;
    return {
      ask: (text) => {
        let quick = core.sessions.find((s) => s.id === 's-quick');
        if (!quick) {
          quick = makeSession('s-quick', 'Szybkie pytania', core.scheduler.now());
          core.sessions.unshift(quick);
          core.emit([{ type: 'SessionUpdated', session: quick }]);
        }
        const list = core.turnsOf('s-quick');
        const user = this.appendUser('s-quick', list[list.length - 1]?.id ?? null, text, null);
        const assistant =
          user.status === 'queued' ? null : this.respond('s-quick', user.id, quickResponse(text));
        return core.reply({
          session_id: 's-quick',
          user_turn_id: user.id,
          assistant_turn_id: assistant,
        });
      },
      expandToMain: (sessionId) => {
        core.activeSession = sessionId;
        return core.reply(undefined);
      },
      hide: () => core.reply(undefined),
    };
  }

  /** Po powrocie online: wysyła wiadomości z kolejki. */
  flushQueue(): void {
    const { core } = this;
    for (const [sid, list] of core.turns) {
      for (const turn of list.filter((t) => t.status === 'queued')) {
        turn.status = 'complete';
        core.emit([{ type: 'TurnStatus', session_id: sid, turn_id: turn.id, status: 'complete' }]);
        this.respond(sid, turn.id, pickResponse(turn.text, turn.addressed_to, 0));
      }
    }
    core.setStatus({ queued_messages: 0 });
  }

  castFor(
    template: 'standard' | 'solo' | 'coding' | 'research',
  ): Record<AgentId, readonly string[]> {
    switch (template) {
      case 'solo':
        return {
          alfa: ['conductor', 'speaker', 'researcher', 'operator', 'writer'],
          beta: [],
          gama: [],
          delta: [],
        };
      case 'coding':
        return {
          alfa: ['speaker'],
          beta: ['keeper'],
          gama: ['critic'],
          delta: ['conductor', 'coder'],
        };
      case 'research':
        return {
          alfa: ['speaker'],
          beta: ['writer'],
          gama: ['conductor', 'researcher'],
          delta: ['critic'],
        };
      default:
        return { ...STANDARD_CAST };
    }
  }
}
