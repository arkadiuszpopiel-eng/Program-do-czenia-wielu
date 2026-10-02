<!--
  Replay krok po kroku (makieta 7): przebiegi agentek w sesji — krok, narzędzie, wejście/wyjście
  w skrócie (zwykły tekst), status (ikona + tekst), czas, „Cofnij krok", „Otwórz terminal".
  Odtwarzanie: Od początku / Poprzedni / Następny krok / Wszystkie.
-->
<script lang="ts">
  import { Avatar, Button } from '@alfa/ui-kit';
  import Brain from '@lucide/svelte/icons/brain';
  import ListChecks from '@lucide/svelte/icons/list-checks';
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import ShieldCheck from '@lucide/svelte/icons/shield-check';
  import Wrench from '@lucide/svelte/icons/wrench';
  import Undo2 from '@lucide/svelte/icons/undo-2';
  import SquareTerminal from '@lucide/svelte/icons/square-terminal';
  import type { AgentRunDetail, ReplayStep } from '../../api/types';
  import {
    applyRun,
    applyStep,
    markUndone,
    statusTone,
    stepCursor,
    visibleSteps,
  } from '../../logic/replay';
  import { agentName } from '../../logic/work';
  import { useApp } from '../../state/context';

  interface Props {
    sessionId: string;
  }

  let { sessionId }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const ICONS = {
    plan: ListChecks,
    think: Brain,
    tool: Wrench,
    verify: ShieldCheck,
    steer: MessageSquare,
  };

  let runs = $state<AgentRunDetail[]>([]);
  let selected = $state<string | null>(null);
  let cursor = $state<number | null>(null);

  $effect(() => {
    void app.client.agents.runs(sessionId).then((list) => {
      runs = [...list];
      selected = list.filter((r) => !r.run.parent_id).at(-1)?.run.id ?? list.at(-1)?.run.id ?? null;
      cursor = null;
    });
  });

  $effect(() =>
    app.on((event) => {
      if (event.type === 'AgentRunUpdated' && event.session_id === sessionId) {
        const known = runs.some((r) => r.run.id === event.run.id);
        runs = applyRun(runs, event.run);
        if (!known && !event.run.parent_id) selected = event.run.id;
      } else if (event.type === 'AgentStep' && event.session_id === sessionId) {
        runs = applyStep(runs, event.run_id, event.step);
      }
    }),
  );

  const current = $derived(runs.find((r) => r.run.id === selected) ?? runs.at(-1) ?? null);
  /** Podprzebiegi bieżącego przebiegu (delegacja, Krytyczka, umiejętność). */
  const children = $derived(current ? runs.filter((r) => r.run.parent_id === current.run.id) : []);
  const parent = $derived(
    current?.run.parent_id ? (runs.find((r) => r.run.id === current.run.parent_id) ?? null) : null,
  );

  function runLabel(r: AgentRunDetail): string {
    const who = agentName(r.run.agent);
    return r.run.parent_id ? `↳ ${r.run.label ?? who} (${who})` : who;
  }
  const steps = $derived(current ? visibleSteps(current.steps, cursor) : []);
  const total = $derived(current?.steps.length ?? 0);

  function isUndone(step: ReplayStep): boolean {
    return app.runs.isUndone(step.undo_token, step.undone);
  }

  async function undo(step: ReplayStep) {
    if (!step.undo_token) return;
    await app.undoStep(step.undo_token, step.output || step.title);
    runs = markUndone(runs, step.undo_token);
  }

  async function terminal(step: ReplayStep) {
    try {
      await app.client.agents.openTerminal(step.id);
    } catch (error) {
      app.toasts.show({
        kind: 'error',
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }
</script>

<div class="replay">
  {#if runs.length === 0}
    <p class="empty">{t('replay.empty')}</p>
  {:else}
    {#if runs.length > 1}
      <label class="pick">
        <span>{t('replay.run')}</span>
        <select bind:value={selected} onchange={() => (cursor = null)}>
          {#each [...runs].reverse() as r (r.run.id)}
            <option value={r.run.id}
              >{runLabel(r)}: {r.run.goal.slice(0, 48)} ({app.i18n.time(r.run.started_at)})</option
            >
          {/each}
        </select>
      </label>
    {/if}
    {#if current}
      {@const run = current.run}
      <header class="head">
        <Avatar agent={run.agent} size={24} />
        <div class="goal">
          <p class="title">{run.goal}</p>
          <p class="meta">
            <span class="state state-{run.state}">{t(`replay.state.${run.state}`)}</span>
            · {t('replay.steps', { n: run.usage.steps, max: run.budget.max_steps })}
            · {app.i18n.duration(run.usage.elapsed_ms)}
            {#if run.usage.cost.minor > 0}· {app.i18n.money(run.usage.cost)}{/if}
          </p>
          {#if run.workdir}<p class="meta">{t('replay.workdir', { path: run.workdir })}</p>{/if}
          {#if parent}
            <p class="meta">
              {t('replay.subrun', { label: run.label ?? agentName(run.agent) })}
              <button type="button" class="link" onclick={() => (selected = parent.run.id)}
                >{t('replay.toParent')}</button
              >
            </p>
          {/if}
        </div>
      </header>
      {#if children.length}
        <ul class="subruns" aria-label={t('replay.subruns')}>
          {#each children as c (c.run.id)}
            <li>
              <button type="button" class="link" onclick={() => (selected = c.run.id)}
                >↳ {c.run.label ?? agentName(c.run.agent)}</button
              >
              <span class="meta">{agentName(c.run.agent)} · {t(`replay.state.${c.run.state}`)}</span
              >
            </li>
          {/each}
        </ul>
      {/if}
      <div class="controls" role="group" aria-label={t('replay.controls')}>
        <Button size="sm" variant="ghost" onclick={() => (cursor = 0)} disabled={total === 0}
          >{t('replay.fromStart')}</Button
        >
        <Button
          size="sm"
          variant="ghost"
          onclick={() => (cursor = stepCursor(cursor, -1, total))}
          disabled={cursor === 0 || total === 0}>{t('replay.prev')}</Button
        >
        <Button
          size="sm"
          variant="ghost"
          onclick={() => (cursor = stepCursor(cursor, 1, total))}
          disabled={cursor === null}>{t('replay.next')}</Button
        >
        <Button size="sm" variant="ghost" onclick={() => (cursor = null)} disabled={cursor === null}
          >{t('replay.all')}</Button
        >
      </div>
      <p class="position" aria-live="polite">
        {cursor === null
          ? t('replay.showingAll', { n: total })
          : t('replay.position', { n: cursor + 1, total })}
      </p>
      <ol class="steps" aria-label={t('replay.list')}>
        {#each steps as step, i (step.id)}
          {@const Icon = ICONS[step.kind]}
          {@const tone = statusTone(step.status)}
          <li
            class="step"
            class:current={cursor === i}
            aria-current={cursor === i ? 'step' : undefined}
          >
            <span class="n" aria-hidden="true">{step.n}</span>
            <span class="icon" aria-hidden="true"><Icon size={14} strokeWidth={1.5} /></span>
            <div class="body">
              <p class="line">
                <span class="name">{step.title}</span>
                {#if step.tool}<code class="tool">{step.tool}</code>{/if}
                <span class="status tone-{tone}">{t(`replay.status.${step.status}`)}</span>
                {#if step.duration_ms !== null}<span class="time"
                    >{app.i18n.duration(step.duration_ms)}</span
                  >{/if}
              </p>
              {#if step.untrusted}<p class="untrusted">{t('replay.untrusted')}</p>{/if}
              {#if step.input || step.output}
                <details open={cursor === i}>
                  <summary>{t('replay.io')}</summary>
                  {#if step.input}<p class="io"><b>{t('replay.input')}</b> {step.input}</p>{/if}
                  {#if step.output}<p class="io"><b>{t('replay.output')}</b> {step.output}</p>{/if}
                </details>
              {/if}
              <div class="actions">
                {#if step.undo_token}
                  {#if isUndone(step)}
                    <span class="undone">{t('replay.undone')}</span>
                  {:else}
                    <Button size="sm" variant="secondary" onclick={() => void undo(step)}>
                      <Undo2 size={12} strokeWidth={1.5} aria-hidden="true" />
                      {t('replay.undo')}
                    </Button>
                  {/if}
                {/if}
                {#if step.intent?.kind === 'open_in_terminal'}
                  <Button size="sm" variant="secondary" onclick={() => void terminal(step)}>
                    <SquareTerminal size={12} strokeWidth={1.5} aria-hidden="true" />
                    {t('intent.open')}
                  </Button>
                {/if}
              </div>
            </div>
          </li>
        {/each}
      </ol>
    {/if}
  {/if}
</div>

<style>
  .replay {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    font-size: var(--alfa-font-size-sm);
  }
  .empty,
  .position,
  .meta {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .subruns {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding-left: var(--alfa-space-6);
    list-style: none;
    font-size: var(--alfa-font-size-xs);
  }
  .link {
    padding: 0 var(--alfa-space-1);
    border: 0;
    background: transparent;
    color: var(--alfa-color-text);
    font: inherit;
    text-decoration: underline;
    cursor: pointer;
  }
  .pick {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: var(--alfa-font-size-xs);
  }
  .pick select {
    min-height: 28px;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
  }
  .head {
    display: flex;
    gap: var(--alfa-space-2);
  }
  .goal {
    min-width: 0;
  }
  .title {
    font-weight: 600;
  }
  .controls {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-1);
  }
  .steps {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .step {
    display: flex;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-2) 0;
    border-bottom: 1px solid var(--alfa-color-border);
  }
  .step.current {
    background: var(--alfa-color-surface2);
  }
  .n {
    min-width: 1.5em;
    color: var(--alfa-color-text-subtle);
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  .icon {
    padding-top: 2px;
    color: var(--alfa-color-text-muted);
  }
  .body {
    flex: 1;
    min-width: 0;
  }
  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-1);
  }
  .tool {
    font-size: var(--alfa-font-size-xs);
  }
  .status {
    padding: 0 6px;
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface2);
    font-size: var(--alfa-font-size-xs);
  }
  .tone-ok {
    color: var(--alfa-color-success);
  }
  .tone-warn {
    color: var(--alfa-color-warning);
  }
  .tone-error {
    color: var(--alfa-color-error);
  }
  .tone-busy {
    color: var(--alfa-color-info);
  }
  .time,
  .undone,
  .untrusted {
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
  }
  .io {
    overflow-wrap: anywhere;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  details summary {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    cursor: pointer;
  }
  .actions {
    display: flex;
    gap: var(--alfa-space-1);
    margin-top: 4px;
  }
</style>
