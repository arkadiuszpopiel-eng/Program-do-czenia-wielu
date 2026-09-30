<!--
  Ustawienia (PLAN §15, makieta 11): drzewo sekcji + wyszukiwarka. Każde ustawienie ma opis,
  domyślną, zakres i reset. Strony z późniejszych fal pokazują, co się na nich pojawi.
-->
<script lang="ts">
  import { Button, EmptyState } from '@alfa/ui-kit';
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import Search from '@lucide/svelte/icons/search';
  import Hourglass from '@lucide/svelte/icons/hourglass';
  import type { SettingsPageDef } from '../../api/types-system';
  import { bestScore } from '../../logic/fuzzy';
  import { useApp } from '../../state/context';
  import SettingRow from './SettingRow.svelte';
  import CostsPage from './pages/CostsPage.svelte';
  import DevicesPage from './pages/DevicesPage.svelte';
  import PermissionsPage from './pages/PermissionsPage.svelte';
  import ProvidersPage from './pages/ProvidersPage.svelte';
  import ShortcutsPage from './pages/ShortcutsPage.svelte';
  import TransferPage from './pages/TransferPage.svelte';

  const app = useApp();
  const { t } = app.i18n;
  let schema = $state<readonly SettingsPageDef[]>([]);
  let query = $state('');

  $effect(() => {
    void app.client.settings.schema().then((s) => (schema = s));
  });

  const page = $derived(schema.find((p) => p.id === app.settingsPage) ?? schema[0]);
  const results = $derived.by(() => {
    const q = query.trim();
    if (!q) return [];
    return schema
      .flatMap((p) =>
        p.settings.map((def) => ({
          page: p,
          def,
          score: bestScore(q, app.i18n.text(def.label), [
            app.i18n.text(def.description),
            app.i18n.text(p.label),
            def.key,
          ]),
        })),
      )
      .filter((r) => r.score > 0)
      .sort((a, b) => b.score - a.score);
  });
  const pageMatches = $derived(
    query.trim() ? schema.filter((p) => bestScore(query, app.i18n.text(p.label)) > 0.5) : [],
  );

  function open(id: string) {
    query = '';
    app.settingsPage = id;
    requestAnimationFrame(() => document.getElementById('settings-page-title')?.focus());
  }
</script>

<div class="settings">
  <aside class="nav">
    <Button size="sm" variant="ghost" onclick={() => app.closeSettings()}>
      {#snippet icon()}<ArrowLeft size={14} strokeWidth={1.5} />{/snippet}
      {t('settings.back')}
    </Button>
    <h1 class="h1">{t('settings.title')}</h1>
    <div class="search">
      <Search size={14} strokeWidth={1.5} aria-hidden="true" />
      <label class="alfa-visually-hidden" for="alfa-settings-search">{t('settings.search')}</label>
      <input
        id="alfa-settings-search"
        type="search"
        placeholder={t('settings.searchPlaceholder')}
        bind:value={query}
      />
    </div>
    <nav aria-label={t('settings.tree')}>
      <ul class="tree">
        {#each schema as p (p.id)}
          <li>
            <button
              type="button"
              class="node"
              class:later={p.wave > 1}
              aria-current={p.id === page?.id && !query ? 'page' : undefined}
              onclick={() => open(p.id)}
            >
              <span>{app.i18n.text(p.label)}</span>
              {#if p.wave > 1}<span class="wave">{t('settings.wave', { n: p.wave })}</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    </nav>
  </aside>
  <section class="content" aria-labelledby="settings-page-title">
    {#if query.trim()}
      <h2 class="h2" id="settings-page-title" tabindex="-1" aria-live="polite">
        {results.length
          ? t('settings.results', { n: results.length })
          : t('settings.noResults', { query: query.trim() })}
      </h2>
      {#if pageMatches.length}
        <div class="page-hits">
          {#each pageMatches as p (p.id)}
            <Button size="sm" variant="secondary" onclick={() => open(p.id)}
              >{app.i18n.text(p.label)}</Button
            >
          {/each}
        </div>
      {/if}
      {#each results as r (r.def.key)}
        <SettingRow def={r.def} page={app.i18n.text(r.page.label)} />
      {/each}
    {:else if page}
      <h2 class="h2" id="settings-page-title" tabindex="-1">{app.i18n.text(page.label)}</h2>
      {#if page.wave > 1}
        <EmptyState
          title={t('settings.later', { wave: page.wave })}
          description={t('settings.laterList')}
        >
          {#snippet icon()}<Hourglass size={20} strokeWidth={1.5} />{/snippet}
          <ul class="upcoming">
            {#each page.upcoming as item (item.pl)}<li>{app.i18n.text(item)}</li>{/each}
          </ul>
        </EmptyState>
      {:else}
        {#if page.custom === 'providers'}<ProvidersPage />
        {:else if page.custom === 'costs'}<CostsPage />
        {:else if page.custom === 'shortcuts'}<ShortcutsPage />
        {:else if page.custom === 'transfer'}<TransferPage />
        {:else if page.custom === 'permissions'}<PermissionsPage />
        {:else if page.custom === 'devices'}<DevicesPage />
        {/if}
        {#each page.settings as def (def.key)}
          <SettingRow {def} />
        {/each}
      {/if}
    {/if}
  </section>
</div>

<style>
  .settings {
    display: flex;
    height: 100%;
    min-height: 0;
  }
  .nav {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    width: 260px;
    flex: none;
    padding: var(--alfa-space-3);
    overflow: auto;
    border-right: 1px solid var(--alfa-color-border);
    background: var(--alfa-color-surface);
  }
  .nav > :global(button:first-child) {
    align-self: flex-start;
  }
  .h1 {
    padding: var(--alfa-space-2) var(--alfa-space-2) 0;
    font-size: var(--alfa-font-size-xl);
  }
  .search {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    height: 32px;
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text-subtle);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: transparent;
  }
  .search input:focus {
    outline: none;
  }
  .search:focus-within {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
    outline-offset: 1px;
  }
  .tree {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .node {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    min-height: 30px;
    padding: 0 var(--alfa-space-2);
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-sm);
    text-align: left;
  }
  .node:hover {
    background: var(--alfa-color-surface2);
  }
  .node[aria-current='page'] {
    background: var(--alfa-color-surface3);
    font-weight: var(--alfa-weight-semibold);
  }
  .node.later {
    color: var(--alfa-color-text-muted);
  }
  .wave {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .content {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: var(--alfa-space-6) var(--alfa-space-8) var(--alfa-space-12);
  }
  .content > :global(*) {
    max-width: 760px;
  }
  .h2 {
    margin-bottom: var(--alfa-space-4);
    font-size: var(--alfa-font-size-2xl);
  }
  .h2:focus {
    outline: none;
  }
  .page-hits {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
    margin-bottom: var(--alfa-space-3);
  }
  .upcoming {
    margin: 0;
    padding-left: var(--alfa-space-4);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
    text-align: left;
  }
  @media (max-width: 720px) {
    .settings {
      flex-direction: column;
    }
    .nav {
      width: 100%;
      max-height: 40%;
      border-right: 0;
      border-bottom: 1px solid var(--alfa-color-border);
    }
    .content {
      padding: var(--alfa-space-4);
    }
  }
</style>
