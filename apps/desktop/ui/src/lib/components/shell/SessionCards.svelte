<!--
  Widok „Karty” panelu Sesje (P4): te same grupy co lista (przypięte, projekty, bez projektu,
  archiwum), karty w siatce — w wąskim panelu jedna kolumna, w szerokim kilka. Dane profilu
  (agentki, ostatnia wiadomość, model, koszt) przychodzą komendą `sessions_cards` tylko dla rozmów
  nowych albo zmienionych (`updated_at`), partiami po 200 — rdzeń nie czyta wtedy historii wszystkich
  rozmów po każdej wiadomości. Błąd pokazuje komunikat z „Ponów”, a karty zostają z danymi z listy.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import type { SessionCard as CardData } from '../../api/types';
  import { MAX_CARDS, columnsOf, inBatches, moveInGroups } from '../../logic/session-cards';
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
  /** `updated_at` rozmów, dla których karta jest pobrana (bez reaktywności — tylko porównanie). */
  let fetched: Record<string, string> = {};
  /** Numer ostatniego zapytania — spóźniona odpowiedź na starą listę jest pomijana. */
  let latest = 0;

  const sessionsNow = $derived(groups.flatMap((g) => g.sessions));
  // Zmiana kolejności, nowa wiadomość albo nowa rozmowa — odświeżenie zmienionych kart.
  const stamp = $derived(sessionsNow.map((s) => `${s.id}@${s.updated_at}`).join('|'));

  /** Rozmowy bez karty albo zmienione od ostatniego pobrania. */
  function stale(): { id: string; updated: string }[] {
    const seen: Record<string, true> = {};
    const out: { id: string; updated: string }[] = [];
    for (const s of untrack(() => sessionsNow)) {
      if (seen[s.id]) continue;
      seen[s.id] = true;
      if (fetched[s.id] !== s.updated_at) out.push({ id: s.id, updated: s.updated_at });
    }
    return out;
  }

  async function fetchCards() {
    const todo = stale();
    if (!todo.length) return;
    const token = ++latest;
    const next: Record<string, CardData> = {};
    const done: { id: string; updated: string }[] = [];
    for (const batch of inBatches(todo, MAX_CARDS)) {
      const result = await load(() => app.client.sessions.cards(batch.map((b) => b.id)));
      if (token !== latest) return;
      if (result.status !== 'ready') {
        if (result.status === 'failed') failed = result.error;
        break;
      }
      for (const card of result.value) next[card.session_id] = card;
      done.push(...batch);
    }
    if (token !== latest) return;
    if (done.length === todo.length) failed = null;
    for (const item of done) fetched[item.id] = item.updated;
    if (done.length) cards = { ...cards, ...next };
  }

  function retry() {
    fetched = {};
    void fetchCards();
  }

  $effect(() => {
    void stamp;
    const handle = setTimeout(() => void fetchCards(), 120);
    return () => clearTimeout(handle);
  });

  /** Strzałki, Home i End: kolumny liczone z kart grupy, w której jest fokus; ruch w grupie. */
  function move(event: KeyboardEvent) {
    const lists = Array.from(root?.querySelectorAll<HTMLElement>('ul.grid') ?? []);
    const perGroup = lists.map((ul) =>
      Array.from(ul.querySelectorAll<HTMLElement>('[data-session-card]')),
    );
    const all = perGroup.flat();
    const current = event.currentTarget as HTMLElement;
    const at = all.indexOf(current);
    if (at < 0) return;
    const group = perGroup.find((cards) => cards.includes(current)) ?? [];
    const columns = columnsOf(group.map((el) => el.getBoundingClientRect().top));
    const next = moveInGroups(
      perGroup.map((cards) => cards.length),
      at,
      event.key,
      columns,
    );
    if (next === null) return;
    event.preventDefault();
    all[next]?.focus();
    all[next]?.scrollIntoView({ block: 'nearest' });
  }
</script>

<div class="cards" bind:this={root}>
  {#if failed}
    <LoadFailed error={failed} onretry={retry} />
  {/if}
  {#each groups as group (group.id)}
    <section class="group" aria-labelledby="card-grp-{group.id}">
      <h3 class="group-title" id="card-grp-{group.id}">{title(group)}</h3>
      <ul class="grid" aria-label={t('sessions.cards')}>
        {#each group.sessions as session (session.id)}
          <SessionCard
            {session}
            card={cards[session.id]}
            showProject={group.kind === 'pinned' || group.kind === 'archived'}
            onmove={move}
          />
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
