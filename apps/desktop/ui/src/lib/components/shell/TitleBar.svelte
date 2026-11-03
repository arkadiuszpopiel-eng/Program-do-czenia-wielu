<!--
  Pasek tytułu (36 px, PLAN §14.2): znak i nazwa programu, przełącznik panelu Sesje, ścieżka projekt ▸ sesja (klik = zmień
  nazwę), obsada α β γ δ z pierścieniem koloru (świeci mówiąca / pracująca), wskaźniki na żywo
  (ekran, kamera, mikrofon), znak „Pełny dostęp”, panel prawy, ustawienia. Stan (profil, tryb
  pracy, koszt) jest w pasku pisania (właściciel 29.10). Puste obszary to region przeciągania okna (Tauri); natywne przyciski
  okna (`WindowControls`) rysuje interfejs — okno nie ma belki systemowej. Wzdłuż dolnej krawędzi
  biegnie pasek LED (`TitleLed`).
-->
<script lang="ts">
  import type { Component } from 'svelte';
  import { IconButton } from '@alfa/ui-kit';
  import PanelLeft from '@lucide/svelte/icons/panel-left';
  import Settings from '@lucide/svelte/icons/settings';
  import PanelRight from '@lucide/svelte/icons/panel-right';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import { useApp } from '../../state/context';
  import AlfaMark from './AlfaMark.svelte';
  import CastStrip from './CastStrip.svelte';
  import MicIndicator from './MicIndicator.svelte';
  import ModeBadge from './ModeBadge.svelte';
  import TitleLed from './TitleLed.svelte';
  import WindowControls from './WindowControls.svelte';

  const app = useApp();
  const { t } = app.i18n;
  const session = $derived(app.sessions.active);
  const panels = $derived(app.layout.current(app.activeId));
  /** Uproszczony ekran (K.3, `ui.simple`): mniej przycisków; wskaźniki i tryb pracy zostają. */
  const simple = $derived(app.bool('ui.simple', false));
  const place = $derived(app.layout.placement(app.activeId));
  const renaming = $derived(
    session !== null && app.sessions.renamingId === session.id && app.sessions.renamingInTitle,
  );
  const title = $derived(session?.title ?? t('titlebar.noSession'));
  const crumbs = $derived(session?.project ? `${session.project.name} ▸ ${title}` : title);
  let draft = $state('');

  // Wskaźniki sterowania ekranem i kamery: osobny moduł (budżet startu), pobierany zaraz po starcie
  // w bezczynności (najpóźniej po 500 ms) — gotowy, zanim agentka sięgnie po ekran albo kamerę.
  // Nieudane pobranie ponawia co 2 s; gdy agentka już steruje ekranem albo kamera działa — od razu.
  let live = $state<{ gui: Component; camera: Component } | null>(null);
  let loading = false;
  const loadLive = (): void => {
    if (live || loading) return;
    loading = true;
    Promise.all([import('./GuiIndicator.svelte'), import('./CameraIndicator.svelte')])
      .then(([gui, camera]) => (live = { gui: gui.default, camera: camera.default }))
      .catch(() => window.setTimeout(loadLive, 2000))
      .finally(() => (loading = false));
  };
  $effect(() => {
    if (window.requestIdleCallback) window.requestIdleCallback(loadLive, { timeout: 500 });
    else window.setTimeout(loadLive, 0);
  });
  $effect(() => {
    const camera = app.cameras?.cameras.some((c) =>
      ['live', 'opening', 'closing'].includes(c.state),
    );
    if (app.work.gui || camera) loadLive();
  });

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
  <div class="zone left" data-tauri-drag-region>
    <div class="brand" data-tauri-drag-region>
      <AlfaMark size={18} />
      <span class="name" data-tauri-drag-region>Alfa</span>
    </div>
    {#if app.view !== 'settings' && !simple}
      <IconButton
        label={place.left === 'hidden' ? t('titlebar.showSessions') : t('titlebar.hideSessions')}
        size="sm"
        pressed={place.left !== 'hidden'}
        onclick={() => app.layout.toggleLeft(app.activeId)}
        aria-keyshortcuts="Control+B"
      >
        <PanelLeft size={16} strokeWidth={1.5} />
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
  </div>
  <div class="cast-center"><CastStrip /></div>
  <div class="zone right" data-tauri-drag-region>
    <div class="drag" data-tauri-drag-region></div>
    <div class="group live-group">
      {#if live}<live.gui /><live.camera />{/if}
      <MicIndicator />
    </div>
    <ModeBadge />
    <div class="group tools">
      {#if app.view !== 'settings' && !simple}
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
    </div>
    <WindowControls />
  </div>
  <TitleLed />
</header>

<style>
  /* Trzy strefy jak w aplikacjach ChatGPT i Claude na komputer (właściciel 27.10: „ładna
     i symetryczna”): lewa i prawa po równo (1fr), obsada dokładnie na środku — nic na nią nie
     wchodzi, bo nie leży już nad innymi elementami. Wszystkie kontrolki mają jedną wysokość. */
  .titlebar {
    position: relative;
    display: grid;
    /* Prawa strona nie kurczy się poniżej treści: gdy jest szersza niż połowa, obsada przesuwa
       się w lewo zamiast wchodzić pod kapsuły (lewa strona skraca okruszki). */
    grid-template-columns: minmax(0, 1fr) auto minmax(max-content, 1fr);
    align-items: center;
    column-gap: var(--alfa-space-3);
    --bar-h: 32px;
    --led-lane: 5px;
    height: var(--alfa-size-titlebar);
    /* Elementy są wyśrodkowane w części nad torem paska LED. */
    padding: 0 var(--alfa-space-2) var(--led-lane);
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
  .zone {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    min-width: 0;
    height: 100%;
  }
  .zone.right {
    justify-content: flex-end;
  }
  .group {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-1);
  }
  .group:empty {
    display: none;
  }
  .live-group:not(:has(*)) {
    display: none;
  }
  .titlebar :global(.icon-btn.sm),
  .crumbs {
    height: var(--bar-h);
    min-height: var(--bar-h);
  }
  .titlebar :global(.icon-btn.sm) {
    width: var(--bar-h);
  }
  .cast-center {
    display: flex;
    justify-content: center;
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
  @media (min-width: 721px) and (max-width: 1100px) {
    .titlebar,
    .zone {
      gap: var(--alfa-space-1);
    }
    .titlebar :global(.mic .text) {
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
    max-width: 100%;
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
  /* Wąskie okno: obsady nie ma na środku, więc zamiast trzech stref jeden rząd jak dawniej —
     siatka nie mieściła przycisków okna przy 360 px. */
  @media (max-width: 520px) {
    .titlebar {
      display: flex;
      align-items: center;
    }
    .zone {
      display: contents;
    }
    .titlebar,
    .zone {
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
    .zone {
      gap: 2px;
    }
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
