<script lang="ts">
  import { agents } from '../tokens';
  import type { CaptionLine } from '../types';

  interface Props {
    lines: readonly CaptionLine[];
  }

  let { lines }: Props = $props();
  const who = (l: CaptionLine) => (l.speaker === 'user' ? 'Ty' : agents[l.speaker].name);
</script>

<div class="captions" aria-live="polite" aria-atomic="false" aria-label="Napisy na żywo">
  {#each lines as line, i (i)}
    {@const cut = line.interruptedAt ?? line.text.length}
    {@const spoken = Math.min(line.spokenChars, cut)}
    <p
      class="line"
      class:user={line.speaker === 'user'}
      class:partial={line.partial}
      style:--accent={line.speaker === 'user'
        ? 'var(--alfa-color-text)'
        : `var(--alfa-agent-${line.speaker})`}
    >
      <span class="who">{who(line)}</span>
      <span class="spoken">{line.text.slice(0, spoken)}</span><span class="pending"
        >{line.text.slice(spoken, cut)}</span
      >
      {#if line.interruptedAt !== undefined}
        <span class="cut" role="note">przerwano tutaj</span>
        <span class="dropped" aria-label="Niewypowiedziana reszta">{line.text.slice(cut)}</span>
      {/if}
      {#if line.partial}<span class="alfa-visually-hidden">(tekst częściowy)</span>{/if}
    </p>
  {/each}
</div>

<style>
  .captions {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    max-width: 60ch;
    font-size: var(--alfa-font-size-lg);
    line-height: var(--alfa-leading-text);
  }
  .line {
    color: var(--alfa-color-text-muted);
  }
  .who {
    display: block;
    color: var(--accent);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .spoken {
    color: var(--alfa-color-text);
  }
  .user .spoken {
    color: var(--alfa-color-text);
  }
  .partial .pending,
  .user .pending {
    color: var(--alfa-color-text-subtle);
  }
  .cut {
    display: inline-block;
    margin: 0 var(--alfa-space-1);
    padding: 0 6px;
    border: 1px solid var(--alfa-color-warning);
    border-radius: var(--alfa-radius-full);
    color: var(--alfa-color-warning);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
    vertical-align: middle;
    white-space: nowrap;
  }
  .dropped {
    color: var(--alfa-color-text-subtle);
    text-decoration: line-through;
    text-decoration-color: color-mix(in srgb, currentColor 40%, transparent);
  }
</style>
