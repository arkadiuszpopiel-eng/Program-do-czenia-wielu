<!--
  Cennik modeli (Ustawienia → Koszty): ceny, z których liczony jest koszt każdej odpowiedzi (USD za
  1 mln tokenów, przeliczane na PLN po kursie NBP), z datą sprawdzenia i źródłem. Aktualizacja:
  wklejony tekst `[[price]]` (np. wynik obowiązku „Kontrola cenników”) → podgląd zmian → „Zastosuj”.
  Zapis zawsze na kliknięcie właściciela; agentka sama cennika nie zmienia.
-->
<script lang="ts">
  import { Button, Select } from '@alfa/ui-kit';
  import type { PriceRow, PricingView } from '../../../api/types';
  import { errorText } from '../../../api/command-error';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import { load } from '../../../state/attempt';
  import { useApp } from '../../../state/context';
  import './work.css';

  const app = useApp();
  const { t } = app.i18n;
  const uid = $props.id();
  let view = $state<PricingView | null>(null);
  let loadError = $state<string | null>(null);
  let provider = $state('');
  let pasting = $state(false);
  let text = $state('');
  let error = $state<string | null>(null);
  let preview = $state<PricingView | null>(null);
  let busy = $state(false);

  async function reload() {
    const result = await load(() => app.client.costs.pricing());
    loadError = result.status === 'failed' ? result.error : null;
    if (result.status === 'ready') view = result.value;
  }

  $effect(() => {
    void reload();
  });

  const providers = $derived([...new Set((view?.rows ?? []).map((r) => r.provider))].sort());
  const shown = $derived((view?.rows ?? []).filter((r) => !provider || r.provider === provider));
  const number = $derived(
    new Intl.NumberFormat(app.i18n.locale, { maximumFractionDigits: 4, minimumFractionDigits: 2 }),
  );
  const usd = (n: number | null) => (n === null ? '—' : `$${number.format(n)}`);
  const host = (url: string) => {
    try {
      return new URL(url).hostname;
    } catch {
      return url;
    }
  };

  async function run(save: boolean) {
    busy = true;
    error = null;
    try {
      const result = await app.client.costs.applyPricing(text, save);
      if (save) {
        view = result;
        preview = null;
        pasting = false;
        text = '';
        app.toasts.show({
          kind: 'success',
          message: t('pricing.saved', { n: result.changes.length }),
        });
      } else {
        preview = result;
      }
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  function priceText(r: PriceRow): string {
    return t('pricing.inOut', { input: usd(r.input), output: usd(r.output) });
  }
</script>

<section class="card" aria-labelledby="{uid}-h">
  <h3 id="{uid}-h">{t('pricing.title')}</h3>
  <p class="desc">{t('pricing.desc')}</p>
  {#if loadError}
    <LoadFailed error={loadError} onretry={() => void reload()} />
  {:else if view}
    <p class="desc">
      {t('pricing.checked', { date: view.checked, n: view.rows.length })}
    </p>
    <div class="wk-actions">
      <Select
        label={t('pricing.provider')}
        value={provider}
        options={[
          { value: '', label: t('pricing.allProviders') },
          ...providers.map((p) => ({ value: p, label: p })),
        ]}
        onchange={(v: string) => (provider = v)}
      />
      <Button size="sm" variant="secondary" onclick={() => ((pasting = !pasting), (error = null))}
        >{t('pricing.update')}</Button
      >
    </div>
    <div class="table" role="region" aria-label={t('pricing.table')} tabindex="-1">
      <table>
        <thead>
          <tr>
            <th scope="col">{t('pricing.model')}</th>
            <th scope="col">{t('pricing.price')}</th>
            <th scope="col">{t('pricing.cache')}</th>
            <th scope="col">{t('pricing.source')}</th>
          </tr>
        </thead>
        <tbody>
          {#each shown as r (`${r.provider}/${r.model}`)}
            <tr>
              <td>
                <span class="model">{r.model}</span>
                <span class="muted">{r.provider}{r.user ? ` · ${t('pricing.yours')}` : ''}</span>
              </td>
              <td>{priceText(r)}</td>
              <td>{usd(r.cache_read)}</td>
              <td class="muted" title={r.note ?? ''}>{host(r.source)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    {#if pasting}
      <label class="wk-field">
        <span>{t('pricing.paste')}</span>
        <textarea rows="7" spellcheck="false" autocomplete="off" bind:value={text}></textarea>
      </label>
      <p class="desc">{t('pricing.pasteHint')}</p>
      {#if error}<p class="wk-error" role="alert">{error}</p>{/if}
      {#if preview}
        <div role="status">
          {#if preview.changes.length === 0}
            <p class="desc">{t('pricing.noChanges')}</p>
          {:else}
            <p class="desc">{t('pricing.changes', { n: preview.changes.length })}</p>
            <ul class="changes">
              {#each preview.changes as c (`${c.after.provider}/${c.after.model}`)}
                <li>
                  <strong>{c.after.provider} / {c.after.model}</strong>:
                  {c.before ? `${priceText(c.before)} → ` : `${t('pricing.new')} `}{priceText(
                    c.after,
                  )}
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      {/if}
      <div class="wk-actions">
        <Button
          size="sm"
          variant="secondary"
          disabled={busy || !text.trim()}
          onclick={() => void run(false)}>{t('pricing.check')}</Button
        >
        <Button
          size="sm"
          disabled={busy || !preview || preview.changes.length === 0}
          onclick={() => void run(true)}>{t('pricing.apply')}</Button
        >
        <Button
          size="sm"
          variant="ghost"
          onclick={() => ((pasting = false), (preview = null), (error = null))}
          >{t('pricing.cancel')}</Button
        >
      </div>
    {/if}
  {/if}
</section>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    margin-bottom: var(--alfa-space-4);
    padding: var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
  }
  h3 {
    font-size: var(--alfa-font-size-md);
  }
  .desc,
  .muted {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .table {
    max-height: 320px;
    overflow: auto;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--alfa-font-size-sm);
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
    background: var(--alfa-color-surface);
    font-weight: 600;
  }
  td:first-child {
    display: flex;
    flex-direction: column;
  }
  .model {
    font-family: var(--alfa-font-mono);
  }
  .changes {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    padding-left: var(--alfa-space-4);
    font-size: var(--alfa-font-size-sm);
  }
</style>
