<!--
  Katalog roboczy sesji = zakres narzędzi agentek (pliki, polecenia). Bez katalogu agentki
  odpowiadają bez narzędzi. Wybór wyłącznie tutaj (natywny dialog w rdzeniu).
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import type { SessionWorkdir, WorkdirChoice } from '../../api/types';
  import { load, showError } from '../../state/attempt';
  import { useApp } from '../../state/context';
  import LoadFailed from '../shell/LoadFailed.svelte';
  import Loading from '../shell/Loading.svelte';

  interface Props {
    sessionId: string;
  }

  let { sessionId }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const titleId = $props.id();
  let workdir = $state<SessionWorkdir | null>(null);
  let busy = $state(false);
  let loadError = $state<string | null>(null);

  // Błąd = „Nie udało się wczytać" z „Ponów" zamiast samego nagłówka bez przycisków.
  async function loadWorkdir() {
    const id = sessionId;
    loadError = null;
    const result = await load(() => app.client.sessions.workdir(id));
    if (id !== sessionId) return;
    if (result.status === 'ready') workdir = result.value;
    else if (result.status === 'failed') loadError = result.error;
  }

  $effect(() => {
    void loadWorkdir();
  });

  async function choose(choice: WorkdirChoice) {
    busy = true;
    try {
      workdir = await app.client.sessions.chooseWorkdir(sessionId, choice);
      app.toasts.show({
        kind: 'success',
        message: workdir.path ? t('workdir.set', { path: workdir.path }) : t('workdir.cleared'),
      });
    } catch (error) {
      showError(app.toasts, error);
    } finally {
      busy = false;
    }
  }
</script>

<section class="workdir" aria-labelledby={titleId}>
  <h3 id={titleId} class="title">
    <FolderOpen size={14} strokeWidth={1.5} aria-hidden="true" />
    {t('workdir.title')}
  </h3>
  {#if loadError}
    <LoadFailed error={loadError} onretry={() => void loadWorkdir()} />
  {:else if !workdir}
    <Loading lines={2} />
  {:else}
    {#if workdir.path}
      <p class="path"><code>{workdir.path}</code></p>
      <p class="hint">{t('workdir.on')}</p>
    {:else}
      <p class="hint">{t('workdir.off')}</p>
    {/if}
    <div class="actions">
      <Button size="sm" variant="secondary" disabled={busy} onclick={() => void choose('dialog')}
        >{t('workdir.pick')}</Button
      >
      {#if workdir.path !== workdir.default_path}
        <Button size="sm" variant="ghost" disabled={busy} onclick={() => void choose('default')}
          >{t('workdir.default')}</Button
        >
      {/if}
      {#if workdir.path}
        <Button size="sm" variant="ghost" disabled={busy} onclick={() => void choose('none')}
          >{t('workdir.disable')}</Button
        >
      {/if}
    </div>
  {/if}
</section>

<style>
  .workdir {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
  }
  .title {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-1);
    font-size: var(--alfa-font-size-sm);
  }
  .path {
    overflow-wrap: anywhere;
    font-size: var(--alfa-font-size-xs);
  }
  .hint {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-1);
  }
</style>
