<!--
  Panele chowane (właściciel 27.10: „mają się chować, wysuwać i mieć znaczniki, że tam są”; 29.10:
  „w pełni chowane i wysuwane”). Przy krawędzi schowanego panelu świeci znacznik; krótkie najechanie
  wysuwa panel nad rozmowę (`PeekLayer`), kliknięcie dokuje go. Esc i klik obok chowają wysunięty
  panel, „Przypnij panel” dokuje go (i wyłącza ustawienie „Panele chowają się same”).
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import type { PanelPlacement, PanelSide } from '../../logic/layout';
  import { panelMs, skipPanelMotion } from '../../logic/motion';
  import { useApp } from '../../state/context';
  import EdgeTab from './EdgeTab.svelte';
  import PeekLayer from './PeekLayer.svelte';
  import RightPanel from './RightPanel.svelte';
  import SessionsPanel from './SessionsPanel.svelte';

  interface Props {
    left: PanelPlacement;
    right: PanelPlacement;
    rightWidth: number;
    /** Znaczniki tylko w widoku rozmowy poza trybem skupienia. */
    show: boolean;
    /** Znaczniki przy krawędzi (uproszczony ekran je chowa; skrót dalej wysuwa panel). */
    tabs?: boolean;
  }

  let { left, right, rightWidth, show, tabs = true }: Props = $props();

  const app = useApp();
  const { t } = app.i18n;
  const sid = $derived(app.activeId);
  const side = $derived<PanelSide | null>(
    left === 'peek' ? 'left' : right === 'peek' ? 'right' : null,
  );

  $effect(() => {
    if (!show) untrack(() => (app.layout.peek = null));
  });

  let showTimer: number | undefined;
  const enter = (s: PanelSide) => {
    window.clearTimeout(showTimer);
    if (app.layout.peekable) showTimer = window.setTimeout(() => (app.layout.peek = s), 160);
  };
  const leaveTab = () => window.clearTimeout(showTimer);
  $effect(() => () => window.clearTimeout(showTimer));

  // Klawiatura: znacznik otwarty Enterem przenosi fokus do panelu, a po schowaniu wraca na znacznik.
  let focusIn = $state(false);
  let focusedSide: PanelSide | null = null;
  const fromTab = (s: PanelSide, open: () => void) => {
    focusIn = true;
    open();
    if (!app.layout.peek) focusIn = false;
    focusedSide = s;
  };
  // Panel wyjeżdża jeszcze chwilę po schowaniu (fokus bywa wtedy w nim), a zadokowany zastępuje
  // znacznik — fokus wraca na znacznik, a gdy go już nie ma, do pierwszej kontrolki panelu.
  $effect(() => {
    if (side || !focusedSide) return;
    const s = focusedSide;
    focusedSide = null;
    focusIn = false;
    const active = document.activeElement;
    if (active && active !== document.body && !active.closest('.peek')) return;
    window.setTimeout(() => {
      const now = document.activeElement;
      if (now && now !== document.body && !now.closest('.peek')) return;
      const target =
        document.querySelector<HTMLElement>(`.edge-tab.${s}`) ??
        document.querySelector<HTMLElement>(`aside.${s} button, aside.${s} input`);
      target?.focus();
    }, panelMs() + 20);
  });
  const pin = (s: PanelSide) => {
    // Najpierw stan układu (synchronicznie), żeby `update` nie wysunął panelu z powrotem.
    if (app.layout.autohide) {
      app.layout.autohide = false;
      void app.setSetting('ui.panels_autohide', false);
    }
    // Wysunięty panel zostaje na miejscu jako zadokowany — bez drugiego wjazdu.
    skipPanelMotion();
    app.layout.peek = null;
    app.layout.update(sid, s === 'left' ? { left_open: true } : { right_open: true });
  };
</script>

{#if show && tabs && left === 'hidden'}
  <EdgeTab
    side="left"
    label={t('edge.sessions')}
    onenter={() => enter('left')}
    onleave={leaveTab}
    onclick={() => fromTab('left', () => app.layout.toggleLeft(sid))}
  />
{/if}
{#if show && tabs && right === 'hidden'}
  <EdgeTab
    side="right"
    label={t('edge.panel')}
    onenter={() => enter('right')}
    onleave={leaveTab}
    onclick={() => fromTab('right', () => app.layout.toggleRight(sid))}
  />
{/if}
{#if show && side}
  {@const s = side}
  {#key s}
    <PeekLayer
      side={s}
      width={s === 'left' ? app.layout.leftWidth : rightWidth}
      label={s === 'left' ? t('sessions.title') : t('panel.label')}
      focusOnOpen={focusIn}
      onclose={() => (app.layout.peek = null)}
      onpin={() => pin(s)}
    >
      {#if s === 'left'}<SessionsPanel floating />{:else}<RightPanel />{/if}
    </PeekLayer>
  {/key}
{/if}
