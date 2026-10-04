<!--
  Ustawienia → Modele i silniki (ładowana leniwie): wyszukiwanie w pamięci (embedder leksykalny albo
  zainstalowany model, przebudowa wektorów w tle z postępem, „Przebuduj teraz” / „Przerwij”) i katalog
  pozycji (modele rozmowy, STT, głosy, VAD, słowa wywoławcze, głos właściciela, embeddingi, sidecary)
  z filtrami rodzaju i stanu. Stan na żywo ze zdarzeń `ModelChanged`, `ModelProgress`, `ReindexStatus`.
-->
<script lang="ts">
  import { Button, ConfirmDialog, Select } from '@alfa/ui-kit';
  import type { ModelItem, ModelsView } from '../../../api/types-models';
  import {
    KIND_FILTERS,
    LEXICAL,
    STATE_FILTERS,
    embedderChoices,
    embedderFallback,
    filterItems,
    reindexPercent,
    trustHashes,
    upsertItem,
    type KindFilter,
    type StateFilter,
  } from '../../../logic/models';
  import { useApp } from '../../../state/context';
  import ModelRow from './ModelRow.svelte';
  import './work.css';

  const app = useApp();
  const { t, tk } = app.i18n;
  let view = $state<ModelsView | null>(null);
  let kind = $state<string>('all');
  let stateFilter = $state<string>('all');
  let choice = $state('');
  let busy = $state<string | null>(null);
  let removing = $state<ModelItem | null>(null);
  let confirmOpen = $state(false);

  const items = $derived(
    view ? filterItems(view.items, kind as KindFilter, stateFilter as StateFilter) : [],
  );
  const choices = $derived(view ? embedderChoices(view.items) : []);
  const reindex = $derived(view?.reindex ?? null);
  const percent = $derived(reindex ? reindexPercent(reindex) : null);

  function nameOf(id: string): string {
    if (id === LEXICAL) return t('engines.search.lexical');
    return view?.items.find((i) => i.id === id)?.name ?? id;
  }

  async function load() {
    try {
      view = await app.client.engines.list();
      choice = view.embedder.active;
    } catch (e) {
      app.toasts.show({ kind: 'error', message: e instanceof Error ? e.message : String(e) });
    }
  }

  $effect(() => {
    void load();
    return app.on((event) => {
      if (!view) return;
      if (event.type === 'ModelChanged')
        view = { ...view, items: upsertItem(view.items, event.item) };
      else if (event.type === 'ReindexStatus') view = { ...view, reindex: event.status };
      else if (event.type === 'ModelProgress') {
        const progress = { file: event.file, done: event.done, total: event.total };
        view = {
          ...view,
          items: view.items.map((i) => (i.id === event.item_id ? { ...i, progress } : i)),
        };
      }
    });
  });

  async function act(id: string, action: () => Promise<unknown>, message?: string) {
    busy = id;
    try {
      const result = await action();
      if (result && typeof result === 'object' && 'state' in result && view) {
        view = { ...view, items: upsertItem(view.items, result as ModelItem) };
      }
      if (message) app.toasts.show({ kind: 'success', message });
    } catch (e) {
      app.toasts.show({ kind: 'error', message: e instanceof Error ? e.message : String(e) });
    } finally {
      busy = null;
    }
  }

  async function activate(model: string) {
    await act(
      model,
      async () => {
        const embedder = await app.client.engines.activateEmbedder(model);
        if (view) view = { ...view, embedder };
        choice = embedder.active;
        await load();
      },
      t('engines.activated', { name: nameOf(model) }),
    );
  }

  async function verify(item: ModelItem) {
    busy = item.id;
    try {
      const result = await app.client.engines.verify(item.id);
      if (view) view = { ...view, items: upsertItem(view.items, result) };
      const kindOf = result.state === 'corrupt' ? 'error' : 'success';
      const state = tk(`engines.state.${result.state}`);
      app.toasts.show({ kind: kindOf, message: t('engines.verified', { name: item.name, state }) });
    } catch (e) {
      app.toasts.show({ kind: 'error', message: e instanceof Error ? e.message : String(e) });
    } finally {
      busy = null;
    }
  }
</script>

<p class="intro">{t('engines.intro')}</p>

<section class="wk-card" aria-labelledby="mm-search">
  <h3 id="mm-search">{t('engines.search.title')}</h3>
  {#if view}
    <p>{t('engines.search.active', { name: nameOf(view.embedder.active) })}</p>
    {#if embedderFallback(view.embedder)}
      <p class="wk-warn">
        {view.embedder.error
          ? t('engines.search.fallback', { name: nameOf(view.embedder.configured) })
          : t('engines.search.notInstalled', { name: nameOf(view.embedder.configured) })}
      </p>
    {/if}
    {#if view.embedder.error}<p class="wk-error">{view.embedder.error}</p>{/if}
    <p class="wk-meta wk-code">{t('engines.search.index', { id: view.embedder.index_id })}</p>
    <div class="wk-actions">
      <Select
        size="sm"
        bind:value={choice}
        label={t('engines.search.choose')}
        options={[
          { value: LEXICAL, label: t('engines.search.lexical') },
          ...choices.map((c) => ({ value: c.id, label: c.name })),
        ]}
      />
      <Button
        size="sm"
        variant="secondary"
        disabled={busy !== null || choice === view.embedder.active}
        onclick={() => void activate(choice)}>{t('engines.search.use')}</Button
      >
    </div>
    <p class="wk-note">{t('engines.search.hint')}</p>
    {#if reindex}
      <div class="reindex" role="status" aria-live="polite">
        {#if reindex.running}
          <progress
            class="bar"
            max="100"
            value={percent ?? undefined}
            aria-label={t('engines.reindex.progress')}
          ></progress>
          <span class="wk-meta">
            {t('engines.reindex.running', {
              done: app.i18n.int(reindex.done),
              total: app.i18n.int(reindex.total),
              dbs: reindex.databases,
            })}
          </span>
        {:else if reindex.cancelled}
          <span class="wk-meta">{t('engines.reindex.cancelled')}</span>
        {:else if reindex.finished && reindex.databases > 0}
          <span class="wk-meta">{t('engines.reindex.done', { n: reindex.embedded })}</span>
        {:else}
          <span class="wk-meta">{t('engines.reindex.idle')}</span>
        {/if}
        {#if reindex.failed > 0}
          <span class="wk-warn">{t('engines.reindex.failed', { n: reindex.failed })}</span>
        {/if}
      </div>
      <div class="wk-actions">
        {#if reindex.running}
          <Button
            size="sm"
            variant="ghost"
            onclick={() =>
              void act('reindex', async () => {
                const status = await app.client.engines.reindexCancel();
                if (view) view = { ...view, reindex: status };
              })}>{t('engines.reindex.cancel')}</Button
          >
        {:else}
          <Button
            size="sm"
            variant="ghost"
            disabled={busy !== null}
            onclick={() =>
              void act('reindex', async () => {
                const status = await app.client.engines.reindexStart();
                if (view) view = { ...view, reindex: status };
              })}>{t('engines.reindex.start')}</Button
          >
        {/if}
      </div>
    {/if}
  {/if}
</section>

<section class="wk-card" aria-labelledby="mm-list">
  <h3 id="mm-list">{t('engines.list')}</h3>
  <div class="filters">
    <label class="wk-field">
      <span>{t('engines.filter.kind')}</span>
      <Select
        size="sm"
        bind:value={kind}
        label={t('engines.filter.kind')}
        options={KIND_FILTERS.map((k) => ({ value: k, label: tk(`engines.kind.${k}`) }))}
      />
    </label>
    <label class="wk-field">
      <span>{t('engines.filter.state')}</span>
      <Select
        size="sm"
        bind:value={stateFilter}
        label={t('engines.filter.state')}
        options={STATE_FILTERS.map((s) => ({ value: s, label: tk(`engines.stateFilter.${s}`) }))}
      />
    </label>
    <span class="wk-meta count" aria-live="polite">{t('engines.count', { n: items.length })}</span>
  </div>
  {#if view && items.length === 0}
    <p class="wk-meta">{t('engines.empty')}</p>
  {/if}
  <ul class="list">
    {#each items as item (item.id)}
      <ModelRow
        {item}
        busy={busy === item.id}
        ondownload={() => void act(item.id, () => app.client.engines.download(item.id))}
        oncancel={() => void act(item.id, () => app.client.engines.cancel(item.id))}
        onverify={() => void verify(item)}
        onremove={() => {
          removing = item;
          confirmOpen = true;
        }}
        ontrust={() =>
          void act(item.id, () => app.client.engines.trustHash(item.id, trustHashes(item)))}
        onactivate={() => void activate(item.id)}
      />
    {/each}
  </ul>
</section>

<ConfirmDialog
  bind:open={confirmOpen}
  title={t('engines.removeConfirm.title', { name: removing?.name ?? '' })}
  description={t('engines.removeConfirm.body')}
  confirmLabel={t('engines.remove')}
  cancelLabel={t('common.cancel')}
  danger
  onconfirm={() => {
    confirmOpen = false;
    const target = removing;
    if (target) {
      void act(
        target.id,
        () => app.client.engines.remove(target.id),
        t('engines.removed', { name: target.name }),
      );
    }
  }}
/>

<style>
  .intro {
    margin: 0 0 var(--alfa-space-4);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .reindex {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
  }
  .bar {
    width: 100%;
    height: 8px;
    accent-color: var(--alfa-color-info);
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--alfa-space-3);
  }
  .count {
    margin-left: auto;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
</style>
