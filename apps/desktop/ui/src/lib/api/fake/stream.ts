// Skryptowany strumień odpowiedzi atrapy: myślenie → kroki narzędzi → (karta zatwierdzenia) →
// tekst ~100 tokenów/s → Usage → Stop. Anulowanie (`Stop`) w dowolnym momencie.
import type { AgentId } from '@alfa/ui-kit';
import type { RenderedBlock, Turn, TurnError, TurnUsage } from '../types';
import type { AlfaEvent } from '../types-system';
import { changedBlocks, renderBlocks, tokenize } from './render';
import type { ResponseScript } from './responses';
import type { Scheduler } from './scheduler';

export interface StreamHost {
  readonly scheduler: Scheduler;
  readonly tokensPerSecond: number;
  emit(events: readonly AlfaEvent[]): void;
  /** Wywoływane po zakończeniu (także błędem / anulowaniem). */
  finished(turn: Turn, agent: AgentId, usage: TurnUsage | null): void;
}

interface Active {
  readonly turn: Turn;
  readonly agent: AgentId;
  timer: number | null;
  stop: (reason: 'cancelled') => void;
}

export class FakeStreamer {
  private readonly active = new Map<string, Active>();

  constructor(private readonly host: StreamHost) {}

  isActive(sessionId: string): boolean {
    return this.active.has(sessionId);
  }

  cancel(sessionId: string): boolean {
    const entry = this.active.get(sessionId);
    if (!entry) return false;
    entry.stop('cancelled');
    return true;
  }

  start(turn: Turn, script: ResponseScript, failure: TurnError | null): void {
    const { scheduler, tokensPerSecond } = this.host;
    const sid = turn.session_id;
    const tid = turn.id;
    const started = scheduler.now();
    const tokens = tokenize(script.text);
    let tokenIndex = 0;
    let blocks: RenderedBlock[] = [];
    let phase: 'thinking' | 'tools' | 'text' | 'done' = 'thinking';
    let toolIndex = 0;
    const entry: Active = {
      turn,
      agent: script.agent,
      timer: null,
      stop: () => finish('cancelled'),
    };
    this.active.set(sid, entry);

    const at = (ms: number, fn: () => void): void => {
      entry.timer = scheduler.setTimeout(() => {
        entry.timer = null;
        fn();
      }, ms);
    };
    const emit = (...events: AlfaEvent[]): void => this.host.emit(events);

    const activity = (description: string, step: number, total: number): void =>
      emit({
        type: 'ActivityChanged',
        session_id: sid,
        activity: {
          session_id: sid,
          agent: script.agent,
          description,
          step,
          total_steps: total,
          started_at: new Date(started).toISOString(),
        },
      });

    const finish = (reason: 'end' | 'max_tokens' | 'cancelled' | 'error'): void => {
      if (phase === 'done') return;
      phase = 'done';
      if (entry.timer !== null) scheduler.clearTimeout(entry.timer);
      this.active.delete(sid);
      if (turn.thinking?.active) turn.thinking = { ...turn.thinking, active: false };
      let usage: TurnUsage | null = null;
      if (reason === 'error' && failure) {
        turn.status = 'error';
        turn.error = failure;
        emit({ type: 'Error', session_id: sid, turn_id: tid, error: failure });
      } else {
        turn.status = reason === 'cancelled' ? 'cancelled' : 'complete';
        turn.truncated = reason === 'max_tokens';
        const final = renderBlocks(turn.text, false);
        const changed = changedBlocks(blocks, final);
        turn.blocks = final;
        if (changed.length)
          emit({ type: 'TextDelta', session_id: sid, turn_id: tid, text: '', blocks: changed });
        const out = Math.max(1, tokenIndex);
        usage = {
          input_tokens: 1200 + turn.text.length,
          output_tokens: out,
          cost: { minor: Math.max(1, Math.round(out / 20)), currency: 'PLN' },
          latency_ms: scheduler.now() - started,
          provider: 'Anthropic',
          model: 'claude-opus-5-5',
        };
        turn.usage = usage;
        emit({ type: 'Usage', session_id: sid, turn_id: tid, usage });
        emit({
          type: 'Stop',
          session_id: sid,
          turn_id: tid,
          reason: reason === 'error' ? 'end' : reason,
        });
      }
      emit({ type: 'ActivityChanged', session_id: sid, activity: null });
      this.host.finished(turn, script.agent, usage);
    };

    const nextToken = (): void => {
      if (failure && tokenIndex >= (failure.code === 'provider' ? 3 : 0)) {
        finish('error');
        return;
      }
      const token = tokens[tokenIndex++];
      if (token === undefined) {
        finish(script.truncated ? 'max_tokens' : 'end');
        return;
      }
      turn.text += token;
      const next = renderBlocks(turn.text, true);
      const changed = changedBlocks(blocks, next);
      blocks = next;
      turn.blocks = next;
      emit({ type: 'TextDelta', session_id: sid, turn_id: tid, text: token, blocks: changed });
      at(1000 / tokensPerSecond, nextToken);
    };

    const nextTool = (): void => {
      const tool = script.tools[toolIndex];
      if (!tool) {
        if (script.approval) {
          const approval = { ...script.approval, id: `ap-${tid}`, status: 'pending' as const };
          turn.approval = approval;
          emit({ type: 'ApprovalPending', session_id: sid, turn_id: tid, approval });
        }
        phase = 'text';
        nextToken();
        return;
      }
      const id = `${tid}-t${toolIndex}`;
      const running = {
        id,
        icon: tool.icon,
        label: tool.label,
        status: 'running' as const,
        duration_ms: null,
        undo_token: null,
      };
      turn.tools = [...turn.tools, running];
      emit({ type: 'ToolCall', session_id: sid, turn_id: tid, step: running });
      activity(tool.label, toolIndex + 1, script.tools.length);
      at(tool.ms, () => {
        const done = {
          ...running,
          status: 'done' as const,
          duration_ms: tool.ms,
          undo_token: tool.undo ? `${sid}:u${toolIndex + 1}` : null,
        };
        turn.tools = turn.tools.map((s) => (s.id === id ? done : s));
        emit({ type: 'ToolCall', session_id: sid, turn_id: tid, step: done });
        toolIndex++;
        nextTool();
      });
    };

    const think = (elapsed: number): void => {
      if (elapsed >= script.thinkingMs) {
        turn.thinking = { duration_ms: script.thinkingMs, active: false };
        emit({
          type: 'ThinkingDelta',
          session_id: sid,
          turn_id: tid,
          elapsed_ms: script.thinkingMs,
          done: true,
        });
        phase = 'tools';
        nextTool();
        return;
      }
      turn.thinking = { duration_ms: elapsed, active: true };
      emit({
        type: 'ThinkingDelta',
        session_id: sid,
        turn_id: tid,
        elapsed_ms: elapsed,
        done: false,
      });
      at(250, () => think(elapsed + 250));
    };

    if (script.thinkingMs > 0) think(0);
    else {
      phase = 'tools';
      nextTool();
    }
  }
}
