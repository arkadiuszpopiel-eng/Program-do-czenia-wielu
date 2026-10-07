<script lang="ts">
  import Sparkles from '@lucide/svelte/icons/sparkles';
  import Composer from '../components/Composer.svelte';
  import MicButton from '../components/MicButton.svelte';
  import Chip from '../components/Chip.svelte';
  import Avatar from '../components/Avatar.svelte';
  import { agentIds } from '../tokens';
  import type { MicState } from '../types';

  interface Suggestion {
    readonly title: string;
    readonly text: string;
  }

  interface Props {
    suggestions: readonly Suggestion[];
    micState?: MicState;
    onsubmit?: (text: string) => void;
  }

  let { suggestions, micState = 'off', onsubmit }: Props = $props();
  let draft = $state('');
</script>

<main class="start">
  <div class="hero">
    <div class="cast" aria-label="Twoje agentki">
      {#each agentIds as id (id)}<Avatar agent={id} size={32} />{/each}
    </div>
    <h1 class="title">Co robimy?</h1>
    <p class="lead">
      Napisz albo powiedz. Agentki podzielą się pracą i zapytają, zanim zrobią coś, czego nie da się
      cofnąć.
    </p>
  </div>

  <ul class="suggestions" aria-label="Sugestie na start">
    {#each suggestions as s (s.title)}
      <li>
        <button type="button" class="card" onclick={() => (draft = s.text)}>
          <Sparkles size={16} strokeWidth={1.5} aria-hidden="true" />
          <span class="card-title">{s.title}</span>
          <span class="card-text">{s.text}</span>
        </button>
      </li>
    {/each}
  </ul>

  <div class="composer">
    <Composer bind:value={draft} {onsubmit} onattach={() => {}}>
      {#snippet chips()}
        <Chip agent="alfa" size="sm" onclick={() => {}}>Alfa</Chip>
        <Chip size="sm" onclick={() => {}}>Hybryda</Chip>
      {/snippet}
      {#snippet trailing()}
        <MicButton state={micState} showLabel={false} />
      {/snippet}
    </Composer>
    <p class="hint">Enter — wyślij · Shift+Enter — nowa linia · Ctrl+K — paleta poleceń</p>
  </div>
</main>

<style>
  .start {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--alfa-space-8);
    min-height: 100%;
    padding: var(--alfa-space-8) var(--alfa-space-4);
    background: var(--alfa-color-bg);
  }
  .hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--alfa-space-3);
    text-align: center;
    max-width: 52ch;
  }
  .cast {
    display: flex;
    gap: var(--alfa-space-2);
  }
  .title {
    font-size: var(--alfa-font-size-3xl);
  }
  .lead {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-lg);
  }
  .suggestions {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: var(--alfa-space-3);
    width: min(760px, 100%);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .card {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--alfa-space-1);
    width: 100%;
    height: 100%;
    padding: var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text-muted);
    box-shadow: var(--alfa-shadow-1);
    text-align: left;
    transition: transform var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .card:hover {
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text);
  }
  .card:active {
    transform: scale(0.99);
  }
  .card-title {
    color: var(--alfa-color-text);
    font-weight: var(--alfa-weight-semibold);
  }
  .card-text {
    font-size: var(--alfa-font-size-sm);
  }
  .composer {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    width: min(760px, 100%);
  }
  .hint {
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
    text-align: center;
  }
</style>
