<!-- Wiersz sesji: wybór, kropka aktywności / nieprzeczytane, zmiana nazwy w miejscu (F2), menu akcji. -->
<script lang="ts">
  import { IconButton, Menu, type MenuItem } from '@alfa/ui-kit';
  import Ellipsis from '@lucide/svelte/icons/ellipsis';
  import Pin from '@lucide/svelte/icons/pin';
  import type { SessionSummary } from '../../api/types';
  import { useApp } from '../../state/context';
  import { exportConversation } from '../../state/exports';

  interface Props {
    session: SessionSummary;
    showProject?: boolean;
  }

  let { session, showProject = false }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const active = $derived(app.activeId === session.id);
  const renaming = $derived(app.sessions.renamingId === session.id);
  let draft = $state('');

  $effect(() => {
    if (renaming) draft = session.title;
  });

  const items = $derived<MenuItem[]>([
    {
      id: 'rename',
      label: t('sessions.rename'),
      onSelect: () => (app.sessions.renamingId = session.id),
    },
    {
      id: 'pin',
      label: t(session.pinned ? 'sessions.unpin' : 'sessions.pin'),
      onSelect: () => void app.setPinned(session.id, !session.pinned),
    },
    {
      id: 'archive',
      label: t(session.archived ? 'sessions.unarchive' : 'sessions.archive'),
      onSelect: () => void app.setArchived(session.id, !session.archived),
    },
    {
      id: 'duplicate',
      label: t('sessions.duplicate'),
      onSelect: () => void app.client.sessions.duplicateAsTemplate(session.id),
    },
    {
      id: 'export',
      label: t('sessions.export'),
      onSelect: () => void app.exportSession(session.id),
    },
    {
      id: 'export-md',
      label: t('exp.markdown'),
      onSelect: () => void exportConversation(app, session.id, 'markdown'),
    },
    {
      id: 'export-html',
      label: t('exp.html'),
      onSelect: () => void exportConversation(app, session.id, 'html'),
    },
    {
      id: 'delete',
      label: t('sessions.delete'),
      danger: true,
      separatorBefore: true,
      onSelect: () => void app.deleteSession(session.id),
    },
  ]);

  function rowKey(event: KeyboardEvent) {
    if (event.key === 'F2') {
      event.preventDefault();
      event.stopPropagation();
      app.sessions.renamingId = session.id;
    } else if (event.key === 'Delete') {
      event.preventDefault();
      void app.deleteSession(session.id);
    } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const rows = Array.from(document.querySelectorAll<HTMLElement>('[data-session-row]'));
      const at = rows.indexOf(event.currentTarget as HTMLElement);
      rows[at + (event.key === 'ArrowDown' ? 1 : -1)]?.focus();
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

<li class="row" class:active>
  {#if renaming}
    <label class="alfa-visually-hidden" for="rename-{session.id}">{t('titlebar.renameLabel')}</label
    >
    <input
      id="rename-{session.id}"
      class="rename"
      bind:value={draft}
      onkeydown={renameKey}
      onblur={() => void app.renameSession(session.id, draft)}
      use:focusSelect
    />
  {:else}
    <button
      type="button"
      class="item"
      data-session-row
      aria-current={active ? 'page' : undefined}
      aria-keyshortcuts="F2 Delete"
      onclick={() => void app.openSession(session.id)}
      onkeydown={rowKey}
    >
      <span
        class="dot"
        class:working={session.working}
        class:unread={session.unread && !session.working}
        aria-hidden="true"
      ></span>
      <span class="text">
        <span class="name">{session.title}</span>
        {#if showProject && session.project}<span class="project">{session.project.name}</span>{/if}
      </span>
      {#if session.pinned}<Pin size={12} strokeWidth={1.5} aria-hidden="true" class="pin" />{/if}
      {#if session.working}<span class="alfa-visually-hidden">({t('sessions.working')})</span>{/if}
      {#if session.unread}<span class="alfa-visually-hidden">({t('sessions.unread')})</span>{/if}
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
  .row {
    position: relative;
    display: flex;
    align-items: center;
    border-radius: var(--alfa-radius-control);
  }
  .row:hover,
  .row:focus-within {
    background: var(--alfa-color-surface2);
  }
  .row.active {
    background: var(--alfa-color-surface3);
  }
  .item {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    flex: 1;
    min-width: 0;
    min-height: 32px;
    padding: var(--alfa-space-1) var(--alfa-space-2);
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text);
    text-align: left;
    font-size: var(--alfa-font-size-sm);
  }
  .active .item {
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
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .project {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-regular);
  }
  .item :global(.pin) {
    flex: none;
    margin-left: auto;
    color: var(--alfa-color-text-subtle);
  }
  .menu {
    opacity: 0;
    transition: opacity var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .row:hover .menu,
  .row:focus-within .menu {
    opacity: 1;
  }
  .rename {
    flex: 1;
    min-width: 0;
    height: 32px;
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-focus);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-bg);
    font-size: var(--alfa-font-size-sm);
  }
</style>
