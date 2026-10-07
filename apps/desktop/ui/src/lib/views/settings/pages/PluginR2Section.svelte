<!--
  „Zdrowie systemu” → propozycje wtyczek (pierścień R2) i problemy wtyczek: karta R2 z `r2_proposal`
  (klucz `plugins.<id>.version` = hash przejrzanej wersji, nowe zdolności). Zatwierdzenie odbywa się
  wyłącznie na karcie wtyczki w Ustawienia → „Wtyczki” (przycisk przenosi tam).
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { PluginsView } from '../../../api/types-plugins';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import { load } from '../../../state/attempt';
  import { useApp } from '../../../state/context';
  import './work.css';

  const app = useApp();
  const { t } = app.i18n;
  let view = $state<PluginsView | null>(null);
  const proposals = $derived(
    (view?.plugins ?? []).filter((p) => p.state === 'proposed' && p.r2 !== null),
  );

  let loadError = $state<string | null>(null);

  async function reload() {
    const result = await load(() => app.client.plugins.list());
    view = result.status === 'ready' ? result.value : null;
    loadError = result.status === 'failed' ? result.error : null;
  }

  $effect(() => {
    void reload();
  });
</script>

{#if loadError}
  <section class="wk-card" aria-labelledby="hl-plugins-failed">
    <h3 id="hl-plugins-failed">{t('plugins.healthTitle')}</h3>
    <LoadFailed error={loadError} onretry={() => void reload()} />
  </section>
{/if}

{#if proposals.length || view?.problems.length}
  <section class="wk-card" aria-labelledby="hl-plugins">
    <h3 id="hl-plugins">{t('plugins.healthTitle')}</h3>
    {#if view?.problems.length}
      <p class="wk-warn">{t('plugins.healthProblems', { n: view.problems.length })}</p>
    {/if}
    <ul class="wk-list">
      {#each proposals as p (`${p.id}@${p.version}`)}
        <li>
          <strong>{p.id} {p.version}</strong>
          <span class="wk-meta">{t('plugins.r2Note', { key: p.r2?.key ?? '' })}</span>
          {#if p.r2?.added_capabilities.length}
            <span class="wk-warn"
              >{t('plugins.r2Added', { caps: p.r2.added_capabilities.join(', ') })}</span
            >
          {/if}
        </li>
      {/each}
    </ul>
    <div class="wk-actions">
      <Button size="sm" variant="secondary" onclick={() => (app.settingsPage = 'plugins')}
        >{t('plugins.openPage')}</Button
      >
    </div>
  </section>
{/if}
