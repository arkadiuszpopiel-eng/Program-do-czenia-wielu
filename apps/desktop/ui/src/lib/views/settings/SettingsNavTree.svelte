<!--
  Lista stron w nawigacji Ustawień: grupy (zwijane), strony
  z ikonami, znacznik bieżącej strony, liczba trafień podczas szukania. Strzałki, Home i End
  przenoszą fokus po widocznych stronach.
-->
<script lang="ts">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import type { SettingsPageDef } from '../../api/types-system';
  import type { SettingsGroup } from '../../logic/settings-groups';
  import { nextPage } from '../../logic/settings-nav';
  import { useApp } from '../../state/context';
  import { settingsIcon } from './settings-icons';

  interface Props {
    groups: readonly SettingsGroup<SettingsPageDef>[];
    collapsed: readonly string[];
    searching: boolean;
    current: string | undefined;
    hits: ReadonlyMap<string, number>;
    /** W wąskim oknie lista jest schowana, dopóki nie zostanie otwarta. */
    open: boolean;
    onchoose: (id: string) => void;
    ontoggle: (groupId: string) => void;
    onscrolled: (scrolled: boolean) => void;
  }
  let { groups, collapsed, searching, current, hits, open, onchoose, ontoggle, onscrolled }: Props =
    $props();

  const app = useApp();
  const { t } = app.i18n;
  let list = $state<HTMLElement | null>(null);

  function nodes(): HTMLButtonElement[] {
    return [...(list?.querySelectorAll<HTMLButtonElement>('button.node') ?? [])];
  }

  function listKey(event: KeyboardEvent) {
    const from = (event.target as HTMLElement).closest<HTMLButtonElement>('button.node');
    if (!from) return;
    const all = nodes();
    const ids = all.map((node) => node.dataset['page'] ?? '');
    const target = nextPage(ids, from.dataset['page'] ?? '', event.key);
    if (target === null) return;
    event.preventDefault();
    all[ids.indexOf(target)]?.focus();
  }

  export function focusFirst() {
    nodes()[0]?.focus();
  }

  /** Przewija listę tak, żeby strona była widoczna. */
  export function reveal(id: string) {
    list?.querySelector(`[data-page="${CSS.escape(id)}"]`)?.scrollIntoView({ block: 'nearest' });
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<nav
  id="settings-nav-list"
  aria-label={t('settings.tree')}
  class:open
  bind:this={list}
  onkeydown={listKey}
  onscroll={(event) => onscrolled(event.currentTarget.scrollTop > 2)}
>
  {#each groups as group (group.id)}
    {@const closed = !searching && collapsed.includes(group.id)}
    {@const name = t(`settings.group.${group.id}`)}
    {#if searching}
      <p class="group">{name}</p>
    {:else}
      <button
        type="button"
        class="group"
        aria-expanded={!closed}
        aria-controls="settings-group-{group.id}"
        aria-label={t('settings.nav.group', { name })}
        onclick={() => ontoggle(group.id)}
      >
        <span class="chev" class:open={!closed} aria-hidden="true">
          <ChevronDown size={12} strokeWidth={2} />
        </span>
        <span class="group-name">{name}</span>
        {#if closed}<span class="count" aria-hidden="true">{group.pages.length}</span>{/if}
      </button>
    {/if}
    <ul class="tree" id="settings-group-{group.id}" hidden={closed}>
      {#each group.pages as p (p.id)}
        {@const Icon = settingsIcon(p.id)}
        {@const found = hits.get(p.id) ?? 0}
        <li>
          <button
            type="button"
            class="node"
            class:later={p.wave > 1}
            data-page={p.id}
            aria-current={p.id === current && !searching ? 'page' : undefined}
            onclick={() => onchoose(p.id)}
          >
            <span class="ico" aria-hidden="true"><Icon size={16} strokeWidth={1.5} /></span>
            <span class="label">{app.i18n.text(p.label)}</span>
            {#if searching && found > 0}
              <span class="badge hits" aria-hidden="true">{found}</span>
            {:else if p.wave > 1}
              <span class="badge">{t('settings.soon')}</span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    {#if searching}<p class="none">{t('settings.nav.none')}</p>{/if}
  {/each}
</nav>

<style>
  nav {
    flex: 1;
    min-height: 0;
    padding: 0 var(--alfa-space-2) var(--alfa-space-3);
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: var(--alfa-color-border-strong) transparent;
  }
  .group {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-1);
    width: 100%;
    min-height: 28px;
    margin-top: var(--alfa-space-2);
    padding: 0 var(--alfa-space-2) 0 var(--alfa-space-1);
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
    letter-spacing: 0.05em;
    text-align: left;
    text-transform: uppercase;
  }
  .group:first-child {
    margin-top: 0;
  }
  button.group:hover {
    color: var(--alfa-color-text);
  }
  p.group {
    margin-bottom: 0;
    padding-left: var(--alfa-space-3);
  }
  .group-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chev {
    display: inline-flex;
    transform: rotate(-90deg);
    transition: transform var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .chev.open {
    transform: none;
  }
  .count {
    font-variant-numeric: tabular-nums;
    letter-spacing: 0;
  }
  .tree {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .tree[hidden] {
    display: none;
  }
  .node {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    width: 100%;
    min-height: 32px;
    padding: 0 var(--alfa-space-2) 0 var(--alfa-space-3);
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-sm);
    text-align: left;
  }
  :global(:root[data-density='compact']) .node {
    min-height: 28px;
  }
  .node:hover {
    background: var(--alfa-color-surface2);
  }
  .node[aria-current='page'] {
    background: var(--alfa-color-surface3);
    font-weight: var(--alfa-weight-semibold);
  }
  .node[aria-current='page']::before {
    content: '';
    position: absolute;
    left: 3px;
    top: 7px;
    bottom: 7px;
    width: 3px;
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-focus);
  }
  .ico {
    display: inline-flex;
    flex: none;
    color: var(--alfa-color-text-muted);
  }
  .node[aria-current='page'] .ico,
  .node:hover .ico {
    color: var(--alfa-color-text);
  }
  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .node.later .label {
    color: var(--alfa-color-text-muted);
  }
  .badge {
    flex: none;
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-full);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-regular);
    line-height: 18px;
  }
  .badge.hits {
    border-color: var(--alfa-color-border-strong);
    color: var(--alfa-color-text);
    font-variant-numeric: tabular-nums;
    font-weight: var(--alfa-weight-semibold);
  }
  .none {
    margin: var(--alfa-space-3) var(--alfa-space-2);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  @media (max-width: 720px) {
    nav:not(.open) {
      display: none;
    }
  }
</style>
