<!-- Panel Sesje (PLAN §14.8): wyszukiwanie pełnotekstowe, przypięte, projekty, archiwum, nowa rozmowa. -->
<script lang="ts">
  import { IconButton } from '@alfa/ui-kit';
  import Plus from '@lucide/svelte/icons/plus';
  import Search from '@lucide/svelte/icons/search';
  import PanelLeftClose from '@lucide/svelte/icons/panel-left-close';
  import Archive from '@lucide/svelte/icons/archive';
  import { useApp } from '../../state/context';
  import SessionRow from './SessionRow.svelte';

  interface Props {
    /** Panel jako szuflada / arkusz — bez przycisku zwijania do paska ikon. */
    floating?: boolean;
  }

  let { floating = false }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const sessions = app.sessions;
  let searching = $state(false);

  $effect(() => {
    const query = sessions.query.trim();
    if (!query) {
      sessions.hits = [];
      return;
    }
    searching = true;
    const handle = setTimeout(async () => {
      sessions.hits = [...(await app.client.sessions.search(query))];
      searching = false;
    }, 150);
    return () => clearTimeout(handle);
  });

  function groupTitle(kind: string, name: string | undefined): string {
    if (kind === 'pinned') return t('sessions.pinned');
    if (kind === 'archived') return t('sessions.archived');
    if (kind === 'loose') return t('sessions.recent');
    return name ?? '';
  }
</script>

<nav class="sessions" aria-label={t('sessions.title')}>
  <div class="head">
    <h2 class="title">{t('sessions.title')}</h2>
    <IconButton
      label={t('sessions.new')}
      size="sm"
      onclick={() => void app.newSession()}
      aria-keyshortcuts="Control+N"
    >
      <Plus size={16} strokeWidth={1.5} />
    </IconButton>
    {#if !floating}
      <IconButton
        label={t('sessions.collapse')}
        size="sm"
        onclick={() => app.layout.setLeftCollapsed(true)}
      >
        <PanelLeftClose size={16} strokeWidth={1.5} />
      </IconButton>
    {/if}
  </div>
  <div class="search">
    <Search size={14} strokeWidth={1.5} aria-hidden="true" />
    <label class="alfa-visually-hidden" for="alfa-sessions-search">{t('sessions.search')}</label>
    <input
      id="alfa-sessions-search"
      type="search"
      placeholder={t('sessions.searchPlaceholder')}
      bind:value={sessions.query}
      aria-keyshortcuts="Control+Shift+F"
    />
  </div>
  <div class="scroll">
    {#if sessions.query.trim()}
      <p class="meta" aria-live="polite">
        {#if !searching}
          {sessions.hits.length
            ? t('sessions.results', { n: sessions.hits.length })
            : t('sessions.noResults', { query: sessions.query.trim() })}
        {/if}
      </p>
      <ul class="hits">
        {#each sessions.hits as hit (hit.session_id + (hit.turn_id ?? ''))}
          <li>
            <button type="button" class="hit" onclick={() => void app.openSession(hit.session_id)}>
              <span class="hit-title">{hit.title}</span>
              {#if hit.snippet}<span class="hit-snippet">{hit.snippet}</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    {:else if sessions.list.length === 0}
      <p class="meta">{t('sessions.empty')}</p>
    {:else}
      {#each sessions.groups as group (group.id)}
        <section class="group" aria-labelledby="grp-{group.id}">
          <h3 class="group-title" id="grp-{group.id}">
            {groupTitle(group.kind, group.project?.name)}
          </h3>
          <ul class="list">
            {#each group.sessions as session (session.id)}
              <SessionRow
                {session}
                showProject={group.kind === 'pinned' || group.kind === 'archived'}
              />
            {/each}
          </ul>
        </section>
      {/each}
    {/if}
  </div>
  <div class="foot">
    <button type="button" class="new" onclick={() => void app.newSession()}>
      <Plus size={14} strokeWidth={1.5} aria-hidden="true" />
      {t('sessions.new')}
    </button>
    <IconButton
      label={t(sessions.showArchived ? 'sessions.hideArchived' : 'sessions.showArchived')}
      size="sm"
      pressed={sessions.showArchived}
      onclick={() => (sessions.showArchived = !sessions.showArchived)}
    >
      <Archive size={14} strokeWidth={1.5} />
    </IconButton>
  </div>
</nav>

<style>
  .sessions {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-width: 0;
    background: var(--alfa-color-surface);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-1);
    padding: var(--alfa-space-2) var(--alfa-space-2) var(--alfa-space-1) var(--alfa-space-3);
  }
  .title {
    flex: 1;
    font-size: var(--alfa-font-size-xs);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--alfa-color-text-muted);
  }
  .search {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    height: 32px;
    margin: 0 var(--alfa-space-2) var(--alfa-space-2);
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    color: var(--alfa-color-text-subtle);
    background: var(--alfa-color-bg);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: transparent;
    font-size: var(--alfa-font-size-sm);
  }
  .search input:focus {
    outline: none;
  }
  .search:focus-within {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
    outline-offset: 1px;
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 0 var(--alfa-space-2);
  }
  .group + .group {
    margin-top: var(--alfa-space-3);
  }
  .group-title {
    padding: var(--alfa-space-1) var(--alfa-space-2);
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
  }
  .list,
  .hits {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .meta {
    padding: var(--alfa-space-2);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .hit {
    display: flex;
    flex-direction: column;
    width: 100%;
    padding: var(--alfa-space-2);
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text);
    text-align: left;
    font-size: var(--alfa-font-size-sm);
  }
  .hit:hover {
    background: var(--alfa-color-surface2);
  }
  .hit-title {
    font-weight: var(--alfa-weight-semibold);
  }
  .hit-snippet {
    display: -webkit-box;
    overflow: hidden;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .foot {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-1);
    padding: var(--alfa-space-2);
    border-top: 1px solid var(--alfa-color-border);
  }
  .new {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    flex: 1;
    min-height: 32px;
    padding: 0 var(--alfa-space-3);
    border: 1px dashed var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .new:hover {
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text);
  }
</style>
