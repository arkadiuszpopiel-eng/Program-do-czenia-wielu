<!--
  Import kluczy API ze zmiennych środowiskowych (plan v6, C2.17): rdzeń sprawdza wyłącznie nazwy
  zmiennych z katalogu dostawców (np. `OPENAI_API_KEY`), a znaleziony klucz zapisuje w Windows
  Credential Manager. Interfejs dostaje tylko nazwy zmiennych i wynik — nigdy wartości.
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import Check from '@lucide/svelte/icons/check';
  import Minus from '@lucide/svelte/icons/minus';
  import X from '@lucide/svelte/icons/x';
  import type { EnvImportResult } from '../../../api/types-hub';
  import { useApp } from '../../../state/context';

  interface Props {
    results: readonly EnvImportResult[];
    onclose: () => void;
  }
  let { results, onclose }: Props = $props();

  const app = useApp();
  const { t } = app.i18n;
  const found = $derived(results.filter((r) => r.result !== 'not_set'));
  const missing = $derived(results.filter((r) => r.result === 'not_set'));
  const imported = $derived(found.filter((r) => r.result === 'imported').length);
</script>

<section class="env" aria-labelledby="env-title" role="status">
  <div class="head">
    <h4 id="env-title">
      {imported > 0 ? t('env.done', { n: imported }) : t('env.nothing')}
    </h4>
    <Button size="sm" variant="ghost" onclick={onclose}>
      {#snippet icon()}<X size={14} strokeWidth={1.5} />{/snippet}
      {t('common.close')}
    </Button>
  </div>
  {#if found.length > 0}
    <ul class="found">
      {#each found as item (item.var)}
        <li>
          <span class="mark" class:ok={item.result === 'imported'} aria-hidden="true">
            {#if item.result === 'imported'}<Check size={14} strokeWidth={2} />{:else}<Minus
                size={14}
                strokeWidth={2}
              />{/if}
          </span>
          <span class="who">{item.provider_name}</span>
          <code>{item.var}</code>
          <span class="what">
            {item.result === 'imported' ? t('env.imported') : t('env.already')}
          </span>
        </li>
      {/each}
    </ul>
  {/if}
  {#if missing.length > 0}
    <p class="missing">
      {t('env.missing', { n: missing.length })}
      <span class="vars">{missing.map((m) => m.var).join(', ')}</span>
    </p>
  {/if}
  <p class="note">{t('env.note')}</p>
</section>

<style>
  .env {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-bg);
    font-size: var(--alfa-font-size-sm);
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-2);
  }
  h4 {
    margin: 0;
    font-size: var(--alfa-font-size-md);
  }
  .found {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .found li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-2);
  }
  .mark {
    display: inline-flex;
    color: var(--alfa-color-text-muted);
  }
  .mark.ok {
    color: var(--alfa-color-success);
  }
  .who {
    font-weight: var(--alfa-weight-semibold);
  }
  code {
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-xs);
  }
  .what,
  .missing,
  .note {
    margin: 0;
    color: var(--alfa-color-text-muted);
  }
  .vars {
    display: block;
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-xs);
    overflow-wrap: anywhere;
  }
  .note {
    font-size: var(--alfa-font-size-xs);
  }
</style>
