<!--
  Testy podzespołów: wybór (lekkie i dyski), uruchomienie z postępem (stan odpytywany co ~0,8 s
  w trakcie), przerwanie i wyniki ostatniego przebiegu z miarami, uwagami i zmianą względem
  poprzedniego pomiaru.
-->
<script lang="ts">
  import { Button, Checkbox } from '@alfa/ui-kit';
  import { errorText } from '../../../api/command-error';
  import type { CheckRun, DiagProgress } from '../../../api/types-diag';
  import {
    QUICK_TESTS,
    change,
    diskKeys,
    fmtValue,
    keyOf,
    mainMetric,
    testLabelKey,
    trend,
    verdictClass,
  } from '../../../logic/diag';
  import { useApp } from '../../../state/context';
  import './work.css';

  interface Props {
    available: readonly string[];
    lastRun: CheckRun | null;
    history: readonly CheckRun[];
    ondone: () => void;
  }

  let { available, lastRun, history, ondone }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  let selected = $state<Record<string, boolean>>({});
  let running = $state(false);
  let progress = $state<DiagProgress | null>(null);
  const disks = $derived(diskKeys(available));
  const chosen = $derived(available.filter((k) => selected[k]));

  function label(key: string): string {
    const l = testLabelKey(key);
    return l.letter ? t('diag.test.disk', { letter: l.letter }) : tk(l.key);
  }

  async function run(keys: readonly string[]) {
    if (!keys.length || running) return;
    running = true;
    const timer = setInterval(() => {
      void app.client.diag.state().then((s) => (progress = s.running));
    }, 800);
    try {
      await app.client.diag.run(keys);
    } catch (e) {
      app.toasts.show({ kind: 'error', message: errorText(e) });
    } finally {
      clearInterval(timer);
      running = false;
      progress = null;
      ondone();
    }
  }

  function trendText(key: string): string | null {
    const metric = mainMetric(key);
    if (!metric) return null;
    const values = trend(history, key, metric);
    const pct = change(values);
    if (pct === null) return null;
    const prev = values[values.length - 2]!;
    return t('diag.trend', { prev: prev.toFixed(1), pct: pct.toFixed(0) });
  }
</script>

<section class="wk-card" aria-labelledby="diag-tests">
  <h3 id="diag-tests">{t('diag.tests')}</h3>
  <p class="wk-meta">{t('diag.testsHint')}</p>
  <div class="cols">
    <fieldset>
      <legend>{t('diag.quick')}</legend>
      {#each QUICK_TESTS as key (key)}
        <Checkbox
          label={label(key)}
          checked={selected[key] ?? false}
          disabled={running}
          onchange={(on) => (selected = { ...selected, [key]: on })}
        />
      {/each}
    </fieldset>
    <fieldset>
      <legend>{t('diag.disks')}</legend>
      {#each disks as key (key)}
        <Checkbox
          label={label(key)}
          checked={selected[key] ?? false}
          disabled={running}
          onchange={(on) => (selected = { ...selected, [key]: on })}
        />
      {/each}
    </fieldset>
  </div>
  <div class="wk-actions">
    <Button variant="primary" disabled={running} onclick={() => void run(QUICK_TESTS)}
      >{t('diag.runQuick')}</Button
    >
    <Button
      variant="secondary"
      disabled={running || chosen.length === 0}
      onclick={() => void run(chosen)}>{t('diag.run')}</Button
    >
    {#if running}
      <Button variant="secondary" onclick={() => void app.client.diag.cancel()}
        >{t('diag.cancel')}</Button
      >
    {/if}
  </div>
  {#if running}
    <p role="status" aria-live="polite">
      {progress
        ? t('diag.progress', {
            n: progress.index + 1,
            total: progress.total,
            name: label(progress.current),
          })
        : t('diag.recording')}
    </p>
  {/if}

  {#if lastRun}
    <h4>
      {t('diag.lastRun', {
        at: app.i18n.dateTime(lastRun.at),
        verdict: tk(`diag.verdict.${lastRun.overall}`),
      })}
    </h4>
    <ul class="wk-list">
      {#each lastRun.results as r, i (i)}
        {@const key = keyOf(r.kind)}
        <li>
          <div class="wk-actions">
            <strong>{label(key)}</strong>
            <span class={verdictClass(r.verdict)}>{tk(`diag.verdict.${r.verdict}`)}</span>
            <span class="wk-meta"
              >{t('diag.duration', { s: (r.duration_ms / 1000).toFixed(1) })}</span
            >
          </div>
          {#if r.metrics.length}
            <span class="wk-meta">
              {r.metrics.map((m) => `${m.label}: ${fmtValue(m)}`).join(' · ')}
            </span>
          {/if}
          {#if trendText(key)}<span class="wk-meta">{trendText(key)}</span>{/if}
          {#each r.notes as n, j (j)}<span>{n}</span>{/each}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(16rem, 1fr));
    gap: var(--alfa-space-3);
  }
  fieldset {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    margin: 0;
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
  }
  legend {
    padding: 0 var(--alfa-space-1);
    font-weight: 600;
  }
</style>
