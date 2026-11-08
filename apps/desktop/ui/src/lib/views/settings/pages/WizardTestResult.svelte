<!--
  Wynik testu klucza (kreator dostawcy i lista kont): co się stało, co zrobić i — na życzenie — co
  dokładnie odpowiedział dostawca. Przyciski zależą od przyczyny: przy złym kluczu główną akcją jest
  jego poprawienie, ale gdy klucz może być dobry (brak środków, sieć, odmowa z innego powodu) —
  ponowne sprawdzenie, a konto można zostawić. Klucz bez prawa do listy modeli wymaga podania
  nazwy modelu: bez niej Alfa nie miałaby czego wywołać.
-->
<script lang="ts">
  import { Button, TextField } from '@alfa/ui-kit';
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import CircleX from '@lucide/svelte/icons/circle-x';
  import type { TestReport } from '../../../api/types-hub';
  import { FIX_STEPS, KEY_HELP, actionFor, verdictOf } from '../../../logic/api-key';
  import { useApp } from '../../../state/context';

  interface Props {
    report: TestReport | null;
    testing: boolean;
    providerId: string;
    providerName: string;
    busy: boolean;
    onnext: () => void;
    /** Ponowny test zapisanego klucza; brak = nie ma czego testować ponownie (klucza nie zapisano). */
    onretry?: () => void;
    onfix: () => void;
    /** Zamknięcie panelu bez zmian (lista kont). */
    onclose?: () => void;
    /** Zostawienie konta mimo nieudanego testu (klucz może być dobry). */
    onkeep?: () => void;
    /** Zapis nazwy modelu podanej ręcznie (klucz bez prawa do listy modeli). */
    onmodel?: (model: string) => Promise<void>;
    /** Dodatkowe zdanie pod wyjaśnieniem (np. „stary klucz został zachowany”). */
    note?: string;
    nextLabel?: string;
    fixLabel?: string;
    keepLabel?: string;
  }

  let {
    report,
    testing,
    providerId,
    providerName,
    busy,
    onnext,
    onretry,
    onfix,
    onclose,
    onkeep,
    onmodel,
    note,
    nextLabel,
    fixLabel,
    keepLabel,
  }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  const uid = $props.id();

  const verdict = $derived(report ? verdictOf(report) : 'ok');
  const action = $derived(actionFor(verdict));
  const tone = $derived(
    verdict === 'ok' ? 'good' : action === 'works' || verdict === 'unsupported' ? 'warn' : 'bad',
  );
  const steps = $derived(Array.from({ length: FIX_STEPS[verdict] }, (_, i) => i + 1));
  /** Adres strony z kluczami albo ogólne określenie, gdy go nie znamy. */
  const url = $derived(KEY_HELP[providerId]?.url ?? t('wiz.providerSite'));
  /** Można iść dalej, ale Alfa nie zna żadnego modelu — trzeba go podać. */
  const needsModel = $derived(
    action === 'works' && report?.models.length === 0 && Boolean(onmodel),
  );
  /** Co się zmieniło w wyniku — sama nowa kopia tego samego raportu nie zabiera fokusu z pola. */
  const shown = $derived(
    report && !testing ? `${verdict}|${report.status ?? ''}|${report.latency_ms ?? ''}` : '',
  );
  let heading = $state<HTMLElement | null>(null);
  let model = $state('');
  let savingModel = $state(false);

  // Nowy wynik = fokus na jego tytule (przycisk, który go wywołał, już nie istnieje).
  $effect(() => {
    if (shown) heading?.focus();
  });

  async function saveModel(event: SubmitEvent) {
    event.preventDefault();
    if (!onmodel || !model.trim() || savingModel) return;
    savingModel = true;
    try {
      await onmodel(model.trim());
      model = '';
    } finally {
      savingModel = false;
    }
  }
</script>

{#if testing}
  <p class="testing" role="status">{t('wiz.testing', { name: providerName })}</p>
{:else if report}
  <section class="result {tone}" data-verdict={verdict} aria-labelledby="{uid}-verdict">
    <h4 id="{uid}-verdict" tabindex="-1" bind:this={heading}>
      {#if tone === 'good'}
        <CircleCheck size={18} strokeWidth={1.75} aria-hidden="true" />
      {:else if tone === 'warn'}
        <CircleAlert size={18} strokeWidth={1.75} aria-hidden="true" />
      {:else}
        <CircleX size={18} strokeWidth={1.75} aria-hidden="true" />
      {/if}
      {tk(`wiz.verdict.${verdict}`, { name: providerName })}
    </h4>
    {#if verdict === 'ok'}
      <p>
        {t('wiz.testOk', { latency: app.i18n.duration(report.latency_ms ?? 0) })}
        · {t('wiz.modelsFound', { n: report.models.length })}
      </p>
      {#if note}
        <p class="note">{note}</p>
      {/if}
    {:else}
      <p>{tk(`wiz.why.${verdict}`, { name: providerName })}</p>
      {#if verdict === 'network' && report.error}
        <p class="raw">{report.error}</p>
      {/if}
      {#if note}
        <p class="note">{note}</p>
      {/if}
      {#if needsModel}
        <form class="model" onsubmit={saveModel}>
          <TextField
            label={t('wiz.modelName')}
            bind:value={model}
            placeholder="gpt-4o-mini"
            autocomplete="off"
            spellcheck={false}
          />
          <Button
            variant="secondary"
            type="submit"
            disabled={!model.trim() || savingModel}
            loading={savingModel}>{t('wiz.modelSave')}</Button
          >
        </form>
      {:else if action === 'works' && report.models.length === 1 && report.models[0]}
        <p class="note">{t('wiz.modelSet', { model: report.models[0].id })}</p>
      {/if}
      {#if steps.length}
        <h5>{t('wiz.fixTitle')}</h5>
        <ol>
          {#each steps as n (n)}
            <li>{tk(`wiz.fix.${verdict}.${n}`, { name: providerName, url })}</li>
          {/each}
        </ol>
      {/if}
      {#if report.detail || report.status}
        <details>
          <summary>{t('wiz.detailTitle', { name: providerName })}</summary>
          <dl>
            {#if report.status}
              <dt>{t('wiz.detail.status')}</dt>
              <dd>{report.status}</dd>
            {/if}
            {#if report.provider_code}
              <dt>{t('wiz.detail.code')}</dt>
              <dd><code>{report.provider_code}</code></dd>
            {/if}
            {#if report.detail}
              <dt>{t('wiz.detail.message')}</dt>
              <dd lang="en">{report.detail}</dd>
            {/if}
          </dl>
        </details>
      {/if}
    {/if}
  </section>
  <div class="actions">
    {#if action === 'works'}
      {#if verdict !== 'ok'}
        <Button variant="ghost" disabled={busy} loading={busy} onclick={onfix}
          >{fixLabel ?? t('wiz.fixKey')}</Button
        >
        {#if onretry}
          <Button variant="secondary" onclick={onretry}>{t('wiz.testAgain')}</Button>
        {/if}
      {/if}
      <Button variant="primary" disabled={needsModel} onclick={onnext}
        >{nextLabel ?? t('common.next')}</Button
      >
    {:else}
      {#if onclose}
        <Button variant="ghost" onclick={onclose}>{t('common.close')}</Button>
      {/if}
      {#if action === 'keep' && onretry}
        <Button variant="ghost" disabled={busy} loading={busy} onclick={onfix}
          >{fixLabel ?? t('wiz.fixKey')}</Button
        >
        {#if onkeep}
          <Button variant="secondary" onclick={onkeep}>{keepLabel ?? t('wiz.keepAccount')}</Button>
        {/if}
        <Button variant="primary" onclick={onretry}>{t('wiz.testAgain')}</Button>
      {:else}
        {#if onretry}
          <Button variant="secondary" onclick={onretry}>{t('wiz.testAgain')}</Button>
        {/if}
        <Button variant="primary" disabled={busy} loading={busy} onclick={onfix}
          >{fixLabel ?? t('wiz.fixKey')}</Button
        >
      {/if}
    {/if}
  </div>
{/if}

<style>
  .testing {
    margin: 0;
    color: var(--alfa-color-text-muted);
  }
  .result {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-3) var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-inline-start: 3px solid var(--tone);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface2);
    font-size: var(--alfa-font-size-sm);
  }
  .good {
    --tone: var(--alfa-color-success);
  }
  .warn {
    --tone: var(--alfa-color-warning);
  }
  .bad {
    --tone: var(--alfa-color-error);
  }
  h4 {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    margin: 0;
    font-size: var(--alfa-font-size-md);
  }
  h4:focus {
    outline: none;
  }
  h4:focus-visible {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
    outline-offset: 2px;
  }
  h4 :global(svg) {
    flex: none;
    color: var(--tone);
  }
  h5 {
    margin: var(--alfa-space-1) 0 0;
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  p {
    margin: 0;
  }
  ol {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    margin: 0;
    padding-left: 1.4em;
  }
  .raw,
  dd,
  li {
    overflow-wrap: anywhere;
  }
  .raw,
  .note {
    color: var(--alfa-color-text-muted);
  }
  .model {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--alfa-space-2);
  }
  .model > :global(:first-child) {
    flex: 1 1 240px;
  }
  details {
    color: var(--alfa-color-text-muted);
  }
  summary {
    cursor: pointer;
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--alfa-space-1) var(--alfa-space-3);
    margin: var(--alfa-space-2) 0 0;
  }
  dt {
    font-weight: var(--alfa-weight-semibold);
  }
  dd {
    margin: 0;
  }
  code {
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-xs);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--alfa-space-2);
  }
</style>
