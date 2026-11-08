<!-- Panel Sesje (PLAN §14.8): wyszukiwanie pełnotekstowe, przypięte, projekty, archiwum, nowa rozmowa. -->
<script lang="ts">
  import { IconButton, Select } from '@alfa/ui-kit';
  import Plus from '@lucide/svelte/icons/plus';
  import Search from '@lucide/svelte/icons/search';
  import PanelLeftClose from '@lucide/svelte/icons/panel-left-close';
  import Archive from '@lucide/svelte/icons/archive';
  import List from '@lucide/svelte/icons/list';
  import LayoutGrid from '@lucide/svelte/icons/layout-grid';
  import { load } from '../../state/attempt';
  import { openPipelines, openProjects } from '../../state/board-dialog.svelte';
  import { useApp } from '../../state/context';
  import type { SessionGroup } from '../../state/sessions.svelte';
  import LoadFailed from './LoadFailed.svelte';
  import SessionRow from './SessionRow.svelte';
  import Lazy from './Lazy.svelte';

  interface Props {
    /** Panel jako szuflada / arkusz / wysunięty — bez przycisku chowania (zamyka go klik obok). */
    floating?: boolean;
  }

  let { floating = false }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const sessions = app.sessions;
  // Widok kart (seria 6) jest opcjonalny — ładowany leniwie, poza budżetem startowym okna.
  const loadCards = () => import('./SessionCards.svelte');
  let searching = $state(false);
  let searchError = $state<string | null>(null);
  /** Numer ostatniego wyszukiwania — spóźniona odpowiedź na stare zapytanie jest pomijana. */
  let latest = 0;

  /** Błąd wyszukiwania = komunikat z „Ponów", nigdy wieczne „szukam" ani „nic nie znaleziono". */
  async function search(query: string) {
    const token = ++latest;
    searching = true;
    const result = await load(() => app.client.sessions.search(query));
    if (token !== latest) return;
    searching = false;
    searchError = result.status === 'failed' ? result.error : null;
    sessions.hits = result.status === 'ready' ? [...result.value] : [];
  }

  $effect(() => {
    const query = sessions.query.trim();
    latest++;
    searchError = null;
    if (!query) {
      sessions.hits = [];
      searching = false;
      return;
    }
    searching = true;
    const handle = setTimeout(() => void search(query), 150);
    return () => clearTimeout(handle);
  });

  const cardsTitle = (group: SessionGroup) => groupTitle(group.kind, group.project?.name);

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
    <div class="view" role="group" aria-label={t('sessions.view')}>
      <IconButton
        label={t('sessions.view.list')}
        size="sm"
        pressed={sessions.view === 'list'}
        onclick={() => sessions.setView('list')}
      >
        <List size={16} strokeWidth={1.5} />
      </IconButton>
      <IconButton
        label={t('sessions.view.cards')}
        size="sm"
        pressed={sessions.view === 'cards'}
        onclick={() => sessions.setView('cards')}
      >
        <LayoutGrid size={16} strokeWidth={1.5} />
      </IconButton>
    </div>
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
        onclick={() => app.layout.update(app.activeId, { left_open: false })}
        aria-keyshortcuts="Control+B"
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
  <div class="period">
    <Select
      label={t('sessions.period')}
      size="sm"
      bind:value={sessions.period}
      options={[
        { value: 'all', label: t('sessions.period.all') },
        { value: 'today', label: t('sessions.period.today') },
        { value: 'week', label: t('sessions.period.week') },
        { value: 'month', label: t('sessions.period.month') },
      ]}
    />
  </div>
  <div class="scroll">
    {#if sessions.query.trim()}
      {#if searchError}
        <LoadFailed error={searchError} onretry={() => void search(sessions.query.trim())} />
      {:else}
        <p class="meta" aria-live="polite">
          {#if !searching}
            {sessions.hits.length
              ? t('sessions.results', { n: sessions.hits.length })
              : t('sessions.noResults', { query: sessions.query.trim() })}
          {/if}
        </p>
      {/if}
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
    {:else if sessions.groups.length === 0}
      <p class="meta">{t('sessions.periodEmpty')}</p>
    {:else if sessions.view === 'cards'}
      <Lazy
        load={loadCards}
        props={{ groups: sessions.groups, title: cardsTitle }}
        label={t('common.loading')}
      />
    {:else}
      {#each sessions.groups as group (group.id)}
        <section class="group" aria-labelledby="grp-{group.id}">
          <h3 class="group-title" id="grp-{group.id}">
            {#if group.kind === 'project' && group.project}
              <button
                type="button"
                class="proj-btn"
                onclick={() => openProjects({ projectName: group.project?.name ?? null })}
              >
                {groupTitle(group.kind, group.project.name)}
              </button>
            {:else}
              {groupTitle(group.kind, group.project?.name)}
            {/if}
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
    <div class="foot-top">
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
    <div class="foot-grid">
      <button
        type="button"
        class="proj"
        data-testid="open-projects-btn"
        onclick={() => openProjects()}
      >
        {t('sessions.addProject')}
      </button>
      <button
        type="button"
        class="proj"
        data-testid="open-pipelines-btn"
        onclick={() => openPipelines()}
      >
        {t('sessions.pipelinesBtn')}
      </button>
    </div>
  </div>
</nav>

<style>
  .period {
    padding: 0 var(--alfa-space-3) var(--alfa-space-2);
  }
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
  .view {
    display: flex;
    gap: 2px;
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
  .foot { display: flex; flex-direction: column; gap: var(--alfa-space-1); padding: var(--alfa-space-2); border-top: 1px solid var(--alfa-color-border); }
  .foot-top { display: flex; align-items: center; gap: var(--alfa-space-1); }
  .foot-grid { display: grid; grid-template-columns: 1fr 1fr; gap: var(--alfa-space-1); }
  .new {
    display: flex; align-items: center; gap: var(--alfa-space-2); flex: 1; min-width: 0; min-height: 32px;
    padding: 0 var(--alfa-space-3); border: 1px dashed var(--alfa-color-border-strong); border-radius: var(--alfa-radius-control);
    background: transparent; color: var(--alfa-color-text-muted); font-size: var(--alfa-font-size-sm); white-space: nowrap;
  }
  .new:hover, .proj:hover, .proj-btn:hover { background: var(--alfa-color-surface2); color: var(--alfa-color-text); }
  .proj {
    display: inline-flex; align-items: center; justify-content: center; min-width: 0; height: 30px; padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border); border-radius: var(--alfa-radius-control); background: var(--alfa-color-bg);
    color: var(--alfa-color-text-muted); font-size: var(--alfa-font-size-xs); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; cursor: pointer;
  }
  .proj-btn { padding: 2px 4px; border: 0; border-radius: var(--alfa-radius-control); background: transparent; color: inherit; font: inherit; cursor: pointer; }
</style>
