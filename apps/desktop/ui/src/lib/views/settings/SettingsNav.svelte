<!--
  Nawigacja Ustawień: nagłówek (powrót, tytuł, wyszukiwarka z czyszczeniem i skrótem Ctrl+F) stoi
  w miejscu, przewija się tylko lista. Grupy z ikonami stron można zwijać (stan zapamiętany),
  bieżąca strona ma znacznik przy krawędzi i sama wjeżdża w pole widzenia. Podczas szukania lista
  pokazuje tylko strony z trafieniami i ich liczbę. Strzałki, Home i End chodzą po liście.
  W wąskim oknie lista chowa się pod przyciskiem z nazwą bieżącej strony. Całe menu można schować
  za znacznik przy krawędzi (29.10) — wtedy wysuwa się nad stronę po najechaniu.
-->
<script lang="ts">
  import { Button, IconButton } from '@alfa/ui-kit';
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import ChevronsDownUp from '@lucide/svelte/icons/chevrons-down-up';
  import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down';
  import PanelRightClose from '@lucide/svelte/icons/panel-right-close';
  import Search from '@lucide/svelte/icons/search';
  import X from '@lucide/svelte/icons/x';
  import { untrack } from 'svelte';
  import type { SettingsPageDef } from '../../api/types-system';
  import type { SettingsGroup } from '../../logic/settings-groups';
  import { readCollapsed, toggled, writeCollapsed } from '../../logic/settings-nav';
  import { useApp } from '../../state/context';
  import SettingsNavTree from './SettingsNavTree.svelte';
  import { settingsIcon } from './settings-icons';

  interface Props {
    groups: readonly SettingsGroup<SettingsPageDef>[];
    /** Otwarta strona (`undefined`, dopóki schemat się ładuje). */
    current: string | undefined;
    query: string;
    /** Trafienia wyszukiwarki per strona (puste, gdy nic nie wpisano). */
    hits: ReadonlyMap<string, number>;
    onopen: (id: string) => void;
    /** Chowa menu (zadokowane); w wysuniętym menu przycisku nie ma — przypina je „Przypnij”. */
    onhide?: () => void;
  }
  let { groups, current, query = $bindable(), hits, onopen, onhide }: Props = $props();

  const app = useApp();
  const { t } = app.i18n;
  const storage = typeof localStorage === 'undefined' ? null : localStorage;
  let collapsed = $state<string[]>(readCollapsed(storage));
  let expanded = $state(false);
  let scrolled = $state(false);
  let tree = $state<ReturnType<typeof SettingsNavTree> | null>(null);
  let field = $state<HTMLInputElement | null>(null);

  const searching = $derived(query.trim() !== '');
  const shown = $derived(
    searching
      ? groups
          .map((g) => ({ ...g, pages: g.pages.filter((p) => hits.has(p.id)) }))
          .filter((g) => g.pages.length > 0)
      : groups,
  );
  const allCollapsed = $derived(groups.length > 0 && groups.every((g) => collapsed.includes(g.id)));
  const currentPage = $derived(groups.flatMap((g) => g.pages).find((p) => p.id === current));

  function setCollapsed(ids: string[]) {
    collapsed = ids;
    writeCollapsed(storage, ids);
  }

  // Bieżąca strona nie może zniknąć w zwiniętej grupie: po zmianie strony grupa się rozwija,
  // a pozycja wjeżdża w pole widzenia listy.
  $effect(() => {
    const id = current;
    if (!id) return;
    untrack(() => {
      const group = groups.find((g) => g.pages.some((p) => p.id === id));
      if (group && collapsed.includes(group.id)) setCollapsed(toggled(collapsed, group.id));
    });
    requestAnimationFrame(() => tree?.reveal(id));
  });

  // Ctrl+F w Ustawieniach = szukanie ustawień (zamiast systemowego „Znajdź na stronie”).
  $effect(() => {
    const onkey = (event: KeyboardEvent) => {
      if (event.ctrlKey && !event.altKey && !event.shiftKey && event.key.toLowerCase() === 'f') {
        event.preventDefault();
        field?.focus();
        field?.select();
      }
    };
    window.addEventListener('keydown', onkey);
    return () => window.removeEventListener('keydown', onkey);
  });

  function searchKey(event: KeyboardEvent) {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      tree?.focusFirst();
    } else if (event.key === 'Enter') {
      const first = shown[0]?.pages[0]?.id;
      if (searching && first) choose(first);
    } else if (event.key === 'Escape' && query !== '') {
      // Pierwszy Escape czyści szukanie; dopiero następny zamyka Ustawienia.
      event.preventDefault();
      event.stopPropagation();
      query = '';
    }
  }

  function choose(id: string) {
    expanded = false;
    onopen(id);
  }
</script>

<aside class="nav" class:scrolled>
  <div class="head">
    <div class="top">
      <Button size="sm" variant="ghost" onclick={() => app.closeSettings()}>
        {#snippet icon()}<ArrowLeft size={14} strokeWidth={1.5} />{/snippet}
        {t('settings.back')}
      </Button>
    </div>
    <div class="title">
      <h1 class="h1">{t('settings.title')}</h1>
      <IconButton
        label={allCollapsed ? t('settings.nav.expandAll') : t('settings.nav.collapseAll')}
        size="sm"
        onclick={() => setCollapsed(allCollapsed ? [] : groups.map((g) => g.id))}
      >
        {#if allCollapsed}
          <ChevronsUpDown size={15} strokeWidth={1.5} />
        {:else}
          <ChevronsDownUp size={15} strokeWidth={1.5} />
        {/if}
      </IconButton>
      {#if onhide}
        <IconButton
          label={t('settings.nav.hide')}
          size="sm"
          onclick={onhide}
          aria-keyshortcuts="Control+B"
        >
          <PanelRightClose size={15} strokeWidth={1.5} />
        </IconButton>
      {/if}
    </div>
    <div class="search">
      <Search size={14} strokeWidth={1.5} aria-hidden="true" />
      <label class="alfa-visually-hidden" for="alfa-settings-search">{t('settings.search')}</label>
      <input
        id="alfa-settings-search"
        type="text"
        role="searchbox"
        autocomplete="off"
        spellcheck="false"
        placeholder={t('settings.searchPlaceholder')}
        aria-keyshortcuts="Control+F"
        bind:this={field}
        bind:value={query}
        onkeydown={searchKey}
      />
      {#if query !== ''}
        <button
          type="button"
          class="clear"
          aria-label={t('settings.nav.clear')}
          data-tip={t('settings.nav.clear')}
          onclick={() => {
            query = '';
            field?.focus();
          }}
        >
          <X size={14} strokeWidth={1.5} aria-hidden="true" />
        </button>
      {:else}
        <kbd class="hint" aria-hidden="true">Ctrl F</kbd>
      {/if}
    </div>
    {#if currentPage}
      {@const Icon = settingsIcon(currentPage.id)}
      <button
        type="button"
        class="current"
        aria-expanded={expanded || searching}
        aria-controls="settings-nav-list"
        aria-label={t('settings.nav.current', { page: app.i18n.text(currentPage.label) })}
        onclick={() => (expanded = !expanded)}
      >
        <span class="ico" aria-hidden="true"><Icon size={16} strokeWidth={1.5} /></span>
        <span class="label">{app.i18n.text(currentPage.label)}</span>
        <span class="chev" class:open={expanded || searching} aria-hidden="true">
          <ChevronDown size={14} strokeWidth={1.5} />
        </span>
      </button>
    {/if}
  </div>
  <SettingsNavTree
    bind:this={tree}
    groups={shown}
    {collapsed}
    {searching}
    {current}
    {hits}
    open={expanded || searching}
    onchoose={choose}
    ontoggle={(id) => setCollapsed(toggled(collapsed, id))}
    onscrolled={(value) => (scrolled = value)}
  />
  {#if app.appVersion}
    <p class="foot">{t('settings.nav.version', { version: app.appVersion })}</p>
  {/if}
</aside>

<style>
  .nav {
    display: flex;
    flex-direction: column;
    width: 272px;
    flex: none;
    min-height: 0;
    border-left: 1px solid var(--alfa-color-border);
    background: var(--alfa-color-surface);
  }
  .head {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-3) var(--alfa-space-3) var(--alfa-space-2);
    border-bottom: 1px solid transparent;
  }
  .scrolled .head {
    border-bottom-color: var(--alfa-color-border);
  }
  .top {
    display: flex;
  }
  .title {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-1);
    padding-left: var(--alfa-space-2);
  }
  .h1 {
    flex: 1;
  }
  .h1 {
    margin: 0;
    font-size: var(--alfa-font-size-xl);
  }
  .search {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    height: 34px;
    padding: 0 var(--alfa-space-1) 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text-muted);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: transparent;
    color: var(--alfa-color-text);
  }
  .search input:focus {
    outline: none;
  }
  .search:focus-within {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
    outline-offset: 1px;
  }
  .hint {
    padding: 1px var(--alfa-space-1);
    margin-right: var(--alfa-space-1);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-sm, 4px);
    color: var(--alfa-color-text-muted);
    font-family: inherit;
    font-size: var(--alfa-font-size-xs);
    white-space: nowrap;
  }
  .search:focus-within .hint {
    display: none;
  }
  .clear {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text-muted);
  }
  .clear:hover {
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text);
  }
  .current {
    display: none;
    align-items: center;
    gap: var(--alfa-space-2);
    width: 100%;
    min-height: 34px;
    padding: 0 var(--alfa-space-2) 0 var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
    text-align: left;
  }
  .current .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .current .ico,
  .chev {
    display: inline-flex;
    flex: none;
  }
  .chev {
    transform: rotate(-90deg);
    transition: transform var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .chev.open {
    transform: none;
  }
  .foot {
    margin: 0;
    padding: var(--alfa-space-2) var(--alfa-space-4);
    border-top: 1px solid var(--alfa-color-border);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  @media (max-width: 720px) {
    .nav {
      width: 100%;
      max-height: 70%;
      border-left: 0;
      border-bottom: 1px solid var(--alfa-color-border);
    }
    .head {
      display: grid;
      grid-template-columns: auto 1fr;
      align-items: center;
    }
    .title :global(.icon-btn) {
      display: none;
    }
    .search,
    .current {
      grid-column: 1 / -1;
    }
    .current {
      display: flex;
    }
    .foot {
      display: none;
    }
  }
</style>
