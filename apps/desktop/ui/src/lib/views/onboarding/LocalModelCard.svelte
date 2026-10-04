<!--
  Model lokalny w onboardingu (PLAN §14.5, ADR 0014) przez menedżer modeli: pobranie domyślnego
  modelu rozmowy jednym kliknięciem (wznawiane, SHA-256 w rdzeniu), postęp ze zdarzeń
  `ModelProgress`/`ModelChanged`, anulowanie; plik bez przypiętej sumy → jawna zgoda z policzonym
  SHA-256 (jak w Ustawieniach → Modele i silniki).
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { ModelItem } from '../../api/types-models';
  import { groupedHash, progressPercent, trustHashes } from '../../logic/models';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  let model = $state<ModelItem | null>(null);
  let failure = $state<string | null>(null);
  let cancelled = $state(false);

  $effect(() => {
    void app.client.engines.list().then(
      (view) => (model = view.items.find((i) => i.kind === 'llm') ?? null),
      () => undefined,
    );
    return app.on((event) => {
      if (!model) return;
      if (event.type === 'ModelChanged' && event.item.id === model.id) model = event.item;
      if (event.type === 'ModelProgress' && event.item_id === model.id) {
        model = { ...model, progress: { file: event.file, done: event.done, total: event.total } };
      }
    });
  });

  const downloading = $derived(model?.state === 'queued' || model?.state === 'downloading');
  const percent = $derived(model ? progressPercent(model) : null);
  const unpinned = $derived(model?.files.filter((f) => !f.pinned_sha256 && f.sha256) ?? []);

  async function act(action: (id: string) => Promise<ModelItem>) {
    if (!model) return;
    failure = null;
    try {
      model = await action(model.id);
    } catch (error) {
      failure = error instanceof Error ? error.message : String(error);
    }
  }
</script>

{#if model}
  <section class="local" aria-labelledby="ob-local-title">
    <h3 id="ob-local-title">{t('ob.local.title')}</h3>
    <p class="muted">{t('ob.local.desc')}</p>
    {#if model.state === 'installed' || model.state === 'external'}
      <p class="ok" role="status">{t('ob.local.done')}</p>
    {:else if model.state === 'installing'}
      <p class="muted" role="status">{t('engines.state.installing')}</p>
    {:else if downloading}
      <progress aria-label={t('ob.local.progress')} max="100" value={percent ?? undefined}
      ></progress>
      <p class="muted small" role="status">
        {app.i18n.bytes(model.progress?.done ?? 0)} / {app.i18n.bytes(
          model.progress?.total ?? model.size_bytes,
        )}
      </p>
      <Button
        variant="ghost"
        onclick={() => {
          cancelled = true;
          void act((id) => app.client.engines.cancel(id));
        }}>{t('ob.local.cancel')}</Button
      >
    {:else if model.state === 'needs_trust'}
      <div class="trust">
        <p><strong>{t('engines.trust.title')}</strong></p>
        <p class="small">{t('engines.trust.body')}</p>
        <p class="muted small">{t('engines.trust.license', { license: model.license })}</p>
        {#each unpinned as f (f.name)}
          <p class="small">{f.name}</p>
          <code class="hash">{groupedHash(f.sha256 ?? '')}</code>
        {/each}
        <Button
          variant="primary"
          onclick={() =>
            void act((id) => app.client.engines.trustHash(id, model ? trustHashes(model) : {}))}
          >{t('engines.trust.accept')}</Button
        >
      </div>
    {:else}
      {#if model.error || failure}
        <p class="warn" role="alert">
          {t('ob.local.failed', { error: model.error ?? failure ?? '' })}
        </p>
      {:else if cancelled || model.state === 'paused'}
        <p class="muted" role="status">{t('ob.local.cancelled')}</p>
      {/if}
      <Button variant="secondary" onclick={() => void act((id) => app.client.engines.download(id))}
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
  .trust {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-warning);
    border-radius: var(--alfa-radius-control);
  }
  .trust p {
    margin: 0;
  }
  .hash {
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-sm);
    overflow-wrap: anywhere;
    user-select: all;
  }
</style>
