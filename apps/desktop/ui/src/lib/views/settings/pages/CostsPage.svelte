<!-- Koszty (PLAN §14.6): limit miesięczny w PLN z przełącznikiem całkowitego wyłączenia. -->
<script lang="ts">
  import { LevelMeter, Switch } from '@alfa/ui-kit';
  import { useApp } from '../../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  const costs = $derived(app.costs);
  const uid = $props.id();

  async function setLimit(enabled: boolean, zloty: number) {
    const minor = Math.max(0, Math.round(zloty * 100));
    await app.client.costs.setMonthlyLimit(enabled, { minor, currency: 'PLN' });
    await app.refreshCosts();
  }
</script>

{#if costs}
  <section class="card">
    <div class="row">
      <div>
        <h3 id="{uid}-l">{t('costsPage.limitToggle')}</h3>
        <p class="desc" id="{uid}-d">{t('costsPage.limitDesc')}</p>
      </div>
      <Switch
        checked={costs.limit.enabled}
        labelledby="{uid}-l"
        describedby="{uid}-d"
        onchange={(on) => void setLimit(on, costs.limit.monthly.minor / 100)}
      />
    </div>
    {#if costs.limit.enabled}
      <label class="amount">
        <span>{t('costsPage.amount')}</span>
        <input
          type="number"
          min="0"
          step="10"
          value={costs.limit.monthly.minor / 100}
          onchange={(e) => void setLimit(true, Number(e.currentTarget.value))}
        />
      </label>
      <div class="usage">
        <span
          >{t('costsPage.usage')}: {t('costs.limitUsage', {
            used: app.i18n.money(costs.month),
            limit: app.i18n.money(costs.limit.monthly),
          })}</span
        >
        <LevelMeter
          level={costs.month.minor / Math.max(1, costs.limit.monthly.minor)}
          label={t('costsPage.usage')}
          accent={costs.month.minor / Math.max(1, costs.limit.monthly.minor) >= 0.8
            ? 'var(--alfa-color-warning)'
            : 'var(--alfa-color-info)'}
        />
      </div>
    {:else}
      <p class="desc">
        {t('costs.limitOff')}
        {t('costsPage.usage')}: {app.i18n.money(costs.month)}
      </p>
    {/if}
    <p class="desc">
      {t('costs.fx', { date: costs.fx.date, rate: costs.fx.usd_pln })}
      {#if costs.fx.stale}· {t('costs.fxStale')}{/if}
    </p>
    <p class="desc">{t('costsPage.perProvider')}</p>
  </section>
{/if}

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
  .row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--alfa-space-4);
  }
  h3 {
    font-size: var(--alfa-font-size-md);
  }
  .desc {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .amount {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-3);
    font-size: var(--alfa-font-size-sm);
  }
  .amount input {
    width: 120px;
    height: var(--alfa-size-control);
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface);
    text-align: right;
  }
  .usage {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    font-size: var(--alfa-font-size-sm);
  }
</style>
