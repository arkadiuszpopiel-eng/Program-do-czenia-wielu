<!--
  Powłoka okna głównego (PLAN §14.1–14.2): pasek tytułu, panel Sesje (zadokowany / pasek ikon /
  szuflada / arkusz), kolumna rozmowy albo Ustawienia, panel prawy, toasty, ogłoszenia aria-live
  i nakładki ładowane leniwie (paleta, ściągawka). Tryb skupienia chowa wszystko poza rozmową.
-->
<script lang="ts">
  import { ResizeHandle } from '@alfa/ui-kit';
  import { LEFT_MAX, LEFT_MIN, RIGHT_MAX, RIGHT_MIN, dockedRightWidth } from '../../logic/layout';
  import { useApp } from '../../state/context';
  import ConversationView from '../conversation/ConversationView.svelte';
  import { focusTrap } from './focus-trap';
  import Lazy from './Lazy.svelte';
  import { templateDialog } from '../../state/template-dialog.svelte';
  import { boardDialog } from '../../state/board-dialog.svelte';
  import { statsDialog } from '../../state/stats-dialog.svelte';
  import { canvas, closeCanvas } from '../../state/canvas.svelte';
  import RightPanel from './RightPanel.svelte';
  import SessionsPanel from './SessionsPanel.svelte';
  import SessionsRail from './SessionsRail.svelte';
  import TitleBar from './TitleBar.svelte';
  import ToastHost from './ToastHost.svelte';

  const app = useApp();
  const { t } = app.i18n;
  const sid = $derived(app.activeId);
  const inSettings = $derived(app.view === 'settings');
  const place = $derived(app.layout.placement(sid));
  const left = $derived(inSettings ? 'hidden' : place.left);
  const right = $derived(inSettings ? 'hidden' : place.right);
  const rightWidth = $derived(dockedRightWidth(app.layout.rightWidth, app.layout.mode));
  const rail = $derived(left === 'rail' || (left === 'docked' && app.layout.leftCollapsed));
  // „Dopasowana”: kolumna rośnie z oknem — od wąskiej (72 znaki) do ok. 110 znaków, potem margines.
  const COLUMNS: Record<string, string> = {
    auto: 'clamp(var(--alfa-size-reading-column), 68%, 1000px)',
    narrow: 'var(--alfa-size-reading-column)',
    wide: 'var(--alfa-size-reading-column-wide)',
  };
  const column = $derived(COLUMNS[app.str('ui.column', 'auto')] ?? COLUMNS['auto']);

  const loadPalette = () => import('../overlays/PaletteHost.svelte');
  const loadCheatsheet = () => import('../overlays/Cheatsheet.svelte');
  const loadTemplates = () => import('../overlays/TemplateDialog.svelte');
  const loadBoard = () => import('../overlays/BoardDialog.svelte');
  const loadStats = () => import('../overlays/SessionStatsDialog.svelte');
  const loadSettings = () => import('../../views/settings/SettingsView.svelte');
  const loadCanvas = () => import('../canvas/ArtifactCanvas.svelte');
  // Kanwa (C2.8): obok rozmowy na szerokim i średnim oknie, w wąskim — arkusz nad treścią.
  const canvasOpen = $derived(canvas.item !== null && !inSettings);
  const canvasDocked = $derived(app.layout.mode === 'wide' || app.layout.mode === 'medium');
  let paletteReady = $state(false);
  let cheatsheetReady = $state(false);

  // Paleta: moduł pobierany w bezczynności po starcie, żeby otwarcie mieściło się w 50 ms.
  $effect(() => {
    const idle = window.requestIdleCallback ?? ((cb: () => void) => window.setTimeout(cb, 400));
    idle(() => void loadPalette().then(() => (paletteReady = true)));
  });
  $effect(() => {
    if (app.palette.open) paletteReady = true;
    if (app.cheatsheetOpen) cheatsheetReady = true;
  });

  const closeLeft = () => app.layout.update(sid, { left_open: false });
  const closeRight = () => app.layout.update(sid, { right_open: false });
</script>

<div
  class="shell mode-{app.layout.mode}"
  data-surface="app"
  class:focus={app.layout.focus}
  style:--conv-column={column}
>
  <a class="skip" href="#alfa-composer">{t('app.skip')}</a>
  <div class="titlebar-wrap">
    <TitleBar />
  </div>
  <div class="body">
    {#if rail}
      <SessionsRail />
    {:else if left === 'docked'}
      <aside class="left" style:width="{app.layout.leftWidth}px">
        <SessionsPanel />
        <ResizeHandle
          value={app.layout.leftWidth}
          min={LEFT_MIN}
          max={LEFT_MAX}
          side="left"
          label={t('sessions.resize')}
          onchange={(w) => app.layout.setLeftWidth(w)}
        />
      </aside>
    {/if}
    <main class="center">
      {#if inSettings}
        <Lazy load={loadSettings} props={{}} label={t('common.loading')} />
      {:else}
        <ConversationView />
      {/if}
    </main>
    {#if canvasOpen && canvasDocked}
      <aside class="canvas-dock" aria-label={t('canvas.title')}>
        <Lazy load={loadCanvas} props={{}} label={t('canvas.loading')} />
      </aside>
    {/if}
    {#if right === 'docked'}
      <aside class="right" style:width="{rightWidth}px">
        <ResizeHandle
          value={rightWidth}
          min={RIGHT_MIN}
          max={RIGHT_MAX}
          side="right"
          label={t('panel.resize')}
          onchange={(w) => app.layout.setRightWidth(w)}
        />
        <RightPanel />
      </aside>
    {/if}
  </div>

  {#if left === 'drawer' || left === 'sheet'}
    <button type="button" class="scrim" tabindex="-1" aria-hidden="true" onclick={closeLeft}
    ></button>
    <div
      class="drawer from-left {left}"
      role="dialog"
      aria-modal="true"
      aria-label={t('sessions.title')}
      use:focusTrap
    >
      <SessionsPanel floating />
    </div>
  {/if}
  {#if right === 'drawer' || right === 'sheet'}
    <button type="button" class="scrim" tabindex="-1" aria-hidden="true" onclick={closeRight}
    ></button>
    <div
      class="drawer from-right {right}"
      role="dialog"
      aria-modal="true"
      aria-label={t('panel.label')}
      style:width={right === 'drawer' ? `${app.layout.rightWidth}px` : undefined}
      use:focusTrap
    >
      <RightPanel />
    </div>
  {/if}

  {#if canvasOpen && !canvasDocked}
    <button type="button" class="scrim" tabindex="-1" aria-hidden="true" onclick={closeCanvas}
    ></button>
    <div
      class="drawer from-right canvas-sheet"
      role="dialog"
      aria-modal="true"
      aria-label={t('canvas.title')}
      use:focusTrap
    >
      <Lazy load={loadCanvas} props={{}} label={t('canvas.loading')} />
    </div>
  {/if}

  {#if app.layout.focus}
    <p class="focus-hint" role="status">{t('focus.on')}</p>
  {/if}
  <ToastHost />
  <div class="alfa-visually-hidden" role="status" aria-live="polite" aria-atomic="true">
    {app.announcement}
  </div>
  {#if paletteReady}<Lazy load={loadPalette} props={{}} label={t('palette.title')} />{/if}
  {#if cheatsheetReady}<Lazy load={loadCheatsheet} props={{}} label={t('cheatsheet.title')} />{/if}
  {#if templateDialog.open}<Lazy
      load={loadTemplates}
      props={{}}
      label={t('tpl.dialog.title')}
    />{/if}
  {#if boardDialog.open}<Lazy load={loadBoard} props={{}} label={t('board.title')} />{/if}
  {#if statsDialog.open}<Lazy load={loadStats} props={{}} label={t('stats.title')} />{/if}
</div>

<style>
  .shell {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text);
  }
  .skip {
    position: absolute;
    top: -40px;
    left: var(--alfa-space-2);
    z-index: 200;
    padding: var(--alfa-space-1) var(--alfa-space-3);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-overlay, var(--alfa-color-surface));
  }
  .skip:focus {
    top: var(--alfa-space-2);
  }
  .body {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .left,
  .right {
    position: relative;
    flex: none;
    min-height: 0;
  }
  .left {
    border-right: 1px solid var(--alfa-color-border);
  }
  .center {
    flex: 1;
    min-width: 0;
    min-height: 0;
  }
  .canvas-dock {
    flex: none;
    width: clamp(320px, 40%, 760px);
    min-height: 0;
    border-left: 1px solid var(--alfa-color-border);
  }
  .drawer.canvas-sheet {
    width: min(720px, 100%);
    max-width: 100%;
  }
  .scrim {
    position: fixed;
    inset: var(--alfa-size-titlebar) 0 0 0;
    z-index: 40;
    border: 0;
    background: var(--alfa-color-scrim);
    animation: fade var(--alfa-duration-panel) var(--alfa-ease-out);
  }
  .drawer {
    position: fixed;
    top: var(--alfa-size-titlebar);
    bottom: 0;
    z-index: 41;
    display: flex;
    flex-direction: column;
    width: min(360px, 92vw);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-3);
  }
  .from-left {
    left: 0;
    border-right: 1px solid var(--alfa-color-border);
    animation: slide-left var(--alfa-duration-panel) var(--alfa-ease-out);
  }
  .from-right {
    right: 0;
    max-width: 92vw;
    border-left: 1px solid var(--alfa-color-border);
    animation: slide-right var(--alfa-duration-panel) var(--alfa-ease-out);
  }
  .drawer.sheet {
    left: 0;
    right: 0;
    width: 100%;
    max-width: none;
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
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
  /* Tryb skupienia: pasek tytułu wysuwa się dopiero po najechaniu na górną krawędź / fokusie. */
  .focus .titlebar-wrap {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    z-index: 50;
    padding-bottom: 8px;
    transform: translateY(calc(-100% + 8px));
    opacity: 0;
    transition:
      transform var(--alfa-duration-panel) var(--alfa-ease-out),
      opacity var(--alfa-duration-panel) var(--alfa-ease-out);
  }
  .focus .titlebar-wrap:hover,
  .focus .titlebar-wrap:focus-within {
    transform: none;
    opacity: 1;
  }
  .focus-hint {
    position: fixed;
    top: var(--alfa-space-3);
    left: 50%;
    z-index: 30;
    padding: var(--alfa-space-1) var(--alfa-space-3);
    transform: translateX(-50%);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    animation: fade-out 2.5s var(--alfa-ease-out) forwards;
  }
  @keyframes fade-out {
    70% {
      opacity: 1;
    }
    to {
      opacity: 0;
    }
  }
</style>
