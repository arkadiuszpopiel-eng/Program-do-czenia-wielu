<!--
  Baner Brokera (ADR 0003, PLAN §8.2, §8.6): bezpieczny stan (połączenie zerwane, brak Brokera —
  wszystko, co wymaga zgody, jest odrzucane; nie do ukrycia), łączenie, kill-switch awaryjnie
  w aplikacji (watchdog nie działa), tryb deweloperski. Niczego tu się nie zatwierdza.
-->
<script lang="ts">
  import { Banner, Button } from '@alfa/ui-kit';
  import { bannerIsSticky } from '../../logic/broker';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  const banner = $derived(app.broker.banner);
  const detail = $derived(app.broker.view?.detail ?? null);
</script>

{#if banner}
  {@const sticky = bannerIsSticky(banner)}
  <Banner
    kind={sticky ? 'error' : banner === 'dev' ? 'info' : 'warning'}
    icon={sticky || banner === 'watchdog' ? 'warning' : 'info'}
    ondismiss={sticky || banner === 'connecting' ? undefined : () => app.broker.dismiss(banner)}
    dismissLabel={t('banner.dismiss')}
  >
    {t(`broker.banner.${banner}`)}
    {#if sticky && detail}
      <span class="detail">{app.i18n.text(detail)}</span>
    {/if}
    {#snippet actions()}
      <Button size="sm" variant="secondary" onclick={() => app.openSettings('permissions')}>
        {t('broker.banner.details')}
      </Button>
    {/snippet}
  </Banner>
{/if}

<style>
  .detail {
    display: block;
    margin-top: var(--alfa-space-1);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
</style>
