<!--
  Katalog roboczy sesji = zakres narzędzi agentek (pliki, polecenia). Bez katalogu agentki
  odpowiadają bez narzędzi. Wybór wyłącznie tutaj (natywny dialog w rdzeniu).
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import type { SessionWorkdir, WorkdirChoice } from '../../api/types';
  import { useApp } from '../../state/context';

  interface Props {
    sessionId: string;
  }

  let { sessionId }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const titleId = $props.id();
  let workdir = $state<SessionWorkdir | null>(null);
  let busy = $state(false);

  $effect(() => {
    void app.client.sessions.workdir(sessionId).then((w) => (workdir = w));
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
      app.toasts.show({
        kind: 'error',
        message: error instanceof Error ? error.message : String(error),
      });
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
  {#if workdir}
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
