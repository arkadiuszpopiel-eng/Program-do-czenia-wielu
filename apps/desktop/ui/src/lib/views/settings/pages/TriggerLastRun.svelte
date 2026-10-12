<!--
  Ostatni wynik wyzwalacza (plan P14): kiedy ruszył, jak się skończyło (słowem, kolor to dodatek)
  i raport agentki do rozwinięcia. Bez uruchomień — jedno zdanie, że jeszcze nie ruszał.
-->
<script lang="ts">
  import type { LastRun } from '../../../logic/trigger-runs';
  import { useApp } from '../../../state/context';

  interface Props {
    run: LastRun | undefined;
    /** Identyfikator wyzwalacza — do unikalnych identyfikatorów w drzewie. */
    triggerId: string;
  }

  let { run, triggerId }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  let open = $state(false);

  const status = $derived.by(() => {
    if (!run) return '';
    if (run.outcome !== 'submitted') return tk(`triggers.outcome.${run.outcome}`);
    if (!run.task) return t('triggers.last.noTask');
    if (run.task.state !== 'done') return tk(`tasks.state.${run.task.state}`);
    return run.task.result ? tk(`tasks.result.${run.task.result}`) : tk('tasks.state.done');
  });
  const report = $derived(run?.task?.summary?.trim() ?? '');
  const problem = $derived(run?.task?.error ?? run?.detail ?? '');
</script>

<div class="last" data-testid="trigger-last-run" data-tone={run?.tone ?? 'none'}>
  {#if !run}
    <span class="meta">{t('triggers.last.never')}</span>
  {:else}
    <span class="meta">{t('triggers.last.when', { when: app.i18n.dateTime(run.at) })}</span>
    <span class="status {run.tone}"><span class="dot" aria-hidden="true"></span>{status}</span>
    {#if problem}<span class="meta problem">{problem}</span>{/if}
    {#if report}
      <button
        type="button"
        class="toggle"
        aria-expanded={open}
        aria-controls="trg-report-{triggerId}"
        onclick={() => (open = !open)}
      >
        {open ? t('triggers.last.hide') : t('triggers.last.show')}
      </button>
    {/if}
  {/if}
</div>
{#if run && report && open}
  <pre class="report" id="trg-report-{triggerId}">{report}</pre>
{/if}

<style>
  .last {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-2) var(--alfa-space-3);
    font-size: var(--alfa-font-size-sm);
  }
  .meta {
    color: var(--alfa-color-text-muted);
  }
  .problem {
    overflow-wrap: anywhere;
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-weight: var(--alfa-weight-semibold);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--alfa-color-text-muted);
  }
  .ok .dot {
    background: var(--alfa-color-success);
  }
  .running .dot {
    background: var(--alfa-color-focus);
  }
  .bad .dot {
    background: var(--alfa-color-error);
  }
  .toggle {
    padding: 2px var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text);
    font: inherit;
    font-size: var(--alfa-font-size-sm);
    cursor: pointer;
  }
  .toggle:hover {
    background: var(--alfa-color-surface2);
  }
  .report {
    max-height: 320px;
    margin: 0;
    padding: var(--alfa-space-3);
    overflow: auto;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface2);
    font-family: inherit;
    font-size: var(--alfa-font-size-sm);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
