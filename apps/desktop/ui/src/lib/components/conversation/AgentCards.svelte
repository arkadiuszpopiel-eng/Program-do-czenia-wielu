<!--
  Karty kroków agentki pod wiadomością: intencje („uruchom w terminalu", trwałe usunięcie)
  i — po zakończeniu odpowiedzi — karta „Cofnij" z cofalnymi krokami.
-->
<script lang="ts">
  import type { Turn } from '../../api/types';
  import ToolIntentCard from './ToolIntentCard.svelte';
  import UndoCard from './UndoCard.svelte';

  interface Props {
    turn: Turn;
  }

  let { turn }: Props = $props();
  const withIntent = $derived(turn.tools.filter((s) => s.intent !== null));
</script>

<div class="cards">
  {#each withIntent as step (step.id)}
    {#if step.intent}<ToolIntentCard intent={step.intent} stepId={step.id} />{/if}
  {/each}
  {#if turn.status === 'complete'}<UndoCard steps={turn.tools} />{/if}
</div>

<style>
  .cards {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
  }
  .cards:empty {
    display: none;
  }
</style>
