<!--
  Ustawienia → Pamięć: stan porządkowania (Strażniczka pamięci: okno nocne, licznik
  bezczynności, model lokalny, ostatni raport, „Porządkuj teraz") i zakresy z zapomnieniem
  całego zakresu (podgląd kaskady, crypto-shredding bazy). Inspektor — panel boczny „Pamięć".
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type {
    MemoryForgetPreview,
    MemoryScopeInfo,
    MemoryStatus,
  } from '../../../api/types-memory';
  import { useApp } from '../../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  let status = $state<MemoryStatus | null>(null);
  let scopes = $state<readonly MemoryScopeInfo[]>([]);
  let running = $state(false);
  let preview = $state<MemoryForgetPreview | null>(null);

  async function load() {
    [status, scopes] = await Promise.all([app.client.memory.status(), app.client.memory.scopes()]);
  }

  $effect(() => {
    void load();
    return app.on((event) => {
      if (event.type === 'MemoryChanged') void load();
    });
  });

  async function consolidate() {
    running = true;
    try {
      await app.client.memory.consolidateNow();
      app.toasts.show({ kind: 'success', message: t('memory.consolidated') });
      await load();
    } finally {
      running = false;
    }
  }

  async function forgetScope() {
    if (!preview || preview.target.target !== 'scope') return;
    const report = await app.client.memory.forget(preview.target);
    preview = null;
    app.toasts.show({
      kind: 'success',
      message: t('memory.forgotten', {
        removed: report.removed,
        versions: report.versions,
        derived: report.derived,
      }),
    });
    await load();
  }

  function openInspector() {
    app.closeSettings();
    app.openPanel('memory');
  }
</script>

{#if status}
  {@const last = status.last}
  <section class="card" aria-labelledby="mem-status">
    <h3 id="mem-status">{t('memory.status.title')}</h3>
    <p>
      {status.consolidation_enabled
        ? t('memory.status.on', { window: status.window })
        : t('memory.status.off')}
    </p>
    {#if !status.idle_available}<p class="note">{t('memory.status.idleMissing')}</p>{/if}
    {#if !status.model_available}<p class="note">{t('memory.status.noModel')}</p>{/if}
    {#if status.pending}<p>{t('memory.pendingCount', { n: status.pending })}</p>{/if}
    <p class="muted">
      {#if !last}{t('memory.status.never')}
      {:else if last.skipped}{t('memory.status.skipped', { reason: last.skipped })}
      {:else}{t('memory.status.last', {
          when: app.i18n.relative(last.started_at),
          merged: last.merged,
          expired: last.expired,
          proposals: last.proposals,
          conflicts: last.conflicts,
        })}{/if}
    </p>
    <div class="row">
      <Button size="sm" loading={running} onclick={consolidate}>{t('memory.consolidateNow')}</Button
      >
      <Button size="sm" variant="secondary" onclick={openInspector}
        >{t('memory.openInspector')}</Button
      >
    </div>
    <p class="muted">{t('memory.inspectorHint')}</p>
  </section>
{/if}

<section class="card" aria-labelledby="mem-scopes">
  <h3 id="mem-scopes">{t('memory.scopes')}</h3>
  {#if scopes.length === 0}
    <p class="muted">{t('memory.scopesEmpty')}</p>
  {:else}
    <ul class="scopes">
      {#each scopes as s (s.key)}
        <li>
          <span class="name">{t(`memory.scopeKind.${s.scope.kind}`)}: {s.label}</span>
          <span class="muted"
            >{t('memory.count', { n: s.entries })} · {t('memory.scopeEntries', {
              active: s.active,
              pending: s.pending,
            })}{#if !s.document}
              · {t('memory.privateScope')}{/if}</span
          >
          <Button
            size="sm"
            variant="danger"
            onclick={async () =>
              (preview = await app.client.memory.forgetPreview({ target: 'scope', scope: s.key }))}
            >{t('memory.forgetScope')}</Button
          >
        </li>
      {/each}
    </ul>
  {/if}
  {#if preview}
    <div class="forget" role="group" aria-label={t('memory.forgetTitle')}>
      <p>{t('memory.forgetCount', { n: preview.remove.length })}</p>
      {#if preview.shred}<p class="note">{t('memory.shred')}</p>{/if}
      <div class="row">
        <Button size="sm" variant="danger" onclick={forgetScope}>{t('memory.forgetConfirm')}</Button
        >
        <Button size="sm" variant="ghost" onclick={() => (preview = null)}
          >{t('memory.cancel')}</Button
        >
      </div>
    </div>
  {/if}
</section>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin-bottom: var(--alfa-space-4);
    padding: var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    font-size: var(--alfa-font-size-sm);
  }
  h3 {
    font-size: var(--alfa-font-size-md);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
  }
  .muted {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .note {
    font-size: var(--alfa-font-size-xs);
    font-style: italic;
  }
  .scopes {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .scopes li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-2);
  }
  .name {
    font-weight: var(--alfa-weight-semibold);
  }
  .forget {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-2);
    border: 1px solid var(--alfa-color-error);
    border-radius: var(--alfa-radius-control);
  }
</style>
