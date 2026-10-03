<!--
  Ustawienia → O programie: wersja, kanał, data kompilacji, commit, platforma, czy aktualizacje są
  podpisane i skonfigurowane, licencje zależności (Rust + npm) z filtrem. Bez telemetrii.
-->
<script lang="ts">
  import type { AboutInfo } from '../../../api/types-updates';
  import { filterLicenses } from '../../../logic/updates';
  import { useApp } from '../../../state/context';
  import './work.css';

  const app = useApp();
  const { t, tk } = app.i18n;
  let about = $state<AboutInfo | null>(null);
  let error = $state<string | null>(null);
  let query = $state('');

  $effect(() => {
    void app.client.updates.about().then(
      (a) => (about = a),
      (e: unknown) => (error = e instanceof Error ? e.message : String(e)),
    );
  });

  const shown = $derived(about ? filterLicenses(about.licenses, query) : []);
</script>

<section class="wk-card" aria-labelledby="ab-title">
  <h3 id="ab-title">{t('about.title')}</h3>
  {#if error}<p class="wk-error" role="alert">{error}</p>{/if}
  {#if about}
    <dl class="facts">
      <dt>{t('about.version')}</dt>
      <dd data-selectable>{about.version}</dd>
      <dt>{t('about.channel')}</dt>
      <dd>{tk(`updates.channel.${about.channel}`)}</dd>
      <dt>{t('about.built')}</dt>
      <dd>{about.build_date ?? t('about.dev')}</dd>
      {#if about.commit}
        <dt>{t('about.commit')}</dt>
        <dd data-selectable><code>{about.commit}</code></dd>
      {/if}
      <dt>{t('about.target')}</dt>
      <dd>{about.target}</dd>
      <dt>{t('about.updates')}</dt>
      <dd>{about.updates_configured ? t('about.updatesOn') : t('about.updatesOff')}</dd>
    </dl>
    <p class="wk-meta">{t('about.noTelemetry')}</p>
  {/if}
</section>

{#if about}
  <section class="wk-card" aria-labelledby="ab-licenses">
    <h3 id="ab-licenses">{t('about.licenses')}</h3>
    <p class="wk-meta">
      {about.licenses_generated_at
        ? t('about.licensesIntro', { date: about.licenses_generated_at })
        : t('about.licensesUndated')}
    </p>
    <label class="alfa-visually-hidden" for="ab-filter">{t('about.filter')}</label>
    <input
      id="ab-filter"
      class="filter"
      type="search"
      placeholder={t('about.filterPlaceholder')}
      bind:value={query}
    />
    <p class="wk-meta" role="status" aria-live="polite">
      {t('about.count', { n: shown.length })}
    </p>
    <!-- Przewijana tabela musi być osiągalna z klawiatury (axe: scrollable-region-focusable). -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div class="table-wrap" tabindex="0" role="region" aria-labelledby="ab-licenses">
      <table>
        <thead>
          <tr>
            <th scope="col">{t('about.name')}</th>
            <th scope="col">{t('about.license')}</th>
          </tr>
        </thead>
        <tbody>
          {#each shown as l (`${l.source}:${l.name}@${l.version}`)}
            <tr>
              <td data-selectable
                >{l.name}
                <span class="wk-meta">{l.version} · {tk(`about.source.${l.source}`)}</span></td
              >
              <td data-selectable>{l.license}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  </section>
{/if}

<style>
  .facts {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--alfa-space-1) var(--alfa-space-4);
    margin: 0;
  }
  dt {
    color: var(--alfa-color-text-muted);
  }
  dd {
    margin: 0;
  }
  .filter {
    height: 32px;
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text);
  }
  .table-wrap {
    max-height: 420px;
    overflow: auto;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th,
  td {
    padding: var(--alfa-space-1) var(--alfa-space-2);
    border-bottom: 1px solid var(--alfa-color-border);
    text-align: left;
    vertical-align: top;
  }
  th {
    position: sticky;
    top: 0;
    background: var(--alfa-color-surface2);
  }
</style>
