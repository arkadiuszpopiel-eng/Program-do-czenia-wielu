<!--
  Wskaźnik w pasku tytułu: „Delta steruje ekranem" (computer use) z przyciskiem „Zatrzymaj"
  (przejęcie: anuluje akcje GUI i tury) albo „Sterujesz Ty — oddaj sterowanie". Klik w etykietę
  otwiera panel Ekran. Widoczny tylko, gdy agentka steruje albo właściciel przejął sterowanie.
-->
<script lang="ts">
  import MousePointer from '@lucide/svelte/icons/mouse-pointer-2';
  import { agentName } from '../../logic/work';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  const gui = $derived(app.work.gui);

  async function act(action: () => Promise<unknown>) {
    try {
      await action();
    } catch (error) {
      app.toasts.show({
        kind: 'error',
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }
</script>

{#if gui?.taken_over}
  <div class="gui taken" role="status">
    <button type="button" class="label" onclick={() => app.openPanel('screen')}
      >{t('gui.takenOver')}</button
    >
    <button type="button" class="act" onclick={() => act(() => app.client.gui.release())}
      >{t('gui.release')}</button
    >
  </div>
{:else if gui?.control}
  {@const name = agentName(gui.control.agent)}
  <div class="gui live" role="status">
    <button
      type="button"
      class="label"
      aria-label={t('gui.controllingOpen', { name })}
      onclick={() => app.openPanel('screen')}
    >
      <MousePointer size={14} strokeWidth={1.5} aria-hidden="true" />
      {t('gui.controlling', { name })}
    </button>
    <button type="button" class="act stop" onclick={() => act(() => app.client.gui.stop())}
      >{t('gui.stopShort')}</button
    >
  </div>
{/if}

<style>
  .gui {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 26px;
    padding: 0 2px 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-full);
    font-size: var(--alfa-font-size-xs);
  }
  .live {
    border-color: var(--alfa-color-warning);
  }
  .label,
  .act {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 22px;
    padding: 0 var(--alfa-space-2);
    border: 0;
    border-radius: var(--alfa-radius-full);
    background: transparent;
    color: var(--alfa-color-text);
    font: inherit;
    cursor: pointer;
  }
  .act {
    border: 1px solid var(--alfa-color-border-strong);
  }
  .stop {
    border-color: var(--alfa-color-error);
    color: var(--alfa-color-error);
  }
  .label:focus-visible,
  .act:focus-visible {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
  }
</style>
