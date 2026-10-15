<!--
  Przyciski okna (minimalizuj, maksymalizuj / przywróć, zamknij) rysowane przez interfejs — okno
  główne nie ma belki systemowej. Układ i rozmiar jak w Windows 11: trzy pola 44 × wysokość paska,
  przy prawej krawędzi, zamknięcie podświetlane na czerwono.
-->
<script lang="ts">
  import Minus from '@lucide/svelte/icons/minus';
  import Square from '@lucide/svelte/icons/square';
  import Copy from '@lucide/svelte/icons/copy';
  import X from '@lucide/svelte/icons/x';
  import { windowControls, type WindowControls } from '../../api/window';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  let controls = $state<WindowControls | null>(null);
  let maximized = $state(false);

  $effect(() => {
    let off: (() => void) | undefined;
    let gone = false;
    void windowControls().then(async (found) => {
      if (gone) return;
      controls = found;
      const refresh = () => void found.isMaximized().then((value) => (maximized = value));
      refresh();
      off = await found.onResized(refresh);
      if (gone) off();
    });
    return () => {
      gone = true;
      off?.();
    };
  });
</script>

<div class="window-controls" role="group" aria-label={t('window.controls')}>
  <button
    type="button"
    class="wc"
    data-tip={t('window.minimize')}
    aria-label={t('window.minimize')}
    onclick={() => void controls?.minimize()}
  >
    <Minus size={16} strokeWidth={1.5} aria-hidden="true" />
  </button>
  <button
    type="button"
    class="wc"
    data-tip={maximized ? t('window.restore') : t('window.maximize')}
    aria-label={maximized ? t('window.restore') : t('window.maximize')}
    onclick={() => void controls?.toggleMaximize()}
  >
    {#if maximized}
      <Copy size={13} strokeWidth={1.5} aria-hidden="true" />
    {:else}
      <Square size={13} strokeWidth={1.5} aria-hidden="true" />
    {/if}
  </button>
  <button
    type="button"
    class="wc close"
    data-tip={t('window.close')}
    aria-label={t('window.close')}
    onclick={() => void controls?.close()}
  >
    <X size={16} strokeWidth={1.5} aria-hidden="true" />
  </button>
</div>

<style>
  .window-controls {
    display: flex;
    align-self: stretch;
    margin-left: var(--alfa-space-2);
    margin-right: calc(-1 * var(--alfa-space-2));
  }
  .wc {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 44px;
    border: 0;
    border-radius: 0;
    background: transparent;
    color: var(--alfa-color-text);
  }
  .wc:hover {
    background: var(--alfa-color-surface2);
  }
  .wc:active {
    background: var(--alfa-color-border);
  }
  .wc:focus-visible {
    outline: 2px solid var(--alfa-color-focus);
    outline-offset: -2px;
  }
  .wc.close:hover,
  .wc.close:active {
    background: #c42b1c;
    color: #ffffff;
  }
  /* Wąskie okno: węższe pola, żeby pasek mieścił się w 400 px. */
  @media (max-width: 520px) {
    .wc {
      width: 32px;
    }
    .window-controls {
      margin-left: 0;
    }
  }
  /* Bardzo wąskie okno (ekrany od 360 px, E.1). */
  @media (max-width: 380px) {
    .wc {
      width: 28px;
    }
  }
  @media (forced-colors: active) {
    .wc.close:hover {
      background: Highlight;
      color: HighlightText;
    }
  }
</style>
