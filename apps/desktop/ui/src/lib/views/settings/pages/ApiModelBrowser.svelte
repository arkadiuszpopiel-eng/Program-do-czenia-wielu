<!--
  Przeglądarka modeli konta: lista dostawcy bywa długa i zmieszana (rozmowa, mowa, obrazy,
  embeddingi), więc jest wyszukiwarka, filtry rodzaju i dopisywanie do kolejki wybranej klasy.
-->
<script lang="ts">
  import { Button, Checkbox, Chip, TextField } from '@alfa/ui-kit';
  import Plus from '@lucide/svelte/icons/plus';
  import type { ModelInfo, ModelPurpose } from '../../../api/types-hub';
  import {
    PURPOSES,
    addBlock,
    countByPurpose,
    filterModels,
    type TaskClassId,
  } from '../../../logic/model-queue';
  import { useApp } from '../../../state/context';
  import ModelBadges from './ModelBadges.svelte';

  interface Props {
    models: readonly ModelInfo[];
    queue: readonly string[];
    taskClass: TaskClassId;
    saving: boolean;
    onadd: (id: string) => void;
  }

  let { models, queue, taskClass, saving, onadd }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;

  /** Ile wierszy na raz — pełna lista (ponad sto pozycji) rozwija się na żądanie. */
  const PAGE = 40;
  let query = $state('');
  let purposes = $state<ModelPurpose[]>([]);
  let onlyFitting = $state(true);
  let hideSnapshots = $state(true);
  let limit = $state(PAGE);

  const className = $derived(tk(`api.class.${taskClass}`));
  const counts = $derived(countByPurpose(models));
  const present = $derived(PURPOSES.filter((p) => counts.has(p)));
  const found = $derived(
    filterModels(models, {
      query,
      purposes: new Set(purposes),
      hideSnapshots,
      fitting: onlyFitting ? taskClass : null,
    }),
  );
  const shown = $derived(found.slice(0, limit));
  const filtered = $derived(query.trim() !== '' || purposes.length > 0);

  // Nowe kryteria = lista od początku.
  $effect(() => {
    void [query, purposes, onlyFitting, hideSnapshots, taskClass];
    limit = PAGE;
  });

  function toggle(purpose: ModelPurpose) {
    purposes = purposes.includes(purpose)
      ? purposes.filter((p) => p !== purpose)
      : [...purposes, purpose];
    // Wybrany rodzaj spoza klasy (np. „Synteza mowy” przy rozmowie) ma być widoczny.
    if (purposes.length > 0) onlyFitting = false;
  }

  function reset() {
    query = '';
    purposes = [];
  }
</script>

<p class="hint">{t('api.browserHint', { name: className })}</p>
<div class="tools">
  <TextField
    label={t('api.search')}
    bind:value={query}
    placeholder={t('api.searchHint')}
    autocomplete="off"
    spellcheck={false}
  />
  <div class="kinds" role="group" aria-label={t('api.filters')}>
    {#each present as purpose (purpose)}
      <Chip size="sm" selected={purposes.includes(purpose)} onclick={() => toggle(purpose)}>
        {tk(`api.purpose.${purpose}`)} · {counts.get(purpose) ?? 0}
      </Chip>
    {/each}
  </div>
  <Checkbox label={t('api.onlyFitting', { name: className })} bind:checked={onlyFitting} />
  <Checkbox label={t('api.hideSnapshots')} bind:checked={hideSnapshots} />
</div>

<p class="count" role="status">
  {found.length > shown.length
    ? t('api.shownOf', { shown: shown.length, total: found.length })
    : t('api.shown', { n: found.length })}
</p>

{#if found.length === 0}
  <p class="none">
    {t('api.none')}
    {#if filtered}
      <Button variant="ghost" size="sm" onclick={reset}>{t('api.resetFilters')}</Button>
    {/if}
  </p>
{:else}
  <ul class="models">
    {#each shown as model (model.id)}
      {@const block = addBlock(queue, taskClass, model)}
      <li class="model">
        <div class="body">
          <span class="name">{model.name}</span>
          {#if model.name !== model.id}<code>{model.id}</code>{/if}
          <ModelBadges {model} />
        </div>
        {#if block}
          <span class="blocked">{tk(`api.block.${block}`)}</span>
        {:else}
          <Button
            variant="secondary"
            size="sm"
            disabled={saving}
            aria-label={t('api.addLabel', { model: model.id })}
            onclick={() => onadd(model.id)}
          >
            <Plus size={14} strokeWidth={1.75} aria-hidden="true" />
            {t('api.add')}
          </Button>
        {/if}
      </li>
    {/each}
  </ul>
  {#if found.length > shown.length}
    <div class="more">
      <Button variant="ghost" size="sm" onclick={() => (limit += PAGE)}>{t('api.more')}</Button>
    </div>
  {/if}
{/if}

<style>
  .hint,
  .count,
  .none {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .tools {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
  }
  .kinds {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-1);
  }
  .none {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-2);
  }
  .models {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    list-style: none;
  }
  .model {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-3);
    padding: var(--alfa-space-2) var(--alfa-space-3);
  }
  .model + .model {
    border-top: 1px solid var(--alfa-color-border);
  }
  .body {
    display: flex;
    flex: 1;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-1) var(--alfa-space-2);
    min-width: 0;
  }
  .name {
    font-weight: var(--alfa-weight-semibold);
  }
  code {
    color: var(--alfa-color-text-muted);
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-xs);
    overflow-wrap: anywhere;
  }
  .blocked {
    flex: none;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .more {
    display: flex;
    justify-content: center;
  }
</style>
