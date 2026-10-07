<script lang="ts">
  import Square from '@lucide/svelte/icons/square';
  import Avatar from './Avatar.svelte';
  import Button from './Button.svelte';
  import { agents, type AgentId } from '../tokens';

  interface Props {
    agent: AgentId;
    /** Opis bieżącego kroku, np. „edytuję raport.docx". */
    description: string;
    step: number;
    totalSteps: number;
    elapsedSeconds: number;
    onstop?: () => void;
    labels?: Partial<{ step: string; stop: string; stopLabel: string; progress: string }>;
  }

  let {
    agent,
    description,
    step,
    totalSteps,
    elapsedSeconds,
    onstop,
    labels = {},
  }: Props = $props();

  const name = $derived(agents[agent].name);
  const pct = $derived(totalSteps > 0 ? Math.min(100, Math.round((step / totalSteps) * 100)) : 0);
  const elapsed = $derived(
    `${Math.floor(elapsedSeconds / 60)}:${String(elapsedSeconds % 60).padStart(2, '0')}`,
  );
  const text = $derived({
    step: `krok ${step}/${totalSteps}`,
    stop: 'Stop',
    stopLabel: `Zatrzymaj zadanie ${name}`,
    progress: 'Postęp zadania',
    ...labels,
  });
</script>

<div
  class="capsule"
  style:--accent="var(--alfa-agent-{agent})"
  role="status"
  aria-live="polite"
  aria-label="{name} · {description} · {text.step} · {elapsed}"
>
  <Avatar {agent} size={24} working />
  <div class="text">
    <div class="line">
      <span class="name">{name}</span>
      <span class="sep" aria-hidden="true">·</span>
      <span class="desc">{description}</span>
    </div>
    <div class="line meta">
      <span>{text.step}</span>
      <span
        class="bar"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={pct}
        aria-label={text.progress}
      >
        <span class="fill" style:transform="scaleX({pct / 100})"></span>
      </span>
      <span class="time">{elapsed}</span>
    </div>
  </div>
  {#if onstop}
    <Button size="sm" variant="secondary" onclick={onstop} aria-label={text.stopLabel}>
      {#snippet icon()}<Square size={12} strokeWidth={2} />{/snippet}
      {text.stop}
    </Button>
  {/if}
</div>

<style>
  .capsule {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-3);
    max-width: 100%;
    padding: var(--alfa-space-2) var(--alfa-space-2) var(--alfa-space-2) var(--alfa-space-3);
    border: 1px solid color-mix(in srgb, var(--accent) 35%, var(--alfa-color-border));
    border-radius: var(--alfa-radius-overlay);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-2);
    font-size: var(--alfa-font-size-sm);
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    min-width: 0;
  }
  .name {
    color: var(--accent);
    font-weight: var(--alfa-weight-semibold);
  }
  .sep {
    color: var(--alfa-color-text-subtle);
  }
  .desc {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    font-variant-numeric: tabular-nums;
  }
  .bar {
    flex: 1;
    max-width: 120px;
    height: 4px;
    border-radius: 2px;
    background: var(--alfa-color-surface3);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    background: var(--accent);
    transform-origin: left;
    transition: transform var(--alfa-duration-panel) var(--alfa-ease-out);
  }
</style>
