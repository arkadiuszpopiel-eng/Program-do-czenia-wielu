<!--
  Dobór modelu → „Sprawdź na przykładzie”: ocena trudności przykładowej prośby i kolejność prób
  modeli (`routing_preview` — nic nie trafia do modelu).
-->
<script lang="ts">
  import { Button, Select } from '@alfa/ui-kit';
  import type { RoutingPreview } from '../../../api/types-hub';
  import { errorText } from '../../../api/command-error';
  import { DIFFICULTY_CLASSES } from '../../../logic/routing';
  import { useApp } from '../../../state/context';

  const app = useApp();
  const { t, tk } = app.i18n;
  const uid = $props.id();

  let text = $state('');
  let taskClass = $state<string>('chat');
  let busy = $state(false);
  let result = $state<RoutingPreview | null>(null);

  async function check() {
    if (busy || !text.trim()) return;
    busy = true;
    try {
      result = await app.client.accounts.routingPreview(text, taskClass);
    } catch (error) {
      app.toasts.show({ kind: 'error', message: errorText(error) });
    } finally {
      busy = false;
    }
  }
</script>

<section class="card" aria-labelledby="{uid}-title">
  <h3 id="{uid}-title">{t('routing.preview')}</h3>
  <p class="muted">{t('routing.previewHint')}</p>
  <label class="field">
    <span>{t('routing.previewText')}</span>
    <textarea
      rows="3"
      bind:value={text}
      onkeydown={(e) => {
        if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) void check();
      }}></textarea>
  </label>
  <div class="controls">
    <span class="label">{t('routing.previewClass')}</span>
    <Select
      value={taskClass}
      label={t('routing.previewClass')}
      options={DIFFICULTY_CLASSES.map((id) => ({ value: id, label: tk(`api.class.${id}`) }))}
      onchange={(value) => (taskClass = value)}
    />
    <Button size="sm" variant="primary" disabled={busy || !text.trim()} onclick={() => void check()}
      >{t('routing.check')}</Button
    >
  </div>
  {#if result}
    <div class="result" role="status">
      {#if result.difficulty && result.start}
        <p>
          {t('routing.result', {
            level: tk(`routing.level.${result.difficulty}`),
            reasons: result.reasons,
            rung: tk(`routing.rung.${result.start}`),
          })}
          {#if result.score !== null}{t('routing.score', { score: result.score })}{/if}
        </p>
        {#if result.kept_queue}<p class="muted">{t('routing.kept')}</p>{/if}
      {:else}
        <p class="muted">{t('routing.off')}</p>
      {/if}
      {#if result.order.length === 0}
        <p class="muted">{t('routing.empty')}</p>
      {:else}
        <h4>{t('routing.order')}</h4>
        <ol class="order">
          {#each result.order as model, at (model.candidate)}
            <li>
              <span class="name">{model.model}</span>
              <span class="muted">
                {[
                  model.provider_name,
                  tk(`routing.rung.${model.rung}`),
                  model.local ? t('routing.local') : '',
                ]
                  .filter(Boolean)
                  .join(' · ')}
              </span>
              {#if at === 0}<strong class="first">{t('routing.first')}</strong>{/if}
            </li>
          {/each}
        </ol>
      {/if}
    </div>
  {/if}
</section>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    margin-bottom: var(--alfa-space-4);
    padding: var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
  }
  h3 {
    margin: 0;
    font-size: var(--alfa-font-size-lg);
  }
  h4 {
    margin: 0;
    font-size: var(--alfa-font-size-md);
  }
  .muted {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  textarea {
    width: 100%;
    min-height: 72px;
    padding: var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text);
    font: inherit;
    font-weight: var(--alfa-weight-regular);
    resize: vertical;
  }
  textarea:focus-visible {
    outline: 2px solid var(--alfa-color-focus);
    outline-offset: 1px;
  }
  .label {
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  .controls {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-2);
  }
  .result {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-3);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface2);
  }
  .result p {
    margin: 0;
  }
  .order {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    margin: 0;
    padding-left: var(--alfa-space-6);
  }
  .first {
    margin-left: var(--alfa-space-2);
    font-size: var(--alfa-font-size-sm);
  }
  .name {
    margin-right: var(--alfa-space-2);
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-sm);
  }
</style>
