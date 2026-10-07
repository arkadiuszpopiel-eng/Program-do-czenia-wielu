<!--
  Kreator „Dodaj dostawcę" → krok „Dostawca": katalog z filtrem rozmytym. Błąd wczytania katalogu
  to „Nie udało się wczytać" z „Ponów" — nie pusta lista, z której nie da się nic wybrać.
-->
<script lang="ts">
  import { TextField } from '@alfa/ui-kit';
  import type { ProviderInfo } from '../../../api/types-hub';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import { fuzzyRank } from '../../../logic/fuzzy';
  import { load, type Loadable } from '../../../state/attempt';
  import { useApp } from '../../../state/context';

  interface Props {
    onchoose: (provider: ProviderInfo) => void;
  }

  let { onchoose }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  let loaded = $state<Loadable<readonly ProviderInfo[]>>({ status: 'loading' });
  let filter = $state('');
  const catalog = $derived(loaded.status === 'ready' ? loaded.value : []);
  const shown = $derived(
    filter.trim()
      ? fuzzyRank(catalog, filter, (p) => ({ label: p.display_name, keywords: [p.id] })).map(
          (r) => r.item,
        )
      : catalog,
  );

  async function loadCatalog() {
    loaded = { status: 'loading' };
    loaded = await load(() => app.client.accounts.catalog());
  }

  $effect(() => {
    void loadCatalog();
  });
</script>

{#if loaded.status === 'failed'}
  <LoadFailed error={loaded.error} onretry={() => void loadCatalog()} />
{:else}
  <TextField label={t('wiz.filter')} type="search" bind:value={filter} />
  <ul class="providers">
    {#each shown as p (p.id)}
      <li>
        <button type="button" class="provider" onclick={() => onchoose(p)}>
          <span class="p-name">{p.display_name}</span>
          <span class="p-meta"
            >{t('hub.privacy', { tag: p.privacy_tag, jurisdiction: p.jurisdiction })} · {t(
              `hub.compliance.${p.compliance_status}`,
            )}</span
          >
        </button>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .providers {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: var(--alfa-space-2);
    max-height: 320px;
    margin: 0;
    padding: 0;
    overflow: auto;
    list-style: none;
  }
  .provider {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    width: 100%;
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text);
    text-align: left;
  }
  .provider:hover {
    border-color: var(--alfa-color-border-strong);
  }
  .p-name {
    font-weight: var(--alfa-weight-semibold);
    font-size: var(--alfa-font-size-sm);
  }
  .p-meta {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
</style>
