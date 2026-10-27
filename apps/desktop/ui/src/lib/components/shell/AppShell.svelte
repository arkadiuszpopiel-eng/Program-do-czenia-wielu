<!--
  Powłoka okna głównego (PLAN §14.1–14.2): pasek tytułu, panel Sesje (zadokowany / pasek ikon /
  szuflada / arkusz), kolumna rozmowy albo Ustawienia, panel prawy, toasty, ogłoszenia aria-live
  i nakładki ładowane leniwie (paleta, ściągawka). Tryb skupienia chowa wszystko poza rozmową.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { ResizeHandle } from '@alfa/ui-kit';
  import { canvasDock } from '../../logic/canvas';
  import {
    LEFT_MAX,
    LEFT_MIN,
    LEFT_RAIL,
    RIGHT_MAX,
    RIGHT_MIN,
    dockedRightWidth,
  } from '../../logic/layout';
  import { useApp } from '../../state/context';
  import ConversationView from '../conversation/ConversationView.svelte';
  import { focusTrap } from './focus-trap';
  import Lazy from './Lazy.svelte';
  import { templateDialog } from '../../state/template-dialog.svelte';
  import { boardDialog, pipelinesDialog, projectsDialog } from '../../state/board-dialog.svelte';
  import { statsDialog } from '../../state/stats-dialog.svelte';
  import { canvas, closeCanvas, syncCanvas } from '../../state/canvas.svelte';
  import { arenaDialog } from '../../state/arena.svelte';
  import { teachDialog } from '../../state/teach-dialog.svelte';
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
  const baseLeft = $derived(inSettings ? 'hidden' : place.left);
  const baseRight = $derived(inSettings ? 'hidden' : place.right);
  const rightWidth = $derived(dockedRightWidth(app.layout.rightWidth, app.layout.mode));
  const baseRail = $derived(
    baseLeft === 'rail' || (baseLeft === 'docked' && app.layout.leftCollapsed),
  );
  // Kanwa (C2.8): obok rozmowy na szerokim i średnim oknie — szerokość z miejsca po panelach,
  // rozmowa zachowuje ≥ 480 px (lewy panel zwija się do paska, a w razie potrzeby prawy się
  // chowa, dopóki kanwa jest otwarta); gdy się nie mieści albo okno jest wąskie — arkusz.
  const canvasOpen = $derived(canvas.item !== null && !inSettings);
  let bodyWidth = $state(0);
  const dock = $derived.by(() => {
    if (!canvasOpen || (app.layout.mode !== 'wide' && app.layout.mode !== 'medium')) return null;
    const leftDock =
      baseRail || baseLeft === 'docked'
        ? { width: baseRail ? LEFT_RAIL : app.layout.leftWidth, rail: LEFT_RAIL }
        : null;
    const body = bodyWidth || window.innerWidth;
    return canvasDock(body, leftDock, baseRight === 'docked' ? rightWidth : 0);
  });
  const canvasDocked = $derived(dock !== null);
  // Arkusz kanwy jest jedynym oknem modalnym: szuflady paneli chowają się, dopóki jest otwarty.
  const overlay = (p: string) => p === 'drawer' || p === 'sheet';
  const sheetOpen = $derived(canvasOpen && !canvasDocked);
  const left = $derived(sheetOpen && overlay(baseLeft) ? 'hidden' : baseLeft);
  const right = $derived(
    (sheetOpen && overlay(baseRight)) || dock?.hideRight ? 'hidden' : baseRight,
  );
  const rail = $derived(baseRail || (left === 'docked' && dock?.railLeft === true));
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
  const loadProjects = () => import('../overlays/ProjectsDialog.svelte');
const loadPipelines = () => import('../overlays/PipelinesDialog.svelte');
  const loadStats = () => import('../overlays/SessionStatsDialog.svelte');
  const loadArena = () => import('../overlays/ArenaDialog.svelte');
  const loadTeach = () => import('../overlays/TeachDialog.svelte');
  const loadSettings = () => import('../../views/settings/SettingsView.svelte');
  const loadCanvas = () => import('../canvas/ArtifactCanvas.svelte');
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

  // Kanwa należy do sesji i tury, z której ją otwarto (zmiana sesji / ścieżki rozmowy ją zamyka).
  $effect(() => {
    const id = sid;
    const conv = app.conversation;
    const ids =
      conv?.loaded && conv.sessionId === id ? new Set(conv.path.map((t) => t.id)) : null;
    untrack(() => syncCanvas(id, ids ? (turnId) => ids.has(turnId) : null));
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
  <div class="body" bind:clientWidth={bodyWidth}>
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
    {#if dock}
      <aside class="canvas-dock" style:width="{dock.width}px" aria-label={t('canvas.title')}>
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

  {#if sheetOpen}
    <button type="button" class="scrim" tabindex="-1" aria-hidden="true" onclick={closeCanvas}
    ></button>
    <div
      class="drawer from-right canvas-sheet"
      role="dialog"
      aria-modal="true"
      aria-label={t('canvas.title')}
      tabindex="-1"
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
  {#if projectsDialog.open}<Lazy load={loadProjects} props={{}} label={t('sessions.projects')} />{/if}
{#if pipelinesDialog.open}<Lazy load={loadPipelines} props={{}} label={t('sessions.pipelines')} />{/if}
  {#if statsDialog.open}<Lazy load={loadStats} props={{}} label={t('stats.title')} />{/if}
  {#if arenaDialog.open}<Lazy load={loadArena} props={{}} label={t('arena.title')} />{/if}
  {#if teachDialog.open}<Lazy load={loadTeach} props={{}} label={t('teach.title')} />{/if}
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
