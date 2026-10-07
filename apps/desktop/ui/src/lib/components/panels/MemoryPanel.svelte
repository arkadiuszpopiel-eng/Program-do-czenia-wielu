<!--
  Inspektor pamięci (makieta 9, F7): lista wpisów z filtrami (warstwa, zakres, zaufanie, stan)
  i wyszukiwaniem; szczegóły wpisu („dlaczego to pamiętam", wersje, edycja = nowa wersja,
  przypięcie, zatwierdzenie, awans, zapomnienie z podglądem kaskady) i dziennik zakresu z cofaniem.
-->
<script lang="ts">
  import { EmptyState, Select, TextField } from '@alfa/ui-kit';
  import Brain from '@lucide/svelte/icons/brain';
  import Pin from '@lucide/svelte/icons/pin';
  import type {
    MemoryItem,
    MemoryLayer,
    MemoryScopeInfo,
    MemoryState,
  } from '../../api/types-memory';
  import { load } from '../../state/attempt';
  import { useApp } from '../../state/context';
  import LoadFailed from '../shell/LoadFailed.svelte';
  import Loading from '../shell/Loading.svelte';
  import MemoryDetail from './MemoryDetail.svelte';

  interface Props {
    sessionId: string;
  }

  let { sessionId }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const LAYERS: readonly MemoryLayer[] = ['working', 'episodic', 'semantic', 'procedural'];
  const STATES: readonly MemoryState[] = ['active', 'pending', 'superseded', 'expired'];

  let scopes = $state<readonly MemoryScopeInfo[]>([]);
  let items = $state<readonly MemoryItem[]>([]);
  let total = $state(0);
  let loaded = $state(false);
  let loadError = $state<string | null>(null);
  let text = $state('');
  let layer = $state('all');
  let scope = $state('all');
  let trust = $state('all');
  let stateFilter = $state('all');
  let selected = $state<string | null>(null);

  /** Numer ostatniego zapytania: wyszukiwanie idzie przy każdym klawiszu, starsze odpowiedzi odpadają. */
  let requestSeq = 0;

  // Błąd pokazujemy w miejscu listy (z „Ponów"), nie toastem — zapytanie leci przy każdym klawiszu.
  async function refresh() {
    const seq = ++requestSeq;
    const result = await load(() =>
      Promise.all([
        app.client.memory.scopes(),
        app.client.memory.inspect({
          scopes: scope === 'all' ? [] : [scope],
          text: text.trim() || null,
          layers: layer === 'all' ? [] : [layer as MemoryLayer],
          states: stateFilter === 'all' ? [] : [stateFilter as MemoryState],
          trusted: trust === 'all' ? null : trust === 'trusted',
          pinned: null,
          offset: 0,
          limit: 100,
        }),
      ]),
    );
    if (seq !== requestSeq) return;
    if (result.status === 'ready') {
      const [list, page] = result.value;
      scopes = list;
      items = page.items;
      total = page.total;
      loadError = null;
    } else if (result.status === 'failed') {
      loadError = result.error;
    }
    loaded = true;
  }

  $effect(() => {
    void [sessionId, text, layer, scope, trust, stateFilter];
    void refresh();
  });

  $effect(() =>
    app.on((event) => {
      if (event.type === 'MemoryChanged') void refresh();
    }),
  );

  const pending = $derived(items.filter((i) => i.state === 'pending').length);
  const scopeLabel = (key: string): string => scopes.find((s) => s.key === key)?.label ?? key;
  const scopeOptions = $derived([
    { value: 'all', label: t('memory.scope.all') },
    ...scopes.map((s) => ({
      value: s.key,
      label: `${t(`memory.scopeKind.${s.scope.kind}`)}: ${s.label}`,
    })),
  ]);
</script>

<div class="memory">
  <div class="filters" role="group" aria-label={t('memory.filters')}>
    <TextField label={t('memory.search')} type="search" bind:value={text} />
    <Select label={t('memory.scope')} size="sm" bind:value={scope} options={scopeOptions} />
    <Select
      label={t('memory.layer')}
      size="sm"
      bind:value={layer}
      options={[
        { value: 'all', label: t('memory.layer.all') },
        ...LAYERS.map((l) => ({ value: l, label: t(`memory.layer.${l}`) })),
      ]}
    />
    <Select
      label={t('memory.trust')}
      size="sm"
      bind:value={trust}
      options={[
        { value: 'all', label: t('memory.trust.all') },
        { value: 'trusted', label: t('memory.trust.trusted') },
        { value: 'untrusted', label: t('memory.trust.untrusted') },
      ]}
    />
    <Select
      label={t('memory.state')}
      size="sm"
      bind:value={stateFilter}
      options={[
        { value: 'all', label: t('memory.state.all') },
        ...STATES.map((s) => ({ value: s, label: t(`memory.state.${s}`) })),
      ]}
    />
  </div>
  <p class="meta" aria-live="polite">
    {t('memory.count', { n: total })}{#if pending}
      · {t('memory.pendingCount', { n: pending })}{/if}
  </p>

  {#if loadError}
    <LoadFailed error={loadError} onretry={() => void refresh()} />
  {:else if !loaded}
    <Loading />
  {:else if items.length === 0}
    <EmptyState title={t('panel.memory')} description={t('memory.empty')}>
      {#snippet icon()}<Brain size={20} strokeWidth={1.5} />{/snippet}
    </EmptyState>
  {:else}
    <ul class="list" aria-label={t('memory.list')}>
      {#each items as item (item.id)}
        <li>
          <button
            type="button"
            class="item"
            aria-current={item.id === selected ? 'true' : undefined}
            aria-expanded={item.id === selected}
            onclick={() => (selected = selected === item.id ? null : item.id)}
          >
            <span class="text" class:muted={item.state !== 'active'}>{item.text}</span>
            <span class="chips">
              <span class="tag">{t(`memory.layer.${item.layer}`)}</span>
              <span class="tag">{scopeLabel(item.scope_key)}</span>
              {#if item.state !== 'active'}<span class="tag state"
                  >{t(`memory.state.${item.state}`)}</span
                >{/if}
              {#if !item.trusted}<span class="tag warn">{t('memory.untrustedChip')}</span>{/if}
              {#if item.pinned}<Pin size={14} aria-label={t('memory.pinned')} />{/if}
            </span>
          </button>
          {#if item.id === selected}
            <MemoryDetail entryId={item.id} {scopes} onclose={() => (selected = null)} />
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .memory {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
  }
  .filters {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--alfa-space-2);
  }
  .filters > :global(:first-child) {
    grid-column: 1 / -1;
  }
  .meta {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .item {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    width: 100%;
    padding: var(--alfa-space-2);
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text);
    text-align: left;
  }
  .item:hover,
  .item[aria-current='true'] {
    background: var(--alfa-color-surface2);
  }
  .text {
    font-size: var(--alfa-font-size-sm);
    overflow-wrap: anywhere;
  }
  .text.muted {
    color: var(--alfa-color-text-muted);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-1);
  }
  .tag {
    padding: 0 var(--alfa-space-1);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .tag.state {
    color: var(--alfa-color-text);
  }
  .tag.warn {
    border-color: var(--alfa-color-warning);
    color: var(--alfa-color-text);
  }
</style>
