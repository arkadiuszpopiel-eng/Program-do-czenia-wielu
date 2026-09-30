<!-- Kroki narzędzi zwinięte do jednej linii statusu (ikona, opis, czas) z rozwinięciem; „Cofnij". -->
<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import FileText from '@lucide/svelte/icons/file-text';
  import Search from '@lucide/svelte/icons/search';
  import Pencil from '@lucide/svelte/icons/pencil';
  import SquareTerminal from '@lucide/svelte/icons/square-terminal';
  import Globe from '@lucide/svelte/icons/globe';
  import Brain from '@lucide/svelte/icons/brain';
  import Undo2 from '@lucide/svelte/icons/undo-2';
  import LoaderCircle from '@lucide/svelte/icons/loader-circle';
  import type { ToolStep } from '../../api/types';
  import { useApp } from '../../state/context';

  interface Props {
    steps: readonly ToolStep[];
  }

  let { steps }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  let open = $state(false);
  const regionId = $props.id();

  const ICONS = {
    file: FileText,
    search: Search,
    edit: Pencil,
    terminal: SquareTerminal,
    web: Globe,
    memory: Brain,
  };
  const running = $derived(steps.find((s) => s.status === 'running'));
  const total = $derived(steps.reduce((sum, s) => sum + (s.duration_ms ?? 0), 0));
  let undone = $state<Record<string, boolean>>({});

  async function undo(step: ToolStep) {
    if (!step.undo_token) return;
    await app.client.turns.undoStep(step.undo_token);
    undone[step.id] = true;
    app.toasts.show({ kind: 'success', message: t('conv.undone', { label: step.label }) });
  }
</script>

<div class="tools">
  <button
    type="button"
    class="summary"
    aria-expanded={open}
    aria-controls={regionId}
    onclick={() => (open = !open)}
  >
    <ChevronRight
      size={14}
      strokeWidth={1.5}
      aria-hidden="true"
      class="chev {open ? 'open' : ''}"
    />
    {#if running}
      <LoaderCircle size={14} strokeWidth={1.5} aria-hidden="true" class="spin" />
      <span>{t('conv.toolRunning', { label: running.label })}</span>
    {:else}
      <span>{t('conv.tools', { n: steps.length })}</span>
      {#if total > 0}<span class="time">· {app.i18n.duration(total)}</span>{/if}
    {/if}
  </button>
  {#if open}
    <ul class="list" id={regionId}>
      {#each steps as step (step.id)}
        {@const Icon = ICONS[step.icon]}
        <li class="step" class:error={step.status === 'error'}>
          <Icon size={14} strokeWidth={1.5} aria-hidden="true" />
          <span class="label">{step.label}</span>
          {#if step.duration_ms !== null}
            <span class="time">{app.i18n.duration(step.duration_ms)}</span>
          {/if}
          {#if step.undo_token && !undone[step.id]}
            <button type="button" class="undo" onclick={() => undo(step)}>
              <Undo2 size={12} strokeWidth={1.5} aria-hidden="true" />
              {t('common.undo')}
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .tools {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .summary {
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-1);
    align-self: flex-start;
    min-height: 24px;
    padding: 0 var(--alfa-space-1);
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .summary:hover {
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text);
  }
  .summary :global(.chev) {
    transition: transform var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .summary :global(.chev.open) {
    transform: rotate(90deg);
  }
  .summary :global(.spin) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0 0 0 var(--alfa-space-6);
    list-style: none;
  }
  .step {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    min-height: 24px;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .step.error {
    color: var(--alfa-color-error);
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
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .undo:hover {
    color: var(--alfa-color-text);
  }
</style>
