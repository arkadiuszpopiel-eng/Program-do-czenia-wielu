<!--
  Panel wysunięty nad treść zza krawędzi okna (Sesje, panel prawy, menu Ustawień — 29.10: wszystkie
  panele „w pełni chowane i wysuwane”). Wjeżdża i wyjeżdża płynnie; zjechanie myszą poza panel na
  450 ms chowa go (chyba że w nim piszesz albo otwarte jest menu), tak samo kliknięcie obok.
  „Przypnij panel” na dole dokuje go z powrotem.
-->
<script lang="ts">
  import { untrack, type Snippet } from 'svelte';
  import { cubicOut } from 'svelte/easing';
  import { fly } from 'svelte/transition';
  import Pin from '@lucide/svelte/icons/pin';
  import type { PanelSide } from '../../logic/layout';
  import { panelMs } from '../../logic/motion';
  import { useApp } from '../../state/context';

  interface Props {
    side: PanelSide;
    width: number;
    label: string;
    /** Otwarty klawiaturą (ze znacznika) — fokus wchodzi do panelu. */
    focusOnOpen?: boolean;
    onclose: () => void;
    onpin: () => void;
    children: Snippet;
  }

  let { side, width, label, focusOnOpen = false, onclose, onpin, children }: Props = $props();

  const app = useApp();
  const { t } = app.i18n;
  let el = $state<HTMLElement | null>(null);
  let hideTimer: number | undefined;
  const motion = () => ({
    x: side === 'left' ? -width : width,
    opacity: 1,
    duration: panelMs(),
    easing: cubicOut,
  });

  $effect(() => {
    if (el && untrack(() => focusOnOpen))
      el.querySelector<HTMLElement>('button, [href], input, [tabindex="0"]')?.focus();
  });

  // Panel zostaje, gdy w nim piszesz albo gdy otwarte jest menu (np. menu sesji).
  // Ukryta lista palety (Ctrl+K) stale jest w drzewie — liczą się tylko widoczne menu.
  const busy = () =>
    el?.querySelector('input:focus, textarea:focus, :focus-visible') ||
    [...document.querySelectorAll('[role="menu"], [role="listbox"]')].some((m) =>
      m.checkVisibility(),
    );
  const cancelHide = () => {
    window.clearTimeout(hideTimer);
    hideTimer = undefined;
  };
  const scheduleHide = () => {
    if (hideTimer !== undefined) return;
    hideTimer = window.setTimeout(() => {
      hideTimer = undefined;
      if (!busy()) onclose();
    }, 450);
  };
  // Panel wysuwa się pod kursorem, więc „mouseleave” nie zawsze przychodzi — liczy się ruch
  // wskaźnika: poza panelem przez 450 ms chowa go, powrót anuluje.
  $effect(() => {
    const move = (e: PointerEvent) => {
      const target = e.target as Element | null;
      if (el?.contains(target) || target?.closest('[role="menu"], [role="listbox"], .edge-tab'))
        cancelHide();
      else scheduleHide();
    };
    const down = (e: PointerEvent) => {
      const target = e.target as Element | null;
      if (el?.contains(target)) return;
      if (target?.closest('.edge-tab, .titlebar, [role="menu"], [role="listbox"], [role="dialog"]'))
        return;
      onclose();
    };
    window.addEventListener('pointermove', move, true);
    window.addEventListener('pointerdown', down, true);
    return () => {
      window.removeEventListener('pointermove', move, true);
      window.removeEventListener('pointerdown', down, true);
      cancelHide();
    };
  });
</script>

<div
  bind:this={el}
  class="peek from-{side}"
  role="complementary"
  data-layer
  aria-label={label}
  style:width="{width}px"
  transition:fly|global={motion()}
>
  <div class="body">{@render children()}</div>
  <button type="button" class="pin" onclick={onpin}>
    <Pin size={14} strokeWidth={1.5} aria-hidden="true" />
    {t('edge.pin')}
  </button>
</div>

<style>
  .peek {
    position: absolute;
    top: 0;
    bottom: 0;
    z-index: 30;
    display: flex;
    flex-direction: column;
    max-width: calc(100% - 48px);
    overflow: hidden;
    /* Kryjące tło warstwy nad treścią (w stylu „glass” powierzchnia jest półprzezroczysta). */
    background: var(--alfa-color-overlay, var(--alfa-color-surface));
    box-shadow: var(--alfa-shadow-3);
  }
  .from-left {
    left: 0;
    border-right: 1px solid var(--alfa-color-border);
    border-radius: 0 var(--alfa-radius-card) var(--alfa-radius-card) 0;
  }
  .from-right {
    right: 0;
    border-left: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card) 0 0 var(--alfa-radius-card);
  }
  .body {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
  }
  .pin {
    display: flex;
    flex: none;
    align-items: center;
    justify-content: center;
    gap: var(--alfa-space-2);
    min-height: var(--alfa-size-control);
    margin: var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text-muted);
    font: inherit;
    font-size: var(--alfa-font-size-sm);
    cursor: pointer;
  }
  .pin:hover {
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text);
  }
  .pin:focus-visible {
    outline: 2px solid var(--alfa-color-focus);
    outline-offset: 2px;
  }
</style>
