<!--
  Ustawienia → Aktualizacje: wersja i kanał, stan (sprawdzanie, pobieranie z postępem i wznawianiem,
  weryfikacja podpisu, gotowa do restartu), „Sprawdź teraz", „Pobierz i przygotuj", „Uruchom ponownie,
  aby zaktualizować" (zablokowane w trakcie zadania agentki / rozmowy głosowej), „Przywróć poprzednią
  wersję" z potwierdzeniem. Pod kartą — ustawienia strony (kanał, tryb, „Co nowego").
-->
<script lang="ts">
  import { Button, ConfirmDialog } from '@alfa/ui-kit';
  import type { UpdatesView } from '../../../api/types-updates';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import { phaseMessage, progressPercent, updateActions, isRollback } from '../../../logic/updates';
  import { attempt, load } from '../../../state/attempt';
  import { useApp } from '../../../state/context';
  import './work.css';

  const app = useApp();
  const { t, tk } = app.i18n;
  let confirmRollback = $state(false);
  let rollbackTarget = $state<string | null>(null);
  let working = $state(false);
  let loadError = $state<string | null>(null);

  const view = $derived(app.updates.view);
  const actions = $derived(view ? updateActions(view) : null);
  const percent = $derived(view ? progressPercent(view) : null);

  async function reload() {
    const result = await load(() => app.client.updates.status());
    if (result.status === 'ready') {
      app.updates.apply(result.value);
      loadError = null;
    } else if (result.status === 'failed') loadError = result.error;
  }

  $effect(() => {
    void reload();
  });

  async function run(action: () => Promise<UpdatesView | void>, message?: string) {
    working = true;
    try {
      const result = await action();
      if (result) app.updates.apply(result);
      if (message) app.toasts.show({ kind: 'success', message });
    } catch (error) {
      app.toasts.show({
        kind: 'error',
        message: error instanceof Error ? error.message : String(error),
      });
    } finally {
      working = false;
    }
  }

  async function restart() {
    await run(() => app.client.updates.restart());
    // Atrapa „uruchamia ponownie" bez zamykania okna — „Co nowego" pokaże się od razu.
    await attempt(app.toasts, () => app.updates.load(app.client.updates));
  }

  function progressLabel(v: UpdatesView): string {
    const p = v.progress;
    if (!p) return '';
    const done = app.i18n.bytes(p.downloaded);
    const text = p.total
      ? t('updates.progressText', { done, total: app.i18n.bytes(p.total) })
      : t('updates.progressUnknown', { done });
    return p.resumed ? `${text} · ${t('updates.resumed')}` : text;
  }
</script>

<section class="wk-card" aria-labelledby="up-title">
  <h3 id="up-title">{t('updates.title')}</h3>
  {#if loadError}<LoadFailed error={loadError} onretry={() => void reload()} />{/if}
  {#if view && actions}
    {@const message = phaseMessage(view)}
    <p>
      {t('updates.current', { version: view.current })} · {tk(`updates.channel.${view.channel}`)}
    </p>
    <p class="wk-meta">
      {view.last_check
        ? t('updates.lastCheck', { time: app.i18n.dateTime(view.last_check) })
        : t('updates.neverChecked')}
    </p>
    <p
      role="status"
      aria-live="polite"
      class:wk-ok={view.phase === 'up_to_date' || view.phase === 'ready'}
      class:wk-warn={view.phase === 'failed'}
    >
      {tk(message.key, message.params)}
    </p>
    {#if view.phase === 'disabled'}
      <p class="wk-note">{t('updates.disabledHint')}</p>
    {/if}
    {#if view.error && view.phase === 'failed'}<p class="wk-error">{view.error}</p>{/if}
    {#if view.progress && (view.phase === 'downloading' || view.phase === 'verifying')}
      <div class="bar-row">
        <progress
          class="bar"
          max="100"
          value={percent ?? undefined}
          aria-label={t('updates.progress')}
          aria-valuetext={progressLabel(view)}
        ></progress>
        <span class="wk-meta">{progressLabel(view)}</span>
      </div>
    {:else if actions.resume && view.progress}
      <p class="wk-meta">
        {t('updates.partial', { done: app.i18n.bytes(view.progress.downloaded) })}
      </p>
    {/if}
    {#if view.available}
      <div class="notes">
        <h4>{t('updates.notes')} · {view.available.version}</h4>
        <p class="text">{view.available.notes}</p>
      </div>
    {/if}
    {#if view.restart_blocked && actions.restart}
      <p class="wk-warn">
        {t('updates.restartBlocked', { why: app.i18n.text(view.restart_blocked) })}
      </p>
    {/if}
    <div class="wk-actions">
      {#if actions.restart}
        <Button
          size="sm"
          variant="primary"
          disabled={working || view.restart_blocked !== null}
          onclick={restart}
          >{isRollback(view) ? t('updates.restartRollback') : t('updates.restart')}</Button
        >
      {/if}
      {#if actions.download}
        <Button
          size="sm"
          variant="primary"
          disabled={working}
          onclick={() => run(() => app.client.updates.download())}>{t('updates.download')}</Button
        >
      {/if}
      {#if actions.resume}
        <Button
          size="sm"
          variant="primary"
          disabled={working}
          onclick={() => run(() => app.client.updates.download())}>{t('updates.resume')}</Button
        >
      {/if}
      {#if actions.cancel}
        <Button
          size="sm"
          variant="secondary"
          onclick={() => run(() => app.client.updates.cancel(), t('updates.cancelled'))}
          >{t('updates.cancel')}</Button
        >
      {/if}
      {#if actions.check}
        <Button
          size="sm"
          variant="secondary"
          disabled={working}
          onclick={() => run(() => app.client.updates.check())}>{t('updates.check')}</Button
        >
      {/if}
      {#if actions.rollback && view.previous}
        <Button
          size="sm"
          variant="ghost"
          onclick={() => {
            rollbackTarget = view.previous;
            confirmRollback = true;
          }}>{t('updates.rollback', { version: view.previous })}</Button
        >
      {/if}
    </div>
    <p class="wk-meta">{t('updates.noTelemetry')}</p>
  {/if}
</section>

<ConfirmDialog
  bind:open={confirmRollback}
  title={t('updates.rollbackTitle')}
  description={t('updates.rollbackDescription', { version: rollbackTarget ?? '' })}
  confirmLabel={t('updates.rollbackConfirm')}
  cancelLabel={t('common.cancel')}
  onconfirm={() => {
    confirmRollback = false;
    void run(
      () => app.client.updates.rollback(),
      t('updates.rollbackDone', { version: rollbackTarget ?? '' }),
    );
  }}
/>

<style>
  .bar-row {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
  }
  .bar {
    width: 100%;
    height: 8px;
    accent-color: var(--alfa-color-info);
  }
  .notes {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    padding: var(--alfa-space-3);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface2);
  }
  .text {
    margin: 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
