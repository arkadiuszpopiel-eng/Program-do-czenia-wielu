<!--
  Ustawienia → Zdrowie systemu → Dziennik rozruchu: werdykt („Rozruch OK / z uwagami / z problemem”),
  czas, liczba ostrzeżeń i błędów, wpisy od startu programu (filtr „tylko ostrzeżenia i błędy”),
  kopiowanie i ścieżka pliku dziennika. Kopia właściciela nie ma okna konsoli — to jedyny podgląd.
-->
<script lang="ts">
  import { Button, Checkbox } from '@alfa/ui-kit';
  import { errorText } from '../../../api/command-error';
  import { boot, bootSeconds } from '../../../state/boot.svelte';
  import { useApp } from '../../../state/context';
  import './work.css';

  const app = useApp();
  const { t } = app.i18n;
  let onlyProblems = $state(true);
  let loadError = $state<string | null>(null);

  const report = $derived(boot.report);
  const lines = $derived(
    (report?.lines ?? []).filter((l) => !onlyProblems || l.level === 'WARN' || l.level === 'ERROR'),
  );
  const params = $derived({
    s: bootSeconds(app.i18n.locale, report?.elapsed_ms ?? 0),
    w: report?.warnings ?? 0,
    e: report?.errors ?? 0,
  });

  async function refresh() {
    try {
      boot.report = await app.client.system.bootReport();
      loadError = null;
    } catch (error) {
      loadError = errorText(error);
    }
  }

  async function copy() {
    const text = (report?.lines ?? []).map((l) => l.text).join('\n');
    try {
      await navigator.clipboard.writeText(text);
      app.toasts.show({ kind: 'success', message: t('boot.copied') });
    } catch (error) {
      app.toasts.show({ kind: 'error', message: errorText(error) });
    }
  }

  $effect(() => {
    if (!boot.report) void refresh();
  });
</script>

<section class="wk-card" aria-labelledby="hl-boot">
  <h3 id="hl-boot">{t('boot.title')}</h3>
  <p class="wk-meta">{t('boot.intro')}</p>
  {#if loadError}
    <p class="wk-error" role="alert">{t('boot.loadFailed', { error: loadError })}</p>
  {/if}
  {#if report}
    <p
      role="status"
      data-verdict={report.verdict}
      class:wk-ok={report.verdict === 'ok'}
      class:wk-warn={report.verdict === 'warnings'}
      class:wk-error={report.verdict === 'errors'}
    >
      <strong>{t(`boot.verdict.${report.verdict}`)}</strong> · {t('boot.summary', params)}
    </p>
    {#if !report.finished}<p class="wk-note">{t('boot.running')}</p>{/if}
    <Checkbox bind:checked={onlyProblems} label={t('boot.onlyProblems')} />
    {#if lines.length === 0}
      <p class="wk-meta">{t('boot.empty')}</p>
    {:else}
      <div class="wk-diff boot-log" role="log" aria-label={t('boot.title')}>
        {#each lines as l, i (i)}
          <span
            class="boot-line"
            class:removed={l.level === 'ERROR'}
            class:boot-warn={l.level === 'WARN'}>{l.text}</span
          >
        {/each}
      </div>
    {/if}
    {#if report.dropped > 0}<p class="wk-meta">{t('boot.dropped', { n: report.dropped })}</p>{/if}
    {#if report.log_file}<p class="wk-meta">{t('boot.file', { path: report.log_file })}</p>{/if}
  {/if}
  <div class="wk-actions">
    <Button size="sm" variant="secondary" onclick={() => void refresh()}>{t('boot.refresh')}</Button
    >
    <Button size="sm" variant="secondary" disabled={!report} onclick={() => void copy()}
      >{t('boot.copy')}</Button
    >
  </div>
</section>

<style>
  .boot-log {
    max-height: 22rem;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .boot-line {
    display: block;
  }
  .boot-warn {
    color: var(--alfa-color-warning);
  }
</style>
