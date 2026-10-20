<!--
  Karta rozmowy z profilem (P4): tytuł, agentki (awatary), profil i model, ostatnia wiadomość jako
  zwykły tekst, data, koszt, znaczniki „Przypięta” / „Archiwum” i to samo menu akcji co wiersz
  listy. Enter otwiera, F2 zmienia nazwę, Delete usuwa; strzałki obsługuje siatka (`onmove`).
-->
<script lang="ts">
  import { Avatar, IconButton, Menu, agents, type MenuItem } from '@alfa/ui-kit';
  import Ellipsis from '@lucide/svelte/icons/ellipsis';
  import Pin from '@lucide/svelte/icons/pin';
  import Archive from '@lucide/svelte/icons/archive';
  import type { SessionCard, SessionSummary } from '../../api/types';
  import { knownAgents, plainSnippet } from '../../logic/session-cards';
  import { isStandardAgent } from '../../logic/work';
  import { now } from '../../state/clock.svelte';
  import { useApp } from '../../state/context';
  import { sessionMenuItems } from '../../state/session-menu';

  interface Props {
    session: SessionSummary;
    /** Dane z historii (`sessions_cards`); brak — karta bez agentek, fragmentu i kosztu. */
    card?: SessionCard;
    /** Nazwa projektu na karcie (grupy „Przypięte” i „Archiwum”, jak w wierszu listy). */
    showProject?: boolean;
    /** Strzałki, Home, End — ruch po siatce kart. */
    onmove: (event: KeyboardEvent) => void;
  }

  let { session, card, showProject = false, onmove }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const active = $derived(app.activeId === session.id);
  const renaming = $derived(
    app.sessions.renamingId === session.id && !app.sessions.renamingInTitle,
  );
  const items = $derived<MenuItem[]>(sessionMenuItems(app, session));
  const project = $derived(showProject ? (session.project?.name ?? '') : '');
  const cast = $derived(knownAgents(card?.agents ?? []));
  // Rdzeń oddaje już czysty tekst; ponowne czyszczenie chroni widok przed starszym rdzeniem.
  const snippet = $derived(card?.snippet ? plainSnippet(card.snippet) : '');
  const author = $derived.by(() => {
    const id = card?.snippet_author;
    if (!id) return '';
    if (id === 'user') return t('conv.you');
    return isStandardAgent(id) ? agents[id].name : id;
  });
  const label = $derived(
    [
      session.title,
      session.pinned ? t('sessions.card.pinned') : '',
      session.archived ? t('sessions.archived') : '',
      session.working ? t('sessions.working') : '',
      session.unread ? t('sessions.unread') : '',
    ]
      .filter(Boolean)
      .join(', '),
  );
  let draft = $state('');

  $effect(() => {
    if (renaming) draft = session.title;
  });

  function cardKey(event: KeyboardEvent) {
    if (event.key === 'F2') {
      event.preventDefault();
      event.stopPropagation();
      app.sessions.renamingInTitle = false;
      app.sessions.renamingId = session.id;
    } else if (event.key === 'Delete') {
      event.preventDefault();
      void app.deleteSession(session.id);
    } else {
      onmove(event);
    }
  }

  function renameKey(event: KeyboardEvent) {
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

<li class="card" class:active class:archived={session.archived}>
  {#if renaming}
    <label class="alfa-visually-hidden" for="rename-card-{session.id}"
      >{t('titlebar.renameLabel')}</label
    >
    <input
      id="rename-card-{session.id}"
      class="rename"
      bind:value={draft}
      onkeydown={renameKey}
      onblur={() => void app.renameSession(session.id, draft)}
      use:focusSelect
    />
  {:else}
    <button
      type="button"
      class="open"
      data-session-card
      aria-label={label}
      aria-describedby="card-{session.id}-info"
      aria-current={active ? 'page' : undefined}
      aria-keyshortcuts="Enter F2 Delete"
      onclick={() => void app.openSession(session.id)}
      onkeydown={cardKey}
    >
      <span class="head">
        <span
          class="dot"
          class:working={session.working}
          class:unread={session.unread && !session.working}
          aria-hidden="true"
        ></span>
        <span class="name">{session.title}</span>
      </span>
      <span class="info" id="card-{session.id}-info">
        {#if session.pinned || session.archived || project}
          <span class="badges">
            {#if session.pinned}
              <span class="badge"
                ><Pin size={11} strokeWidth={1.5} aria-hidden="true" />{t(
                  'sessions.card.pinned',
                )}</span
              >
            {/if}
            {#if session.archived}
              <span class="badge"
                ><Archive size={11} strokeWidth={1.5} aria-hidden="true" />{t(
                  'sessions.archived',
                )}</span
              >
            {/if}
            {#if project}
              <span class="badge project" title={project}>{project}</span>
            {/if}
          </span>
        {/if}
        <span class="profile">
          {#if cast.length}
            <span class="cast">
              {#each cast as id (id)}<Avatar agent={id} size={20} decorative />{/each}
              <span class="alfa-visually-hidden"
                >{t('sessions.card.agents', {
                  names: cast.map((id) => agents[id].name).join(', '),
                })}</span
              >
            </span>
          {/if}
          <span class="chip">{app.i18n.tk(`profile.${session.profile}`)}</span>
          {#if card?.model}
            <span class="model" title={t('sessions.card.model', { model: card.model })}
              >{card.model}</span
            >
          {/if}
        </span>
        <span class="snippet" class:empty={!snippet}>
          {#if snippet}<span class="author">{author}:</span> {snippet}{:else}{t(
              'sessions.card.empty',
            )}{/if}
        </span>
        <span class="foot">
          <span title={app.i18n.dateTime(session.updated_at)}
            ><span class="alfa-visually-hidden">{t('sessions.card.updated')}&nbsp;</span
            >{app.i18n.relative(session.updated_at, now())}</span
          >
          {#if card?.cost}
            <span class="cost">{t('sessions.card.cost', { cost: app.i18n.money(card.cost) })}</span>
          {/if}
        </span>
      </span>
    </button>
    <span class="menu">
      <Menu {items} label={t('sessions.actions', { title: session.title })}>
        {#snippet trigger(props)}
          <IconButton {...props} label={t('sessions.actions', { title: session.title })} size="sm">
            <Ellipsis size={14} strokeWidth={1.5} />
          </IconButton>
        {/snippet}
      </Menu>
    </span>
  {/if}
</li>

<style>
  .card {
    position: relative;
    min-width: 0;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-bg);
  }
  .card:hover,
  .card:focus-within {
    background: var(--alfa-color-surface2);
  }
  .card.active {
    border-color: var(--alfa-color-border-strong);
    background: var(--alfa-color-surface3);
  }
  .open {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    width: 100%;
    min-width: 0;
    padding: var(--alfa-space-2);
    padding-right: calc(var(--alfa-space-2) + 28px);
    border: 0;
    border-radius: inherit;
    background: transparent;
    color: var(--alfa-color-text);
    text-align: left;
    font-size: var(--alfa-font-size-sm);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    min-width: 0;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--alfa-weight-semibold);
  }
  .dot {
    flex: none;
    width: 6px;
    height: 6px;
    border-radius: var(--alfa-radius-full);
  }
  .dot.working {
    background: var(--alfa-color-success);
  }
  .dot.unread {
    background: var(--alfa-color-info);
  }
  .info {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    min-width: 0;
  }
  .badges,
  .profile,
  .foot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-1) var(--alfa-space-2);
    min-width: 0;
  }
  .badge,
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 0 var(--alfa-space-1);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-full);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .project {
    overflow: hidden;
    max-width: 100%;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cast {
    display: inline-flex;
    gap: 2px;
  }
  .model {
    overflow: hidden;
    min-width: 0;
    max-width: 100%;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .snippet {
    display: -webkit-box;
    overflow: hidden;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow-wrap: anywhere;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .snippet.empty {
    font-style: italic;
  }
  .author {
    color: var(--alfa-color-text);
    font-weight: var(--alfa-weight-semibold);
  }
  .foot {
    justify-content: space-between;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .menu {
    position: absolute;
    top: var(--alfa-space-1);
    right: var(--alfa-space-1);
    opacity: 0;
    transition: opacity var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .card:hover .menu,
  .card:focus-within .menu,
  .card.active .menu {
    opacity: 1;
  }
  .rename {
    width: 100%;
    height: 32px;
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-focus);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-bg);
    font-size: var(--alfa-font-size-sm);
  }
</style>
