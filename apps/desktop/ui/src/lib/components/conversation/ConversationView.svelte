<!-- Kolumna rozmowy: stany systemowe, wyszukiwanie, wiadomości (lub pusty stan), kapsuła aktywności, composer. -->
<script lang="ts">
  import { ActivityCapsule, agents } from '@alfa/ui-kit';
  import { useApp } from '../../state/context';
  import ChatComposer from './ChatComposer.svelte';
  import FindBar from './FindBar.svelte';
  import MessageList from './MessageList.svelte';
  import StartState from './StartState.svelte';
  import SystemBanners from './SystemBanners.svelte';

  const app = useApp();
  const { t } = app.i18n;
  const conv = $derived(app.conversation);
  const activity = $derived(app.activeId ? app.activity[app.activeId] : null);
  let focusTurn = $state<string | null>(null);
  let tick = $state(Date.now());

  // Licznik czasu kapsuły tyka tylko, gdy kapsuła jest widoczna (brak pracy w bezczynności).
  $effect(() => {
    if (!activity) return;
    const handle = setInterval(() => (tick = Date.now()), 1000);
    return () => clearInterval(handle);
  });
  const elapsed = $derived(
    activity ? Math.max(0, Math.round((tick - new Date(activity.started_at).getTime()) / 1000)) : 0,
  );
</script>

<section class="conv" aria-label={t('conv.label')}>
  <div class="top">
    <SystemBanners />
    {#if app.findOpen && conv}<FindBar path={conv.path} bind:focusTurn />{/if}
  </div>
  {#if conv && conv.loaded && conv.path.length > 0}
    {#key conv.sessionId}
      <MessageList {conv} focusTurn={app.findOpen ? focusTurn : null} />
    {/key}
  {:else if !conv || conv.loaded}
    <div class="empty"><StartState /></div>
  {:else}
    <div class="empty" aria-busy="true"></div>
  {/if}
  <div class="bottom">
    {#if activity}
      <div class="capsule">
        <ActivityCapsule
          agent={activity.agent}
          description={activity.description}
          step={activity.step}
          totalSteps={activity.total_steps}
          elapsedSeconds={elapsed}
          onstop={() => void app.stopGeneration()}
          labels={{
            step: t('activity.step', { step: activity.step, total: activity.total_steps }),
            stop: t('activity.stop'),
            stopLabel: t('activity.stopLabel', { name: agents[activity.agent].name }),
            progress: t('activity.progress'),
          }}
        />
      </div>
    {/if}
    <ChatComposer {conv} />
  </div>
</section>

<style>
  .conv {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    height: 100%;
    background: var(--alfa-color-bg);
  }
  .top,
  .bottom {
    width: min(var(--conv-column, var(--alfa-size-reading-column)), 100%);
    margin: 0 auto;
    padding: 0 var(--alfa-space-4);
  }
  .top {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding-top: var(--alfa-space-3);
  }
  .top:empty {
    display: none;
  }
  .bottom {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding-bottom: var(--alfa-space-3);
  }
  .capsule {
    display: flex;
    justify-content: center;
  }
  .empty {
    flex: 1;
    min-height: 0;
    display: flex;
    overflow: auto;
  }
</style>
