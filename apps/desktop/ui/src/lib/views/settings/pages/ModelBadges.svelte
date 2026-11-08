<!-- Krótkie oznaczenia modelu: przeznaczenie, wielkość, migawka, okno kontekstu. -->
<script lang="ts">
  import type { ModelInfo } from '../../../api/types-hub';
  import { useApp } from '../../../state/context';

  let { model }: { model: ModelInfo } = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  /** Wielkość ma sens tylko dla modeli tekstowych; „główny” to brak wyróżnika, więc go pomijamy. */
  const sized = $derived(
    (model.purpose === 'chat' || model.purpose === 'reasoning') && model.tier !== 'standard',
  );
</script>

<span class="badges">
  <span class="badge {model.purpose}">{tk(`api.purpose.${model.purpose}`)}</span>
  {#if sized}<span class="badge">{tk(`api.tier.${model.tier}`)}</span>{/if}
  {#if model.snapshot}<span class="badge">{t('api.snapshot')}</span>{/if}
  {#if model.context_tokens}
    <span class="badge">{t('api.context', { n: app.i18n.int(model.context_tokens) })}</span>
  {/if}
</span>

<style>
  .badges {
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-1);
  }
  .badge {
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-full);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    line-height: 1.6;
    white-space: nowrap;
  }
  .badge.chat,
  .badge.reasoning {
    border-color: var(--alfa-color-border-strong);
    color: var(--alfa-color-text);
  }
</style>
