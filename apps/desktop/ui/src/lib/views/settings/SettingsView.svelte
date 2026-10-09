<!--
  Ustawienia (PLAN §15, makieta 11): nawigacja (`SettingsNav`) + strona. Każde ustawienie ma opis,
  domyślną, zakres i reset. Strony z późniejszych fal pokazują, co się na nich pojawi.
-->
<script lang="ts">
  import { Button, EmptyState } from '@alfa/ui-kit';
  import Hourglass from '@lucide/svelte/icons/hourglass';
  import type { SettingsPageDef } from '../../api/types-system';
  import LoadFailed from '../../components/shell/LoadFailed.svelte';
  import { bestScore } from '../../logic/fuzzy';
  import { groupSettingsPages } from '../../logic/settings-groups';
  import { hitsByPage } from '../../logic/settings-nav';
  import { load, type Loadable } from '../../state/attempt';
  import { useApp } from '../../state/context';
  import SettingRow from './SettingRow.svelte';
  import SettingsNav from './SettingsNav.svelte';
  import BuilderPage from './pages/BuilderPage.svelte';
  import ComputerPage from './pages/ComputerPage.svelte';
  import CostsPage from './pages/CostsPage.svelte';
  import DevicesPage from './pages/DevicesPage.svelte';
  import HealthPage from './pages/HealthPage.svelte';
  import MarshalPage from './pages/MarshalPage.svelte';
  import MemoryPage from './pages/MemoryPage.svelte';
  import PermissionsPage from './pages/PermissionsPage.svelte';
  import PluginsPage from './pages/PluginsPage.svelte';
  import ApiPage from './pages/ApiPage.svelte';
  import BridgesSection from './pages/BridgesSection.svelte';
  import ProvidersPage from './pages/ProvidersPage.svelte';
  import ShortcutsPage from './pages/ShortcutsPage.svelte';
  import SkillsPage from './pages/SkillsPage.svelte';
  import TransferPage from './pages/TransferPage.svelte';
  import TriggersPage from './pages/TriggersPage.svelte';
  import UpdatesPage from './pages/UpdatesPage.svelte';
  import AboutPage from './pages/AboutPage.svelte';
  import VoicePage from './pages/VoicePage.svelte';
  import Lazy from '../../components/shell/Lazy.svelte';

  const app = useApp();
  const { t } = app.i18n;
  let loaded = $state<Loadable<readonly SettingsPageDef[]>>({ status: 'loading' });
  const schema = $derived(loaded.status === 'ready' ? loaded.value : []);
  const groups = $derived(groupSettingsPages(schema));
  let query = $state('');

  // Bez schematu nie ma żadnej sekcji — błąd rdzenia pokazujemy z „Ponów" zamiast pustego widoku.
  async function loadSchema() {
    loaded = { status: 'loading' };
    loaded = await load(() => app.client.settings.schema());
  }

  $effect(() => {
    void loadSchema();
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
  const hits = $derived(hitsByPage(results, pageMatches));

  function open(id: string) {
    query = '';
    app.settingsPage = id;
    requestAnimationFrame(() => document.getElementById('settings-page-title')?.focus());
  }
</script>

<div class="settings">
  <SettingsNav {groups} current={page?.id} bind:query {hits} onopen={open} />
  <section class="content" aria-labelledby="settings-page-title">
    <div class="page">
      {#if loaded.status === 'failed'}
        <h2 class="h2" id="settings-page-title" tabindex="-1">{t('settings.title')}</h2>
        <LoadFailed error={loaded.error} onretry={() => void loadSchema()} />
      {:else if query.trim()}
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
          <EmptyState title={t('settings.later')} description={t('settings.laterList')}>
            {#snippet icon()}<Hourglass size={20} strokeWidth={1.5} />{/snippet}
            <ul class="upcoming">
              {#each page.upcoming as item (item.pl)}<li>{app.i18n.text(item)}</li>{/each}
            </ul>
          </EmptyState>
        {:else}
          {#if page.custom === 'providers'}<ProvidersPage />
          {:else if page.custom === 'api'}<ApiPage />
          {:else if page.custom === 'bridges'}<BridgesSection />
          {:else if page.custom === 'costs'}<CostsPage />
          {:else if page.custom === 'shortcuts'}<ShortcutsPage />
          {:else if page.custom === 'transfer'}<TransferPage />
          {:else if page.custom === 'permissions'}<PermissionsPage />
          {:else if page.custom === 'devices'}<DevicesPage />
          {:else if page.custom === 'voice'}<VoicePage />
          {:else if page.custom === 'memory'}<MemoryPage />
          {:else if page.custom === 'triggers'}<TriggersPage />
          {:else if page.custom === 'marshal'}<MarshalPage />
          {:else if page.custom === 'skills'}<SkillsPage />
          {:else if page.custom === 'builder'}<BuilderPage />
          {:else if page.custom === 'computer'}<ComputerPage />
          {:else if page.custom === 'health'}<HealthPage />
          {:else if page.custom === 'updates'}<UpdatesPage />
          {:else if page.custom === 'about'}<AboutPage />
          {:else if page.custom === 'plugins'}<PluginsPage />
          {:else if page.custom === 'models'}<Lazy
              load={() => import('./pages/ModelsPage.svelte')}
              props={{}}
              label={t('engines.loading')}
            />
          {/if}
          {#if page.custom && page.settings.length > 0}
            <h3 class="h3">{t('settings.pageSettings')}</h3>
          {/if}
          {#each page.settings as def (def.key)}
            <SettingRow {def} />
          {/each}
        {/if}
      {/if}
    </div>
  </section>
</div>

<style>
  .settings {
    display: flex;
    height: 100%;
    min-height: 0;
  }
  .content {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: var(--alfa-space-6) var(--alfa-space-8) var(--alfa-space-12);
  }
  /* Strona ustawień: wyśrodkowana kolumna, która rośnie z oknem (do 1040 px), zamiast wąskiego
     paska przy lewej krawędzi. */
  .page {
    width: min(1040px, 100%);
    margin: 0 auto;
  }
  .h2 {
    margin-bottom: var(--alfa-space-4);
    font-size: var(--alfa-font-size-2xl);
  }
  .h2:focus {
    outline: none;
  }
  .h3 {
    margin: var(--alfa-space-4) 0 var(--alfa-space-2);
    font-size: var(--alfa-font-size-lg);
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
    .content {
      padding: var(--alfa-space-4);
    }
  }
</style>
