<!-- Stany systemowe (PLAN §14.4, makieta 18): offline + kolejka, 429, brak kluczy, mikrofon, dysk,
     przygotowana aktualizacja („Uruchom ponownie, aby zaktualizować"), stan Brokera (bezpieczny
     stan po zerwaniu, kill-switch bez watchdoga). -->
<script lang="ts">
  import { Banner, Button } from '@alfa/ui-kit';
  import { now } from '../../state/clock.svelte';
  import UpdateBanner from '../shell/UpdateBanner.svelte';
  import BrokerBanner from '../shell/BrokerBanner.svelte';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  const status = $derived(app.system);
  const dismiss = (id: string) => () => (app.dismissed[id] = true);
  const any = $derived(
    status !== null &&
      (!status.online ||
        (status.rate_limit !== null && !app.dismissed['rate']) ||
        (!status.keys_configured && !app.dismissed['keys']) ||
        (status.mic !== 'ok' && !app.dismissed['mic']) ||
        (status.disk.low && !app.dismissed['disk'])),
  );
</script>

{#if app.broker.banner}
  <div class="banners" role="region" aria-label={t('broker.region')}>
    <BrokerBanner />
  </div>
{/if}
{#if app.updates.showBanner}
  <div class="banners" role="region" aria-label={t('updates.title')}>
    <UpdateBanner />
  </div>
{/if}
{#if status && any}
  <div class="banners" role="region" aria-label={t('banner.region')}>
    {#if !status.online}
      <Banner kind="warning" icon="offline">
        {t('banner.offline')}
        {#if status.queued_messages > 0}<strong>
            {t('banner.queue', { n: status.queued_messages })}.</strong
          >{/if}
        {#snippet actions()}
          <Button size="sm" variant="secondary" onclick={() => void app.client.system.retryQueue()}>
            {t('banner.retry')}
          </Button>
        {/snippet}
      </Banner>
    {/if}
    {#if status.rate_limit && !app.dismissed['rate']}
      <Banner
        kind="warning"
        icon="clock"
        ondismiss={dismiss('rate')}
        dismissLabel={t('banner.dismiss')}
      >
        {t('banner.rateLimit', {
          provider: status.rate_limit.provider,
          time: app.i18n.time(status.rate_limit.resets_at),
          relative: app.i18n.relative(status.rate_limit.resets_at, now()),
        })}
      </Banner>
    {/if}
    {#if !status.keys_configured && !app.dismissed['keys']}
      <Banner kind="info" icon="key" ondismiss={dismiss('keys')} dismissLabel={t('banner.dismiss')}>
        {t('banner.noKeys')}
        {#snippet actions()}
          <Button size="sm" variant="secondary" onclick={() => app.addProviderKey()}>
            {t('banner.addKey')}
          </Button>
        {/snippet}
      </Banner>
    {/if}
    {#if status.mic !== 'ok' && !app.dismissed['mic']}
      <Banner
        kind="warning"
        icon="mic"
        ondismiss={dismiss('mic')}
        dismissLabel={t('banner.dismiss')}
      >
        {t(status.mic === 'denied' ? 'banner.micDenied' : 'banner.micMissing')}
        {#snippet actions()}
          {#if status.mic === 'denied'}
            <Button
              size="sm"
              variant="secondary"
              onclick={() =>
                void app.client.app.openSystemSettings('ms-settings:privacy-microphone')}
            >
              {t('banner.micSettings')}
            </Button>
          {/if}
        {/snippet}
      </Banner>
    {/if}
    {#if status.disk.low && !app.dismissed['disk']}
      <Banner
        kind="warning"
        icon="disk"
        ondismiss={dismiss('disk')}
        dismissLabel={t('banner.dismiss')}
      >
        {t('banner.disk', { free: app.i18n.bytes(status.disk.free_bytes) })}
        {#snippet actions()}
          <Button
            size="sm"
            variant="secondary"
            onclick={() => void app.client.app.openSystemSettings('ms-settings:storagesense')}
          >
            {t('banner.diskSettings')}
          </Button>
        {/snippet}
      </Banner>
    {/if}
  </div>
{/if}

<style>
  .banners {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
  }
</style>
