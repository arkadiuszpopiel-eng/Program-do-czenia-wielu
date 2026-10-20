<!--
  Widok „Karty” panelu Sesje (P4): te same grupy co lista (przypięte, projekty, bez projektu,
  archiwum), karty w siatce — w wąskim panelu jedna kolumna, w szerokim kilka. Dane profilu
  (agentki, ostatnia wiadomość, model, koszt) przychodzą jedną komendą `sessions_cards` po każdej
  zmianie listy; błąd pokazuje komunikat z „Ponów”, a karty zostają z danymi z listy.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import type { SessionCard as CardData } from '../../api/types';
  import { columnsOf, moveInGrid } from '../../logic/session-cards';
  import { load } from '../../state/attempt';
  import { useApp } from '../../state/context';
  import type { SessionGroup } from '../../state/sessions.svelte';
  import LoadFailed from './LoadFailed.svelte';
  import SessionCard from './SessionCard.svelte';

  interface Props {
    groups: readonly SessionGroup[];
    title: (group: SessionGroup) => string;
  }

  let { groups, title }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  let cards = $state<Record<string, CardData>>({});
  let failed = $state<string | null>(null);
  let root = $state<HTMLElement | null>(null);
  /** Numer ostatniego zapytania — spóźniona odpowiedź na starą listę jest pomijana. */
  let latest = 0;

  const ids = $derived(groups.flatMap((g) => g.sessions.map((s) => s.id)));
  // Zmiana kolejności, nowa wiadomość albo nowa rozmowa — odświeżenie kart.
  const stamp = $derived(
    groups.flatMap((g) => g.sessions.map((s) => `${s.id}@${s.updated_at}`)).join('|'),
  );

  async function fetchCards(list: readonly string[]) {
    const token = ++latest;
    const result = await load(() => app.client.sessions.cards(list));
    if (token !== latest) return;
    if (result.status === 'failed') {
      failed = result.error;
      return;
    }
    failed = null;
    const next: Record<string, CardData> = {};
    for (const card of result.value) next[card.session_id] = card;
    cards = next;
  }

  $effect(() => {
    void stamp;
    const list = untrack(() => ids);
    if (!list.length) return;
    const handle = setTimeout(() => void fetchCards(list), 120);
    return () => clearTimeout(handle);
  });

  /** Strzałki, Home i End: kolumny liczone z kart grupy, w której jest fokus. */
  function move(event: KeyboardEvent) {
    const all = Array.from(root?.querySelectorAll<HTMLElement>('[data-session-card]') ?? []);
    const current = event.currentTarget as HTMLElement;
    const at = all.indexOf(current);
    const group = Array.from(
      current.closest('ul')?.querySelectorAll<HTMLElement>('[data-session-card]') ?? [],
    );
    const columns = columnsOf(group.map((el) => el.getBoundingClientRect().top));
    const next = moveInGrid(at, event.key, columns, all.length);
    if (next === null || at < 0) return;
    event.preventDefault();
    all[next]?.focus();
    all[next]?.scrollIntoView({ block: 'nearest' });
  }
</script>

<div class="cards" bind:this={root}>
  {#if failed}
    <LoadFailed error={failed} onretry={() => void fetchCards(ids)} />
  {/if}
  {#each groups as group (group.id)}
    <section class="group" aria-labelledby="card-grp-{group.id}">
      <h3 class="group-title" id="card-grp-{group.id}">{title(group)}</h3>
      <ul class="grid" aria-label={t('sessions.cards')}>
        {#each group.sessions as session (session.id)}
          <SessionCard {session} card={cards[session.id]} onmove={move} />
        {/each}
      </ul>
    </section>
  {/each}
</div>

<style>
  .cards {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
  }
  .group-title {
    padding: var(--alfa-space-1) var(--alfa-space-2);
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 200px), 1fr));
    gap: var(--alfa-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
</style>
