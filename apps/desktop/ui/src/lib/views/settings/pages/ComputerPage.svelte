<!--
  Ustawienia → Komputer: dostępność sterowania (computer use), strażnik okien (agentki nigdy nie
  sterują Alfą, Brokerem ani watchdogiem; okna Alfy są niewidoczne na zrzutach), przejęcie /
  oddanie sterowania, „zawsze zezwalaj na podgląd pulpitu" (okno Brokera, ≤ 24 h) i wbudowany
  terminal (powłoka — tylko z gestu użytkownika).
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import DesktopGrant from '../../../components/panels/DesktopGrant.svelte';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import { agentName } from '../../../logic/work';
  import { attempt, load } from '../../../state/attempt';
  import { useApp } from '../../../state/context';
  import './work.css';

  const app = useApp();
  const { t } = app.i18n;
  const gui = $derived(app.work.gui);
  let loadError = $state<string | null>(null);

  async function reload() {
    const result = await load(() => app.client.gui.status());
    if (result.status === 'ready') {
      app.work.applyGui(result.value);
      loadError = null;
    } else if (result.status === 'failed') loadError = result.error;
  }

  $effect(() => {
    void reload();
  });

  async function act(action: () => Promise<unknown>) {
    await attempt(app.toasts, action);
  }

  function openScreen() {
    app.closeSettings();
    app.openPanel('screen');
  }
</script>

<section class="wk-card" aria-labelledby="pc-title">
  <h3 id="pc-title">{t('pc.title')}</h3>
  <p>{t('pc.intro')}</p>
  {#if loadError}<LoadFailed error={loadError} onretry={() => void reload()} />{/if}
  {#if gui && !gui.available}
    <p class="wk-note" role="status">
      {gui.reason ? app.i18n.text(gui.reason) : t('gui.unavailable')}
    </p>
  {/if}
  <ul class="wk-plain">
    <li>{t('pc.guard')}</li>
    <li>{t('pc.protect')}</li>
    <li>{t('pc.tools')}</li>
  </ul>
  {#if gui}
    <p role="status" class:wk-warn={gui.control && !gui.taken_over}>
      {gui.taken_over
        ? t('gui.takenOverLong')
        : gui.control
          ? t('gui.controlling', { name: agentName(gui.control.agent) })
          : t('gui.idle')}
    </p>
  {/if}
  <div class="wk-actions">
    {#if gui?.taken_over}
      <Button size="sm" variant="primary" onclick={() => act(() => app.client.gui.release())}
        >{t('gui.release')}</Button
      >
    {:else}
      <Button size="sm" variant="danger" onclick={() => act(() => app.client.gui.stop())}
        >{t('gui.stop')}</Button
      >
    {/if}
    <Button size="sm" variant="ghost" onclick={openScreen}>{t('pc.openScreen')}</Button>
  </div>
</section>

<section class="wk-card" aria-labelledby="pc-grant">
  <h3 id="pc-grant">{t('gui.grantTitle')}</h3>
  {#if app.activeId}
    <p class="wk-meta">
      {t('pc.grantSession', { title: app.sessions.active?.title ?? app.activeId })}
    </p>
    <DesktopGrant sessionId={app.activeId} />
  {:else}
    <p class="wk-meta">{t('pc.grantNoSession')}</p>
  {/if}
</section>

<section class="wk-card" aria-labelledby="pc-term">
  <h3 id="pc-term">{t('pc.terminal')}</h3>
  <p class="wk-meta">{t('term.privacy')}</p>
  <div class="wk-actions">
    <Button size="sm" variant="secondary" onclick={() => app.work.openTerminal('shell')}
      >{t('term.open.shell')}</Button
    >
    <Button size="sm" variant="ghost" onclick={() => app.work.openTerminal('cmd')}
      >{t('term.open.cmd')}</Button
    >
  </div>
</section>
