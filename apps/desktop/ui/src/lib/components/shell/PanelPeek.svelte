<!--
  Panele chowane (właściciel 27.10: „mają się chować, wysuwać i mieć znaczniki, że tam są”).
  Przy krawędzi schowanego panelu świeci znacznik; krótkie najechanie wysuwa panel nad rozmowę,
  zjechanie myszą chowa go (chyba że ma fokus albo otwarte menu), tak samo kliknięcie obok i Esc.
  „Przypnij panel” dokuje go z powrotem (i wyłącza ustawienie „Panele chowają się same”).
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import Pin from '@lucide/svelte/icons/pin';
  import type { PanelPlacement, PanelSide } from '../../logic/layout';
  import { useApp } from '../../state/context';
  import EdgeTab from './EdgeTab.svelte';
  import RightPanel from './RightPanel.svelte';
  import SessionsPanel from './SessionsPanel.svelte';

  interface Props {
    left: PanelPlacement;
    right: PanelPlacement;
    rightWidth: number;
    /** Znaczniki tylko w widoku rozmowy poza trybem skupienia. */
    show: boolean;
  }

  let { left, right, rightWidth, show }: Props = $props();

  const app = useApp();
  const { t } = app.i18n;
  const sid = $derived(app.activeId);
  const side = $derived<PanelSide | null>(
    left === 'peek' ? 'left' : right === 'peek' ? 'right' : null,
  );

  $effect(() => {
    if (!show) untrack(() => (app.layout.peek = null));
  });

  let peekEl = $state<HTMLElement | null>(null);
  let showTimer: number | undefined;
  let hideTimer: number | undefined;
  const enter = (s: PanelSide) => {
    window.clearTimeout(showTimer);
    if (app.layout.peekable) showTimer = window.setTimeout(() => (app.layout.peek = s), 160);
  };
  const leaveTab = () => window.clearTimeout(showTimer);
  // Panel zostaje, gdy w nim piszesz albo gdy otwarte jest menu (np. menu sesji).
  // Ukryta lista palety (Ctrl+K) stale jest w drzewie — liczą się tylko widoczne menu.
  const busy = () =>
    peekEl?.querySelector('input:focus, textarea:focus') ||
    [...document.querySelectorAll('[role="menu"], [role="listbox"]')].some((el) =>
      el.checkVisibility(),
    );
  const cancelHide = () => {
    window.clearTimeout(hideTimer);
    hideTimer = undefined;
  };
  const scheduleHide = () => {
    if (hideTimer !== undefined) return;
    hideTimer = window.setTimeout(() => {
      hideTimer = undefined;
      if (!busy()) app.layout.peek = null;
    }, 450);
  };
  const pin = (s: PanelSide) => {
    if (app.layout.autohide) void app.setSetting('ui.panels_autohide', false);
    app.layout.peek = null;
    app.layout.update(sid, s === 'left' ? { left_open: true } : { right_open: true });
  };
  // Panel wysuwa się pod kursorem, więc „mouseleave” nie zawsze przychodzi — liczy się ruch
  // wskaźnika: poza panelem przez 450 ms chowa go, powrót anuluje.
  $effect(() => {
    if (!app.layout.peek) return;
    const move = (e: PointerEvent) => {
      const target = e.target as Element | null;
      if (peekEl?.contains(target) || target?.closest('[role="menu"], [role="listbox"]'))
        cancelHide();
      else scheduleHide();
    };
    const down = (e: PointerEvent) => {
      const target = e.target as Element | null;
      if (peekEl?.contains(target)) return;
      if (target?.closest('.edge-tab, .titlebar, [role="menu"], [role="listbox"], [role="dialog"]'))
        return;
      app.layout.peek = null;
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

{#if show && left === 'hidden'}
  <EdgeTab
    side="left"
    label={t('edge.sessions')}
    onenter={() => enter('left')}
    onleave={leaveTab}
    onclick={() => app.layout.toggleLeft(sid)}
  />
{/if}
{#if show && right === 'hidden'}
  <EdgeTab
    side="right"
    label={t('edge.panel')}
    onenter={() => enter('right')}
    onleave={leaveTab}
    onclick={() => app.layout.toggleRight(sid)}
  />
{/if}
{#if show && side}
  <div
    bind:this={peekEl}
    class="peek from-{side}"
    role="complementary"
    data-layer
    aria-label={side === 'left' ? t('sessions.title') : t('panel.label')}
    style:width="{side === 'left' ? app.layout.leftWidth : rightWidth}px"
  >
    <div class="body">
      {#if side === 'left'}<SessionsPanel floating />{:else}<RightPanel />{/if}
    </div>
    <button type="button" class="pin" onclick={() => pin(side)}>
      <Pin size={14} strokeWidth={1.5} aria-hidden="true" />
      {t('edge.pin')}
    </button>
  </div>
{/if}

<style>
  .peek {
    position: absolute;
    top: 0;
    bottom: 0;
    z-index: 30;
    display: flex;
    flex-direction: column;
    max-width: calc(100% - 48px);
    /* Kryjące tło warstwy nad treścią (w stylu „glass” powierzchnia jest półprzezroczysta). */
    background: var(--alfa-color-overlay, var(--alfa-color-surface));
    box-shadow: var(--alfa-shadow-3);
  }
  .from-left {
    left: 0;
    border-right: 1px solid var(--alfa-color-border);
    animation: slide-left var(--alfa-duration-panel) var(--alfa-ease-out);
  }
  .from-right {
    right: 0;
    border-left: 1px solid var(--alfa-color-border);
    animation: slide-right var(--alfa-duration-panel) var(--alfa-ease-out);
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
  @keyframes slide-left {
    from {
      transform: translateX(-24px);
      opacity: 0;
    }
  }
  @keyframes slide-right {
    from {
      transform: translateX(24px);
      opacity: 0;
    }
  }
</style>
