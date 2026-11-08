<!--
  Karta „Cofnij" pod odpowiedzią agentki: cofalne kroki narzędzi (np. „Delta: przeniesiono
  14 plików · 2,1 s · Cofnij") — jedno kliknięcie = krok dziennika cofania w rdzeniu (PLAN §14.8).
-->
<script lang="ts">
  import Undo2 from '@lucide/svelte/icons/undo-2';
  import type { ToolStep } from '../../api/types';
  import { useApp } from '../../state/context';

  interface Props {
    steps: readonly ToolStep[];
  }

  let { steps }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const titleId = $props.id();
  const undoable = $derived(
    steps.filter(
      (s) => s.undo_token && s.status === 'done' && !app.runs.isUndone(s.undo_token, s.undone),
    ),
  );
</script>

{#if undoable.length}
  <section class="undo-card" aria-labelledby={titleId}>
    <h4 id={titleId} class="title">{t('undo.cardTitle', { n: undoable.length })}</h4>
    <ul class="list">
      {#each undoable as step (step.id)}
        <li class="row">
          <span class="label">{step.label}</span>
          {#if step.duration_ms !== null}
            <span class="time">· {app.i18n.duration(step.duration_ms)}</span>
          {/if}
          <button
            type="button"
            class="undo"
            aria-label={t('undo.stepLabel', { label: step.label })}
            onclick={() => step.undo_token && void app.undoStep(step.undo_token, step.label)}
          >
            <Undo2 size={12} strokeWidth={1.5} aria-hidden="true" />
            {t('common.undo')}
          </button>
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .undo-card {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border: 1px dashed var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    font-size: var(--alfa-font-size-sm);
  }
  .title {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    font-weight: 600;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    min-height: 28px;
  }
  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .time {
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
    font-variant-numeric: tabular-nums;
  }
  .undo {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-height: 24px;
    margin-left: auto;
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-xs);
  }
</style>
