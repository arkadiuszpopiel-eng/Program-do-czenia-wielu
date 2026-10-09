<!--
  Objaśnienie stanu z paska tytułu: połączenie (i kolejka wiadomości bez sieci), profil modeli,
  poziom autonomii — po ludzku, z przejściem do ustawień, w których się to zmienia.
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import { useApp } from '../../state/context';

  interface Props {
    /** Wywoływane przed przejściem do Ustawień (zamyka okienko). */
    onnavigate: () => void;
  }
  let { onnavigate }: Props = $props();

  const app = useApp();
  const { t, tk } = app.i18n;
  const session = $derived(app.sessions.active);
  const profileId = $derived(app.system?.profile ?? session?.profile ?? 'hybrid');
  const level = $derived(session?.autonomy ?? 'L3');
  const offline = $derived(app.system?.online === false);
  const queued = $derived(app.system?.queued_messages ?? 0);

  function go(page: string) {
    onnavigate();
    app.openSettings(page);
  }
</script>

<div class="details">
  <h3 class="h">{t('status.title')}</h3>
  <dl>
    <dt>{t('status.connection')}</dt>
    <dd>
      <strong class:warn={offline}>{t(offline ? 'status.offline' : 'status.online')}</strong>
      <span class="desc">
        {offline ? t('status.offlineDesc', { n: queued }) : t('status.onlineDesc')}
      </span>
    </dd>
    <dt>{t('status.profile')}</dt>
    <dd>
      <strong>{tk(`profile.${profileId}`)}</strong>
      <span class="desc">{tk(`status.profileDesc.${profileId}`)}</span>
      {#if app.system && !app.system.keys_configured && profileId !== 'local'}
        <span class="desc warn">{t('status.noKeys')}</span>
      {/if}
    </dd>
    <dt>{t('status.autonomy')}</dt>
    <dd>
      <strong>{level} — {tk(`perm.${level}.name`)}</strong>
      <span class="desc">{tk(`perm.${level}.desc`)}</span>
    </dd>
  </dl>
  <div class="actions">
    <Button size="sm" variant="secondary" onclick={() => go('permissions')}
      >{t('status.openPermissions')}</Button
    >
    <Button size="sm" variant="secondary" onclick={() => go('providers')}
      >{t('status.openAccounts')}</Button
    >
  </div>
</div>

<style>
  .details {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    padding: var(--alfa-space-3);
    font-size: var(--alfa-font-size-sm);
    white-space: normal;
  }
  .h {
    margin: 0;
    font-size: var(--alfa-font-size-md);
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--alfa-space-2) var(--alfa-space-3);
    margin: 0;
  }
  dt {
    color: var(--alfa-color-text-muted);
  }
  dd {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
  }
  .desc {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    line-height: 1.4;
  }
  .warn {
    color: var(--alfa-color-warning);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
  }
</style>
