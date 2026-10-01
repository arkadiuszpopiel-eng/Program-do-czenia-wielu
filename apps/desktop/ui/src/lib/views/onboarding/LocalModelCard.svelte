<!--
  Model lokalny w onboardingu (PLAN §14.5, ADR 0014): pobranie domyślnego modelu jednym kliknięciem
  (wznawiane, SHA-256 w rdzeniu), postęp ze zdarzeń `LocalModelProgress`, anulowanie.
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { LocalModelInfo } from '../../api/types-hub';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  let model = $state<LocalModelInfo | null>(null);
  let failure = $state<string | null>(null);

  $effect(() => {
    void app.client.models.localList().then((list) => {
      model = list.find((m) => m.default) ?? list[0] ?? null;
    });
  });

  const progress = $derived(
    model && app.localDownload?.model_id === model.id ? app.localDownload : null,
  );
  const installed = $derived(Boolean(model?.installed) || progress?.state === 'done');
  const downloading = $derived(
    progress ? progress.state === 'downloading' : Boolean(model?.downloading),
  );

  async function download() {
    if (!model) return;
    failure = null;
    try {
      await app.client.models.localDownload(model.id);
    } catch (error) {
      failure = error instanceof Error ? error.message : String(error);
    }
  }
</script>

{#if model}
  <section class="local" aria-labelledby="ob-local-title">
    <h3 id="ob-local-title">{t('ob.local.title')}</h3>
    <p class="muted">{t('ob.local.desc')}</p>
    {#if installed}
      <p class="ok" role="status">{t('ob.local.done')}</p>
    {:else if downloading}
      <progress
        aria-label={t('ob.local.progress')}
        max={progress?.total ?? model.size_bytes}
        value={progress?.bytes ?? 0}
      ></progress>
      <p class="muted small" role="status">
        {app.i18n.bytes(progress?.bytes ?? 0)} / {app.i18n.bytes(
          progress?.total ?? model.size_bytes,
        )}
      </p>
      <Button variant="ghost" onclick={() => void app.client.models.localCancel(model?.id ?? null)}
        >{t('ob.local.cancel')}</Button
      >
    {:else}
      {#if progress?.state === 'failed' || failure}
        <p class="warn" role="alert">
          {t('ob.local.failed', { error: progress?.error ?? failure ?? '' })}
        </p>
      {:else if progress?.state === 'cancelled'}
        <p class="muted" role="status">{t('ob.local.cancelled')}</p>
      {/if}
      <Button variant="secondary" onclick={download}
        >{t('ob.local.download', {
          name: model.name,
          size: app.i18n.bytes(model.size_bytes),
        })}</Button
      >
    {/if}
  </section>
{/if}

<style>
  .local {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin-top: var(--alfa-space-4);
  }
  h3 {
    font-size: var(--alfa-font-size-md);
  }
  progress {
    width: 100%;
  }
  .muted {
    color: var(--alfa-color-text-muted);
  }
  .small {
    font-size: var(--alfa-font-size-sm);
  }
  .ok {
    color: var(--alfa-color-success);
  }
  .warn {
    color: var(--alfa-color-warning);
  }
</style>
