<!--
  Ustawienia → Diagnostyka komputera: ocena stanu z ostatniego zapisu parametrów, zapis na
  żądanie, raport do pliku, testy podzespołów z postępem, parametry i historia testów.
  Tylko pomiar — nic nie jest naprawiane ani zmieniane w systemie.
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import { errorText } from '../../../api/command-error';
  import type { CheckRun, DiagView } from '../../../api/types-diag';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import Loading from '../../../components/shell/Loading.svelte';
  import { keyOf, testLabelKey, verdictClass } from '../../../logic/diag';
  import { useApp } from '../../../state/context';
  import DiagParams from './DiagParams.svelte';
  import DiagTests from './DiagTests.svelte';
  import './work.css';

  const app = useApp();
  const { t, tk } = app.i18n;
  let view = $state<DiagView | null>(null);
  let history = $state<readonly CheckRun[]>([]);
  let loadError = $state<string | null>(null);
  let recording = $state(false);

  async function load() {
    try {
      const [v, runs] = await Promise.all([app.client.diag.state(), app.client.diag.runs(20)]);
      view = v;
      history = runs;
      loadError = null;
    } catch (e) {
      loadError = errorText(e);
    }
  }

  $effect(() => {
    void load();
  });

  async function record() {
    recording = true;
    try {
      view = await app.client.diag.snapshot();
    } catch (e) {
      app.toasts.show({ kind: 'error', message: errorText(e) });
    } finally {
      recording = false;
    }
  }

  async function exportReport() {
    try {
      const path = await app.client.diag.export();
      app.toasts.show({ kind: 'success', message: t('diag.exported', { path }) });
    } catch (e) {
      app.toasts.show({ kind: 'error', message: errorText(e) });
    }
  }

  function label(key: string): string {
    const l = testLabelKey(key);
    return l.letter ? t('diag.test.disk', { letter: l.letter }) : tk(l.key);
  }
</script>

<section class="wk-card" aria-labelledby="diag-title">
  <h3 id="diag-title">{t('diag.title')}</h3>
  <p>{t('diag.intro')}</p>
  {#if view?.latest}
    <p class="wk-meta">
      {t('diag.recordedAt', {
        at: app.i18n.dateTime(view.latest.at),
        snapshots: view.snapshots,
        runs: view.runs,
      })}
    </p>
  {:else if view}
    <p class="wk-meta">{t('diag.never')}</p>
  {/if}
  <div class="wk-actions">
    <Button variant="primary" disabled={recording} onclick={() => void record()}
      >{recording ? t('diag.recording') : t('diag.record')}</Button
    >
    <Button
      variant="secondary"
      disabled={recording || !view?.latest}
      onclick={() => void exportReport()}>{t('diag.export')}</Button
    >
  </div>
  {#if loadError}<LoadFailed error={loadError} onretry={() => void load()} />
  {:else if !view}<Loading />{/if}
</section>

{#if view?.integrity}
  <section class="wk-card" aria-labelledby="diag-assessment">
    <h3 id="diag-assessment">
      {t('diag.assessment')}:
      <span class={verdictClass(view.integrity.verdict)}
        >{tk(`diag.verdict.${view.integrity.verdict}`)}</span
      >
    </h3>
    <ul class="wk-plain">
      {#each view.integrity.notes as note, i (i)}<li>{note}</li>{/each}
    </ul>
  </section>
{/if}

{#if view}
  <DiagTests
    available={view.available}
    lastRun={view.last_run}
    {history}
    ondone={() => void load()}
  />
{/if}

{#if view?.latest}<DiagParams snap={view.latest} />{/if}

{#if view}
  <section class="wk-card" aria-labelledby="diag-history">
    <h3 id="diag-history">{t('diag.history')}</h3>
    {#if history.length === 0}
      <p class="wk-meta">{t('diag.historyEmpty')}</p>
    {:else}
      <ul class="wk-list">
        {#each history as run, i (i)}
          <li>
            <div class="wk-actions">
              <strong>{app.i18n.dateTime(run.at)}</strong>
              <span class={verdictClass(run.overall)}>{tk(`diag.verdict.${run.overall}`)}</span>
            </div>
            <span class="wk-meta">
              {run.results
                .map((r) => `${label(keyOf(r.kind))}: ${tk(`diag.verdict.${r.verdict}`)}`)
                .join(' · ')}
            </span>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
{/if}
