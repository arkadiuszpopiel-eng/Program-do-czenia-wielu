<!-- Szczegóły kosztów i kontekstu (klik w stan na pasku tytułu). -->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  const costs = $derived(app.costs);
  const contextRatio = $derived(
    costs ? costs.context.used_tokens / Math.max(1, costs.context.max_tokens) : 0,
  );
  const limitRatio = $derived(
    costs && costs.limit.enabled ? costs.month.minor / Math.max(1, costs.limit.monthly.minor) : 0,
  );
</script>

{#if costs}
  <div class="details">
    <h3 class="h">{t('costs.title')}</h3>
    <dl class="grid">
      <dt>{t('costs.session')}</dt>
      <dd>{app.i18n.money(costs.session)}</dd>
      <dt>{t('costs.day')}</dt>
      <dd>{app.i18n.money(costs.day)}</dd>
      <dt>{t('costs.month')}</dt>
      <dd>{app.i18n.money(costs.month)}</dd>
    </dl>
    <div class="meter-block">
      <span class="label">{t('costs.limit')}</span>
      {#if costs.limit.enabled}
        <div class="bar" aria-hidden="true">
          <span style:transform="scaleX({Math.min(1, limitRatio)})" class:warn={limitRatio >= 0.8}
          ></span>
        </div>
        <span class="muted"
          >{t('costs.limitUsage', {
            used: app.i18n.money(costs.month),
            limit: app.i18n.money(costs.limit.monthly),
          })}</span
        >
        {#if limitRatio >= 0.8}<span class="warn-text"
            >{t('costs.warning', { pct: app.i18n.percent(limitRatio) })}</span
          >{/if}
      {:else}
        <span class="muted">{t('costs.limitOff')}</span>
      {/if}
    </div>
    <div class="meter-block">
      <span class="label">{t('costs.context')}</span>
      <div class="bar" aria-hidden="true">
        <span style:transform="scaleX({Math.min(1, contextRatio)})"></span>
      </div>
      <span class="muted">
        {t('costs.contextUsage', {
          used: app.i18n.int(costs.context.used_tokens),
          max: app.i18n.int(costs.context.max_tokens),
        })}
        {#if costs.context.compacted}· {t('costs.compacted')}{/if}
      </span>
    </div>
    <p class="muted fx">
      {t('costs.fx', { date: costs.fx.date, rate: costs.fx.usd_pln })}
      {#if costs.fx.stale}· {t('costs.fxStale')}{/if}
    </p>
    <Button size="sm" variant="secondary" onclick={() => app.openSettings('costs')}
      >{t('costs.settings')}</Button
    >
  </div>
{/if}

<style>
  .details {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
  }
  .h {
    font-size: var(--alfa-font-size-md);
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: var(--alfa-space-1) var(--alfa-space-3);
    margin: 0;
  }
  dt {
    color: var(--alfa-color-text-muted);
  }
  dd {
    margin: 0;
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  .meter-block {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
  }
  .label {
    font-weight: var(--alfa-weight-semibold);
  }
  .bar {
    height: 6px;
    overflow: hidden;
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface3);
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--alfa-color-info);
    transform-origin: left;
  }
  .bar span.warn {
    background: var(--alfa-color-warning);
  }
  .muted {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .warn-text {
    color: var(--alfa-color-warning);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
  }
  .fx {
    margin: 0;
  }
</style>
