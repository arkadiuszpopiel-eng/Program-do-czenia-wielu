<script lang="ts">
  import Search from '@lucide/svelte/icons/search';
  import Plus from '@lucide/svelte/icons/plus';
  import type { SessionItem } from '../types';

  interface Props {
    sessions: readonly SessionItem[];
    onselect?: (id: string) => void;
    onnew?: () => void;
  }

  let { sessions, onselect, onnew }: Props = $props();
  let query = $state('');
  const filtered = $derived(
    query ? sessions.filter((s) => s.title.toLowerCase().includes(query.toLowerCase())) : sessions,
  );
</script>

<nav class="sessions" aria-label="Sesje">
  <div class="head">
    <h2 class="title">Sesje</h2>
    <label class="search">
      <Search size={14} strokeWidth={1.5} aria-hidden="true" />
      <span class="alfa-visually-hidden">Szukaj sesji</span>
      <input type="search" placeholder="Szukaj" bind:value={query} aria-keyshortcuts="Control+P" />
    </label>
  </div>
  <ul class="list">
    {#each filtered as s (s.id)}
      <li>
        <button
          type="button"
          class="item"
          class:active={s.active}
          aria-current={s.active ? 'page' : undefined}
          onclick={() => onselect?.(s.id)}
        >
          <span class="dot" class:working={s.working} class:unread={s.unread} aria-hidden="true"
          ></span>
          <span class="text">
            <span class="name">{s.title}</span>
            {#if s.project}<span class="project">{s.project}</span>{/if}
          </span>
          {#if s.working}<span class="alfa-visually-hidden">(pracuje w tle)</span>{/if}
          {#if s.unread}<span class="alfa-visually-hidden">(nieprzeczytane)</span>{/if}
        </button>
      </li>
    {/each}
  </ul>
  <button type="button" class="new" onclick={onnew} aria-keyshortcuts="Control+N">
    <Plus size={14} strokeWidth={1.5} aria-hidden="true" /> Nowa rozmowa
  </button>
</nav>

<style>
  .sessions {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-width: 0;
    background: var(--alfa-color-surface);
    border-right: 1px solid var(--alfa-color-border);
  }
  .head {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-3) var(--alfa-space-3) var(--alfa-space-2);
  }
  .title {
    font-size: var(--alfa-font-size-xs);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--alfa-color-text-muted);
  }
  .search {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    height: 28px;
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
    border-color: var(--alfa-color-focus);
  }
  .list {
    flex: 1;
    margin: 0;
    padding: 0 var(--alfa-space-2);
    list-style: none;
    overflow: auto;
  }
  .item {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    width: 100%;
    min-height: 32px;
    padding: var(--alfa-space-1) var(--alfa-space-2);
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text);
    text-align: left;
    font-size: var(--alfa-font-size-sm);
  }
  .item:hover {
    background: var(--alfa-color-surface2);
  }
  .active {
    background: var(--alfa-color-surface3);
    font-weight: var(--alfa-weight-semibold);
  }
  .dot {
    flex: none;
    width: 6px;
    height: 6px;
    border-radius: var(--alfa-radius-full);
    background: transparent;
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
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-regular);
  }
  .new {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    margin: var(--alfa-space-2);
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
