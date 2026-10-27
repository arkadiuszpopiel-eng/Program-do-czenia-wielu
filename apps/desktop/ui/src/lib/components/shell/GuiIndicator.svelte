<!--
  Wskaźnik w pasku tytułu: „Delta steruje ekranem" (computer use) z przyciskiem „Zatrzymaj"
  (przejęcie: anuluje akcje GUI i tury) albo „Sterujesz Ty — oddaj sterowanie". Klik w etykietę
  otwiera panel Ekran. Widoczny tylko, gdy agentka steruje albo właściciel przejął sterowanie.
-->
<script lang="ts">
  import MousePointer from '@lucide/svelte/icons/mouse-pointer-2';
  import Square from '@lucide/svelte/icons/square';
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
      ><span class="text">{t('gui.takenOver')}</span></button
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
      <span class="text">{t('gui.controlling', { name })}</span>
    </button>
    <button
      type="button"
      class="act stop"
      aria-label={t('gui.stopShort')}
      title={t('gui.stopShort')}
      onclick={() => act(() => app.client.gui.stop())}
      ><Square size={12} strokeWidth={2} aria-hidden="true" class="stop-icon" /><span
        class="stop-text">{t('gui.stopShort')}</span
      ></button
    >
  </div>
{/if}

<style>
  /* Wąskie okno: kapsuła się zwęża (tekst z wielokropkiem), nigdy nie łamie się na kilka linii
     w pasku tytułu; poniżej 720 px zostaje ikona (etykieta dla czytników w aria-label). */
  .gui {
    display: flex;
    flex: 0 1 auto;
    align-items: center;
    min-width: 0;
    gap: 2px;
    height: 34px;
    padding: 0 3px 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    background: var(--alfa-color-bg);
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
    height: 26px;
    padding: 0 var(--alfa-space-2);
    border: 0;
    border-radius: var(--alfa-radius-full);
    background: transparent;
    color: var(--alfa-color-text);
    font: inherit;
    cursor: pointer;
  }
  .label {
    min-width: 0;
  }
  .text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .act {
    flex: none;
    border: 1px solid var(--alfa-color-border-strong);
    white-space: nowrap;
  }
  @media (max-width: 720px) {
    .gui {
      flex: none;
    }
    .live .text {
      display: none;
    }
  }
  .live :global(.stop-icon) {
    display: none;
  }
  /* Poniżej 900 px pasek tytułu nie mieści pełnej kapsuły: ściśnięta wsuwała „Zatrzymaj” pod
     przycisk mikrofonu (cel kliknięcia przykryty). Zostaje kwadrat „Zatrzymaj” (nazwa przycisku
     ukryta tylko wizualnie). */
  @media (max-width: 900px) {
    .live {
      flex: none;
      padding-left: 3px;
    }
    .live .label {
      display: none;
    }
    .live :global(.stop-icon) {
      display: block;
    }
    .stop-text {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }
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
