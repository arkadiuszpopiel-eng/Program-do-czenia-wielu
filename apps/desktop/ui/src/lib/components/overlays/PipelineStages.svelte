<script lang="ts">
  import type { PipelineStage } from '../../api/types';
  import type { RoutingModel } from '../../api/types-hub';
  import { MAX_STAGES, moveStage } from '../../logic/pipelines';
  import PipelineStageEditor from './PipelineStageEditor.svelte';
  interface Props {
    stages: readonly PipelineStage[];
    models: readonly RoutingModel[];
    en: boolean;
    onchange: (stages: readonly PipelineStage[]) => void;
  }
  let { stages, models, en, onchange }: Props = $props();
  let active = $state(0);
  const index = $derived(Math.min(active, Math.max(0, stages.length - 1)));
  const stage = $derived(stages[index]);
  function duplicate() {
    if (!stage || stages.length >= MAX_STAGES) return;
    const next = [...stages];
    next.splice(index + 1, 0, { ...stage });
    onchange(next);
    active = index + 1;
  }
  function move(delta: -1 | 1) {
    onchange(moveStage(stages, index, delta));
    active = index + delta;
  }
</script>

<div class="workspace">
  <nav class="stage-nav" aria-label={en ? 'Pipeline stages' : 'Etapy pipeline’u'}>
    {#each stages as item, i (i)}
      <button
        type="button"
        class:active={index === i}
        aria-current={index === i ? 'step' : undefined}
        onclick={() => (active = i)}
      >
        <span class="number">{i + 1}</span>
        <span
          ><strong>{item.title || (en ? 'Untitled stage' : 'Etap bez nazwy')}</strong><small
            >{item.persona ?? (en ? 'Auto agent' : 'Agentka automatycznie')} · {item.model ??
              'Auto (Router)'}</small
          ></span
        >
      </button>
    {/each}
  </nav>
  <div class="editor-scroll">
    {#if stage}
      {#key index}
        <PipelineStageEditor
          {stage}
          {index}
          total={stages.length}
          {models}
          {en}
          onchange={(next) => onchange(stages.map((s, i) => (i === index ? next : s)))}
          onmove={move}
          onduplicate={duplicate}
          onremove={() => onchange(stages.filter((_, i) => i !== index))}
        />
      {/key}
      <p class="flow">
        {index === 0
          ? en
            ? 'Input: your task → this stage → the next stage.'
            : 'Wejście: Twoje zadanie → ten etap → kolejny etap.'
          : en
            ? 'Input: the previous stage output. The initial task is also available.'
            : 'Wejście: wynik poprzedniego etapu. Dostępna jest też pierwotna treść zadania.'}
      </p>
    {/if}
  </div>
</div>

<style>
  .workspace {
    display: grid;
    grid-template-columns: minmax(200px, 0.8fr) minmax(0, 2fr);
    flex: 1;
    min-height: 0;
    gap: var(--alfa-space-3);
  }
  /* Etapy połączone strzałkami przepływu: wynik jednego trafia do następnego. */
  .stage-nav {
    display: flex;
    flex-direction: column;
    gap: 22px;
    overflow: auto;
    min-width: 0;
    padding-bottom: 2px;
  }
  button {
    position: relative;
    display: flex;
    gap: var(--alfa-space-2);
    align-items: flex-start;
    width: 100%;
    min-height: 60px;
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text);
    text-align: left;
  }
  button:hover {
    background: var(--alfa-color-surface2);
  }
  button.active {
    border-color: var(--alfa-color-accent, var(--alfa-color-text));
    background: var(--alfa-color-surface2);
  }
  button:not(:last-child)::after {
    content: '↓';
    position: absolute;
    left: 50%;
    bottom: -20px;
    transform: translateX(-50%);
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-sm);
    line-height: 18px;
    pointer-events: none;
  }
  button.active .number {
    background: var(--alfa-accent-fill, var(--alfa-color-accent, var(--alfa-color-text)));
    color: var(--alfa-color-text-on-accent);
  }
  button:focus-visible {
    outline: 2px solid var(--alfa-color-focus);
    outline-offset: -2px;
  }
  button > span:last-child {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  strong {
    display: block;
    font-size: var(--alfa-font-size-sm);
  }
  small,
  .flow {
    display: block;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    margin-top: 4px;
  }
  .number {
    display: grid;
    place-items: center;
    flex: none;
    width: 24px;
    height: 24px;
    background: var(--alfa-color-surface2);
    border-radius: var(--alfa-radius-full);
  }
  .editor-scroll {
    min-width: 0;
    overflow: auto;
    padding: 0 4px 4px 0;
  }
  .flow {
    padding: var(--alfa-space-2);
  }
  @media (max-width: 1000px) {
    .workspace {
      display: flex;
      flex-direction: column;
    }
    .stage-nav {
      flex-direction: row;
      flex: none;
      max-height: 94px;
    }
    button:not(:last-child)::after {
      content: '→';
      left: auto;
      right: -18px;
      bottom: auto;
      top: 50%;
      transform: translateY(-50%);
    }
    button {
      width: 180px;
      flex: none;
    }
    .editor-scroll {
      flex: 1;
    }
  }
</style>
