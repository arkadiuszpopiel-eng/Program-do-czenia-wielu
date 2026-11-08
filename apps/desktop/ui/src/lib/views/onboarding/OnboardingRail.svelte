<!--
  Wprowadzenie → boczna szyna: obsada, tytuł, lista kroków (pionowo, z ikoną i stanem) i pasek
  postępu. Do kroków już odwiedzonych można wrócić kliknięciem; dalsze prowadzi „Dalej”.
-->
<script lang="ts">
  import { Avatar, agentIds } from '@alfa/ui-kit';
  import StepIcon from './StepIcon.svelte';

  interface Step {
    readonly id: string;
    readonly label: string;
    readonly optional: boolean;
  }

  interface Props {
    steps: readonly Step[];
    current: number;
    title: string;
    tagline: string;
    /** Nazwa listy kroków dla czytników ekranu. */
    label: string;
    optionalLabel: string;
    progressLabel: string;
    onselect: (index: number) => void;
  }

  let { steps, current, title, tagline, label, optionalLabel, progressLabel, onselect }: Props =
    $props();
  const percent = $derived(Math.round(((current + 1) / steps.length) * 100));
</script>

<aside class="rail">
  <div class="brand">
    <div class="cast" aria-hidden="true">
      {#each agentIds as id (id)}<span class="face"><Avatar agent={id} size={32} /></span>{/each}
    </div>
    <h1 id="ob-title">{title}</h1>
    <p class="tagline">{tagline}</p>
  </div>

  <ol class="steps" aria-label={label}>
    {#each steps as step, i (step.id)}
      <li class:done={i < current} class:current={i === current}>
        {#snippet content()}
          <span class="mark" aria-hidden="true">
            {#if i < current}
              <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="3"
                stroke-linecap="round"
                stroke-linejoin="round"><path d="M5 12l5 5 9-10" /></svg
              >
            {:else}
              <StepIcon step={step.id} size={16} />
            {/if}
          </span>
          <span class="text">
            <span class="name">{step.label}</span>
            {#if step.optional}<span class="opt">{optionalLabel}</span>{/if}
          </span>
        {/snippet}
        {#if i < current}
          <button type="button" class="step" onclick={() => onselect(i)}>{@render content()}</button
          >
        {:else}
          <span class="step" aria-current={i === current ? 'step' : undefined}
            >{@render content()}</span
          >
        {/if}
      </li>
    {/each}
  </ol>

  <div
    class="progress"
    role="progressbar"
    aria-label={progressLabel}
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={percent}
  >
    <span class="bar" style:transform="scaleX({percent / 100})"></span>
  </div>
</aside>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-6);
    min-height: 0;
    padding: var(--alfa-space-6);
    border-right: 1px solid var(--alfa-color-border);
    background: var(--alfa-color-surface2);
    overflow: auto;
  }
  .brand {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
  }
  .cast {
    display: flex;
    margin-bottom: var(--alfa-space-2);
  }
  .face {
    display: inline-flex;
    margin-right: calc(-1 * var(--alfa-space-2));
    border: 2px solid var(--alfa-color-surface2);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface2);
  }
  h1 {
    margin: 0;
    font-size: var(--alfa-font-size-2xl);
    line-height: 1.2;
  }
  .tagline {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
    line-height: 1.5;
  }
  .steps {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .step {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-3);
    width: 100%;
    min-height: 40px;
    padding: var(--alfa-space-1) var(--alfa-space-2);
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-md);
    text-align: left;
  }
  button.step:hover {
    background: var(--alfa-color-surface3);
    color: var(--alfa-color-text);
  }
  .mark {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface);
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.25;
  }
  .opt {
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
  }
  .current .step {
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
    font-weight: var(--alfa-weight-semibold);
    box-shadow: var(--alfa-shadow-1);
  }
  .current .opt {
    font-weight: var(--alfa-weight-regular);
  }
  .current .mark {
    border-color: var(--alfa-color-text);
    background: var(--alfa-color-text);
    color: var(--alfa-color-text-on-accent);
  }
  .done .mark {
    border-color: var(--alfa-color-success);
    color: var(--alfa-color-success);
  }
  .progress {
    position: relative;
    flex: none;
    height: 4px;
    overflow: hidden;
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface3);
  }
  .bar {
    position: absolute;
    inset: 0;
    background: var(--alfa-color-success);
    transform-origin: left;
    transition: transform 200ms ease;
  }
  :global([data-motion='off']) .bar {
    transition: none;
  }
  /* Wąskie okno: szyna staje się nagłówkiem, kroki idą w jednym rzędzie bez opisów. */
  @media (max-width: 760px) {
    .rail {
      gap: var(--alfa-space-3);
      padding: var(--alfa-space-4);
      border-right: 0;
      border-bottom: 1px solid var(--alfa-color-border);
      overflow: visible;
    }
    .tagline {
      display: none;
    }
    /* Nazwa kroku zostaje dla czytników ekranu — znika tylko z widoku. */
    .text {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }
    .steps {
      flex-direction: row;
      flex-wrap: wrap;
      gap: var(--alfa-space-1);
    }
    .step {
      width: auto;
      min-height: 32px;
      padding: 2px;
    }
  }
</style>
