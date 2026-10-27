<!--
  Pasek tytułu (36 px, PLAN §14.2): znak i nazwa programu, przełącznik panelu Sesje, ścieżka projekt ▸ sesja (klik = zmień
  nazwę), obsada α β γ δ z pierścieniem koloru (świeci mówiąca / pracująca), stan (profil · L · koszt),
  panel prawy, ustawienia. Puste obszary to region przeciągania okna (Tauri); natywne przyciski
  okna (`WindowControls`) rysuje interfejs — okno nie ma belki systemowej. Wzdłuż dolnej krawędzi
  biegnie pasek LED: światło jedzie od lewej do prawej, szybciej, gdy któraś agentka pracuje.
-->
<script lang="ts">
  import { IconButton } from '@alfa/ui-kit';
  import Menu from '@lucide/svelte/icons/menu';
  import Settings from '@lucide/svelte/icons/settings';
  import PanelRight from '@lucide/svelte/icons/panel-right';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import { useApp } from '../../state/context';
  import AlfaMark from './AlfaMark.svelte';
  import CameraIndicator from './CameraIndicator.svelte';
  import CastStrip from './CastStrip.svelte';
  import GuiIndicator from './GuiIndicator.svelte';
  import MicIndicator from './MicIndicator.svelte';
  import StatusCluster from './StatusCluster.svelte';
  import WindowControls from './WindowControls.svelte';

  const app = useApp();
  const { t } = app.i18n;
  const session = $derived(app.sessions.active);
  const panels = $derived(app.layout.current(app.activeId));
  const place = $derived(app.layout.placement(app.activeId));
  const agentStates = $derived(app.activeId ? (app.agents[app.activeId] ?? []) : []);
  const renaming = $derived(
    session !== null && app.sessions.renamingId === session.id && app.sessions.renamingInTitle,
  );
  const title = $derived(session?.title ?? t('titlebar.noSession'));
  const crumbs = $derived(session?.project ? `${session.project.name} ▸ ${title}` : title);
  const busy = $derived(
    agentStates.some((a) => a.status === 'working' || a.status === 'speaking') ||
      app.conversation?.streaming !== undefined,
  );
  let draft = $state('');

  $effect(() => {
    if (renaming) draft = title;
  });

  function renameKey(event: KeyboardEvent) {
    if (!session) return;
    if (event.key === 'Enter') {
      event.preventDefault();
      void app.renameSession(session.id, draft);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      app.sessions.renamingId = null;
    }
  }

  function focusSelect(node: HTMLInputElement) {
    node.focus();
    node.select();
  }
</script>

<header class="titlebar">
  <div class="brand" data-tauri-drag-region>
    <AlfaMark size={18} />
    <span class="name" data-tauri-drag-region>Alfa</span>
  </div>
  {#if app.view !== 'settings'}
    <IconButton
      label={place.left === 'hidden' ? t('titlebar.showSessions') : t('titlebar.hideSessions')}
      size="sm"
      pressed={place.left !== 'hidden'}
      onclick={() =>
        app.layout.leftCollapsed
          ? app.layout.setLeftCollapsed(false)
          : app.layout.toggleLeft(app.activeId)}
      aria-keyshortcuts="Control+B"
    >
      <Menu size={16} strokeWidth={1.5} />
    </IconButton>
  {/if}
  {#if renaming && session}
    <label class="alfa-visually-hidden" for="alfa-rename">{t('titlebar.renameLabel')}</label>
    <input
      id="alfa-rename"
      class="rename"
      bind:value={draft}
      onkeydown={renameKey}
      onblur={() => void app.renameSession(session.id, draft)}
      use:focusSelect
    />
  {:else}
    <button
      type="button"
      class="crumbs"
      aria-label={t('titlebar.session', { path: crumbs })}
      aria-keyshortcuts="F2"
      disabled={!session}
      onclick={() => {
        if (!session) return;
        app.sessions.renamingInTitle = true;
        app.sessions.renamingId = session.id;
      }}
    >
      {#if session?.project}
        <span class="project">{session.project.name}</span>
        <span class="sep" aria-hidden="true">▸</span>
      {/if}
      <span class="session">{title}</span>
      {#if session}<ChevronDown size={14} strokeWidth={1.5} aria-hidden="true" />{/if}
    </button>
  {/if}
  <div class="drag" data-tauri-drag-region></div>
  <div class="cast-center"><CastStrip /></div>
  <GuiIndicator />
  <CameraIndicator />
  <MicIndicator />
  <StatusCluster />
  {#if app.view !== 'settings'}
    <IconButton
      label={place.right === 'hidden' ? t('titlebar.showRight') : t('titlebar.hideRight')}
      size="sm"
      pressed={panels.right_open && place.right !== 'hidden'}
      onclick={() => app.layout.toggleRight(app.activeId)}
      aria-keyshortcuts="Control+\\"
    >
      <PanelRight size={16} strokeWidth={1.5} />
    </IconButton>
  {/if}
  <IconButton
    label={t('titlebar.settings')}
    size="sm"
    pressed={app.view === 'settings'}
    onclick={() => (app.view === 'settings' ? app.closeSettings() : app.openSettings())}
    aria-keyshortcuts="Control+,"
  >
    <Settings size={16} strokeWidth={1.5} />
  </IconButton>
  <WindowControls />
  <div class="led" class:busy class:offline={app.system?.online === false} aria-hidden="true"></div>
</header>

<style>
  .titlebar {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    --led-lane: 5px;
    height: var(--alfa-size-titlebar);
    /* Elementy są wyśrodkowane w części nad torem paska LED. */
    padding: 0 var(--alfa-space-2) var(--led-lane) var(--alfa-space-3);
    border-bottom: 1px solid var(--alfa-color-border);
    background: var(--alfa-color-surface);
    font-size: var(--alfa-font-size-sm);
    user-select: none;
  }
  /* Znak i nazwa programu — jak na belce okna; też region przeciągania. */
  .brand {
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-2);
    align-self: stretch;
    padding: 0 var(--alfa-space-1) 0 var(--alfa-space-1);
  }
  .brand :global(svg) {
    pointer-events: none;
  }
  .name {
    font-weight: var(--alfa-weight-semibold);
    letter-spacing: 0.01em;
  }
  .drag {
    flex: 1;
    align-self: stretch;
    min-width: var(--alfa-space-4);
  }
  .cast-center {
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    z-index: 2;
  }
  .crumbs {
    max-width: calc(50vw - 210px);
  }
  /* Kapsuła sterowania ekranem bez słów do 1700 px: pełna („Delta steruje ekranem · Zatrzymaj”)
     wchodziła pod obsadę na środku belki (pozycja bezwzględna) i zasłaniała przyciski. Stan
     „Sterujesz Ty” zostaje ze słowami — bez nich przycisk etykiety byłby pusty. */
  @media (min-width: 721px) and (max-width: 1700px) {
    .titlebar :global(.live .text),
    .titlebar :global(.live .stop-text) {
      display: none;
    }
    /* Bez napisu „Zatrzymaj” zostaje kwadrat (nazwa przycisku w aria-label) — pusty przycisk
       nie miał nazwy ani rozmiaru celu (axe: button-name, target-size). */
    .titlebar :global(.live .stop-icon) {
      display: block;
    }
  }
  @media (min-width: 721px) and (max-width: 1400px) {
    .titlebar :global(.cluster .word) {
      display: none;
    }
  }
  @media (min-width: 721px) and (max-width: 1100px) {
    .titlebar {
      gap: var(--alfa-space-1);
    }
    .titlebar :global(.cluster > .anchor:last-child),
    .titlebar :global(.cluster .rule),
    .titlebar :global(.mic .text) {
      display: none;
    }
    .crumbs {
      max-width: calc(50vw - 165px);
    }
  }
  @media (min-width: 721px) and (max-width: 900px) {
    .titlebar :global(.cluster) {
      display: none;
    }
    .titlebar :global(.cluster .seg) {
      padding: 0 var(--alfa-space-1);
    }
    .titlebar :global(.cluster .state .ico),
    .titlebar :global(.cluster .level svg) {
      display: none;
    }
  }
  /* Pasek LED ma własny tor pod elementami belki (`--led-lane`): listwa w barwach agentek i jasne
     światło jadące od lewej do prawej. Nic z paska nie wchodzi na przyciski i kapsuły. */
  .led {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 3px;
    overflow: hidden;
    pointer-events: none;
    /* Barwy agentek rozjaśnione do świecących — czytelne na jasnej i ciemnej belce. */
    --led-a: oklch(66% 0.23 27);
    --led-b: oklch(66% 0.22 292);
    --led-c: oklch(72% 0.15 215);
    --led-d: oklch(72% 0.17 162);
  }
  .led::before {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, var(--led-a), var(--led-b), var(--led-c), var(--led-d));
    opacity: 0.38;
  }
  .led::after {
    content: '';
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    width: 28%;
    background:
      linear-gradient(90deg, transparent 55%, #ffffff 82%, transparent 97%) center / 100% 1px
        no-repeat,
      linear-gradient(
        90deg,
        transparent,
        var(--led-a) 22%,
        var(--led-b) 50%,
        var(--led-c) 76%,
        var(--led-d) 92%,
        transparent
      );
    transform: translateX(-100%);
    animation: led-run 3.2s linear infinite;
  }
  .led.busy::after {
    animation-duration: 1.4s;
  }
  .led.offline {
    --led-a: var(--alfa-color-warning);
    --led-b: var(--alfa-color-warning);
    --led-c: var(--alfa-color-warning);
    --led-d: var(--alfa-color-warning);
  }
  @keyframes led-run {
    to {
      transform: translateX(358%);
    }
  }
  /* Animacje wyłączone: światło stoi — zostaje równa, jaśniejsza listwa. */
  :global(:root[data-motion='off']) .led::after {
    display: none;
  }
  :global(:root[data-motion='off']) .led::before {
    opacity: 0.85;
  }
  @media (prefers-reduced-motion: reduce) {
    .led::after {
      display: none;
    }
    .led::before {
      opacity: 0.85;
    }
  }
  @media (forced-colors: active) {
    .led {
      display: none;
    }
  }
  .crumbs {
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-1);
    min-height: 28px;
    padding: 0 var(--alfa-space-2);
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text);
  }
  .crumbs:hover:not(:disabled) {
    background: var(--alfa-color-surface2);
  }
  .crumbs {
    min-width: 0;
    max-width: max(60px, calc(50vw - 210px));
    font-weight: var(--alfa-weight-semibold);
  }
  .session {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .project {
    color: var(--alfa-color-text-muted);
    font-weight: var(--alfa-weight-regular);
    white-space: nowrap;
  }
  .rename {
    width: min(360px, 40vw);
    height: 28px;
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-focus);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-bg);
  }
  .sep {
    color: var(--alfa-color-text-subtle);
  }
  @media (max-width: 520px) {
    .titlebar {
      gap: var(--alfa-space-1);
    }
    .brand {
      display: none;
    }
    .drag {
      min-width: 0;
    }
    .crumbs {
      padding: 0 var(--alfa-space-1);
    }
  }
  @media (max-width: 380px) {
    .titlebar {
      gap: 2px;
      padding-left: var(--alfa-space-1);
    }
  }
  @media (max-width: 720px) {
    .name {
      display: none;
    }
    .crumbs .project,
    .crumbs .sep {
      display: none;
    }
  }
</style>
