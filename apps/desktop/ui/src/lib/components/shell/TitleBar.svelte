<!--
  Pasek tytułu (36 px, PLAN §14.2): przełącznik panelu Sesje, ścieżka projekt ▸ sesja (klik = zmień
  nazwę), obsada α β γ δ z pierścieniem koloru (świeci mówiąca / pracująca), stan (profil · L · koszt),
  panel prawy, ustawienia. Puste obszary to region przeciągania okna (Tauri); natywne przyciski
  okna rysuje system — miejsce na nie rezerwuje `--alfa-titlebar-controls`.
-->
<script lang="ts">
  import { Avatar, IconButton, Popover, agentIds, agents } from '@alfa/ui-kit';
  import Menu from '@lucide/svelte/icons/menu';
  import Settings from '@lucide/svelte/icons/settings';
  import PanelRight from '@lucide/svelte/icons/panel-right';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import { useApp } from '../../state/context';
  import CostDetails from './CostDetails.svelte';
  import MicIndicator from './MicIndicator.svelte';

  const app = useApp();
  const { t } = app.i18n;
  const session = $derived(app.sessions.active);
  const panels = $derived(app.layout.current(app.activeId));
  const place = $derived(app.layout.placement(app.activeId));
  const agentStates = $derived(app.activeId ? (app.agents[app.activeId] ?? []) : []);
  const renaming = $derived(session !== null && app.sessions.renamingId === session.id);
  const title = $derived(session?.title ?? t('titlebar.noSession'));
  const crumbs = $derived(session?.project ? `${session.project.name} ▸ ${title}` : title);
  const profile = $derived(t(`profile.${app.system?.profile ?? session?.profile ?? 'hybrid'}`));
  const level = $derived(session?.autonomy ?? 'L3');
  const cost = $derived(app.costs ? app.i18n.money(app.costs.session) : '—');
  let costOpen = $state(false);
  let draft = $state('');

  $effect(() => {
    if (renaming) draft = title;
  });

  function stateOf(id: (typeof agentIds)[number]) {
    return agentStates.find((a) => a.id === id);
  }

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

<header class="titlebar" data-tauri-drag-region>
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
      onclick={() => session && (app.sessions.renamingId = session.id)}
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
  <div class="cast" role="group" aria-label={t('titlebar.cast')}>
    {#each agentIds as id (id)}
      {@const st = stateOf(id)}
      <button
        type="button"
        class="cast-btn"
        aria-label={t('titlebar.agent', {
          name: agents[id].name,
          role: (st?.role_ids ?? []).map((r) => app.i18n.tk(`role.${r}`)).join(', ') || '—',
          status: t(`agentStatus.${st?.status ?? 'idle'}`),
        })}
        onclick={() => app.openPanel('agents')}
      >
        <Avatar
          agent={id}
          size={24}
          speaking={st?.status === 'speaking'}
          working={st?.status === 'working' || st?.status === 'waiting_approval'}
          decorative
        />
      </button>
    {/each}
  </div>
  <MicIndicator />
  <Popover label={t('costs.title')} bind:open={costOpen}>
    {#snippet trigger(props)}
      <button
        type="button"
        class="status"
        {...props}
        aria-label={t('titlebar.status', { profile, level, cost })}
      >
        <span class="dot" class:offline={app.system?.online === false} aria-hidden="true"></span>
        <span>{profile}</span>
        <span class="sep" aria-hidden="true">·</span>
        <span>{level}</span>
        <span class="sep" aria-hidden="true">·</span>
        <span class="num">{cost}</span>
      </button>
    {/snippet}
    <CostDetails />
  </Popover>
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
  <div class="controls" data-tauri-drag-region></div>
</header>

<style>
  .titlebar {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    height: var(--alfa-size-titlebar);
    padding: 0 var(--alfa-space-2);
    border-bottom: 1px solid var(--alfa-color-border);
    background: var(--alfa-color-surface);
    font-size: var(--alfa-font-size-sm);
    user-select: none;
  }
  .drag {
    flex: 1;
    align-self: stretch;
    min-width: var(--alfa-space-4);
  }
  .controls {
    width: var(--alfa-titlebar-controls, 0px);
    align-self: stretch;
  }
  .crumbs,
  .status,
  .cast-btn {
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
  .crumbs:hover:not(:disabled),
  .status:hover,
  .cast-btn:hover {
    background: var(--alfa-color-surface2);
  }
  .crumbs {
    min-width: 0;
    max-width: 40vw;
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
  .cast {
    display: flex;
  }
  .cast-btn {
    padding: 0 2px;
  }
  .status {
    color: var(--alfa-color-text-muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-success);
  }
  .dot.offline {
    background: var(--alfa-color-warning);
  }
  @media (max-width: 720px) {
    .status span:not(.dot):not(.num),
    .crumbs .project,
    .crumbs .sep,
    .cast {
      display: none;
    }
  }
</style>
