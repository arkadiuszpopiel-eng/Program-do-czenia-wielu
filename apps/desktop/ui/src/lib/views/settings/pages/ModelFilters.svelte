<!--
  Pasek nad katalogiem modeli: wyszukiwanie po nazwie, filtry rodzaju i stanu, kolejność listy,
  licznik pozycji i podsumowanie miejsca na dysku (ile zajmują pozycje na dysku, ile zajęłyby
  te do pobrania). Podsumowanie liczy cały katalog, nie tylko to, co widać po filtrach.
-->
<script lang="ts">
  import { Select, TextField } from '@alfa/ui-kit';
  import type { DiskUsage } from '../../../logic/model-catalog';
  import { SORT_ORDERS } from '../../../logic/model-catalog';
  import { KIND_FILTERS, STATE_FILTERS } from '../../../logic/models';
  import { useApp } from '../../../state/context';

  interface Props {
    kind: string;
    stateFilter: string;
    query: string;
    order: string;
    /** Liczba pozycji po filtrach (`null` — katalog jeszcze się wczytuje). */
    count: number | null;
    usage: DiskUsage | null;
  }

  let {
    kind = $bindable(),
    stateFilter = $bindable(),
    query = $bindable(),
    order = $bindable(),
    count,
    usage,
  }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
</script>

<div class="filters">
  <div class="search">
    <TextField label={t('engines.find')} type="search" bind:value={query} />
  </div>
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
  <label class="wk-field">
    <span>{t('engines.sort')}</span>
    <Select
      size="sm"
      bind:value={order}
      label={t('engines.sort')}
      options={SORT_ORDERS.map((o) => ({ value: o, label: tk(`engines.sort.${o}`) }))}
    />
  </label>
  <!-- Licznik dopiero po wczytaniu katalogu: „0 pozycji" w trakcie ładowania było nieprawdą
       (i trafiało do czytnika ekranu). -->
  <span class="wk-meta count" aria-live="polite"
    >{#if count !== null}{t('engines.count', { n: count })}{/if}</span
  >
</div>
{#if usage}
  <p class="wk-meta usage" data-testid="models-usage">
    {t('engines.usage.installed', {
      size: app.i18n.bytes(usage.installedBytes),
      n: usage.installedCount,
    })}
    {#if usage.availableCount > 0}
      · {t('engines.usage.available', {
        size: app.i18n.bytes(usage.availableBytes),
        n: usage.availableCount,
      })}
    {/if}
  </p>
{/if}

<style>
  .filters {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--alfa-space-3);
  }
  .search {
    flex: 1 1 200px;
    min-width: 160px;
    max-width: 320px;
  }
  .count {
    margin-left: auto;
  }
  .usage {
    margin: var(--alfa-space-2) 0 0;
  }
</style>
