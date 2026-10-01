<!--
  Karta intencji kroku: „Uruchom w terminalu" (polecenie do skopiowania + terminal w katalogu —
  bez automatycznego wykonania) albo trwałe usunięcie (potwierdzenie wyłącznie w oknie Brokera).
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import SquareTerminal from '@lucide/svelte/icons/square-terminal';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import type { ToolIntent } from '../../api/types';
  import { useApp } from '../../state/context';

  interface Props {
    intent: ToolIntent;
    /** Krok (`agents_open_terminal`). */
    stepId: string;
  }

  let { intent, stepId }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const titleId = $props.id();

  async function copy() {
    if (!intent.command) return;
    try {
      await navigator.clipboard.writeText(intent.command);
      app.toasts.show({ kind: 'success', message: t('intent.copied') });
    } catch {
      app.toasts.show({ kind: 'warning', message: t('intent.copyFailed') });
    }
  }

  async function openTerminal() {
    try {
      await app.client.agents.openTerminal(stepId);
    } catch (error) {
      app.toasts.show({
        kind: 'error',
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }
</script>

<section class="intent" aria-labelledby={titleId}>
  {#if intent.kind === 'open_in_terminal'}
    <h4 id={titleId} class="title">
      <SquareTerminal size={14} strokeWidth={1.5} aria-hidden="true" />
      {t('intent.terminal')}
    </h4>
    <p class="hint">{t('intent.terminalHint', { cwd: intent.cwd ?? '—' })}</p>
    {#if intent.command}<pre class="cmd"><code>{intent.command}</code></pre>{/if}
    <div class="actions">
      <Button size="sm" variant="secondary" onclick={copy}>{t('intent.copy')}</Button>
      <Button size="sm" variant="primary" onclick={openTerminal}>{t('intent.open')}</Button>
    </div>
  {:else}
    <h4 id={titleId} class="title">
      <Trash2 size={14} strokeWidth={1.5} aria-hidden="true" />
      {intent.title}
    </h4>
    <ul class="paths">
      {#each intent.paths as path (path)}<li><code>{path}</code></li>{/each}
    </ul>
    <p class="hint">{t('intent.deleteHint')}</p>
  {/if}
</section>

<style>
  .intent {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    font-size: var(--alfa-font-size-sm);
  }
  .title {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-1);
    font-size: var(--alfa-font-size-sm);
  }
  .hint {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .cmd {
    margin: 0;
    padding: var(--alfa-space-2);
    overflow-x: auto;
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface2);
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-xs);
    white-space: pre-wrap;
  }
  .paths {
    margin: 0;
    padding-left: var(--alfa-space-4);
  }
  .actions {
    display: flex;
    gap: var(--alfa-space-2);
  }
</style>
