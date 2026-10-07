<!--
  Baner „Uruchom ponownie, aby zaktualizować" (PLAN §14.4, §14.8): nowa (albo przywrócona) wersja
  jest przygotowana i zacznie działać po ponownym uruchomieniu przez launcher. Restart czeka, gdy
  agentka wykonuje zadanie albo trwa rozmowa głosowa (powód z rdzenia).
-->
<script lang="ts">
  import { Banner, Button } from '@alfa/ui-kit';
  import { isRollback } from '../../logic/updates';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  const view = $derived(app.updates.view);
  let busy = $state(false);

  async function restart() {
    busy = true;
    try {
      await app.client.updates.restart();
      await app.updates.load(app.client.updates);
    } catch (error) {
      app.toasts.show({
        kind: 'error',
        message: error instanceof Error ? error.message : String(error),
      });
    } finally {
      busy = false;
    }
  }
</script>

{#if view && app.updates.showBanner && view.ready}
  {@const rollback = isRollback(view)}
  <Banner
    kind="info"
    icon="info"
    ondismiss={() => app.updates.hideBanner()}
    dismissLabel={t('updates.bannerLater')}
  >
    {rollback
      ? t('updates.bannerRollback', { version: view.ready })
      : t('updates.banner', { version: view.ready })}
    {#if view.restart_blocked}
      <span class="why"
        >{t('updates.restartBlocked', { why: app.i18n.text(view.restart_blocked) })}</span
      >
    {/if}
    {#snippet actions()}
      <Button size="sm" variant="secondary" onclick={() => app.openSettings('updates')}>
        {t('updates.bannerOpen')}
      </Button>
      <Button
        size="sm"
        variant="primary"
        disabled={busy || view.restart_blocked !== null}
        onclick={restart}
      >
        {rollback ? t('updates.restartRollback') : t('updates.restart')}
      </Button>
    {/snippet}
  </Banner>
{/if}

<style>
  .why {
    display: block;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
</style>
