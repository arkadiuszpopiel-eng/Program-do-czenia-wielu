<!--
  Treść jednej rozmowy w dzienniku rodzica (Z.7) — tylko gdy rodzic włączył ją PIN-em (przełącznik
  w Sędzim Cieni; dziecko widzi wtedy stały znak w belce). Zwykły tekst tur (bez HTML), najwyżej
  ostatnie 40 tur, każda skrócona; wszystkie gałęzie w kolejności czasu.
-->
<script lang="ts">
  import type { Turn } from '../../../api/types';
  import { useApp } from '../../../state/context';

  const TURNS = 40;
  const CHARS = 600;
  let { sessionId }: { sessionId: string } = $props();
  const app = useApp();
  const { t } = app.i18n;
  let turns = $state<Turn[] | null>(null);
  let failed = $state(false);

  $effect(() => {
    const id = sessionId;
    void app.client.turns.list(id).then(
      (snap) => {
        turns = [...snap.turns]
          .filter((x) => x.text.trim() !== '')
          .sort((a, b) => a.created_at.localeCompare(b.created_at))
          .slice(-TURNS);
      },
      () => (failed = true),
    );
  });

  const clip = (text: string) => (text.length > CHARS ? `${text.slice(0, CHARS)}…` : text);
</script>

{#if failed}
  <p class="desc" role="alert">{t('parentLog.failed')}</p>
{:else if turns === null}
  <p class="desc" role="status">{t('parentLog.loading')}</p>
{:else if turns.length === 0}
  <p class="desc">{t('parentLog.noText')}</p>
{:else}
  <ol class="turns" aria-label={t('parentLog.transcript')}>
    {#each turns as turn (turn.id)}
      <li>
        <time datetime={turn.created_at}>{app.i18n.time(turn.created_at)}</time>
        <span class="who">{turn.author === 'user' ? t('parentLog.child') : turn.author}</span>
        <p>{clip(turn.text)}</p>
      </li>
    {/each}
  </ol>
{/if}

<style>
  .turns {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: 0;
    padding: 0 0 0 var(--alfa-space-3);
    list-style: none;
    font-size: var(--alfa-font-size-sm);
  }
  .turns li {
    display: flex;
    flex-wrap: wrap;
    gap: 0 var(--alfa-space-2);
  }
  .turns p {
    flex-basis: 100%;
    margin: 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  time,
  .who {
    color: var(--alfa-color-text-muted);
    font-variant-numeric: tabular-nums;
  }
  .who {
    text-transform: capitalize;
  }
  .desc {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
</style>
