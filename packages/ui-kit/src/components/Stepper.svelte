<script lang="ts">
  interface Step {
    readonly id: string;
    readonly label: string;
  }

  interface Props {
    steps: readonly Step[];
    current: number;
    label: string;
    /** Pozwala przejść do wcześniejszego kroku. */
    onselect?: (index: number) => void;
  }

  let { steps, current, label, onselect }: Props = $props();
</script>

<ol class="stepper" aria-label={label}>
  {#each steps as step, i (step.id)}
    <li class:done={i < current} class:current={i === current}>
      {#if onselect && i < current}
        <button type="button" class="step" onclick={() => onselect(i)}>
          <span class="dot" aria-hidden="true">{i + 1}</span>
          <span class="label">{step.label}</span>
        </button>
      {:else}
        <span class="step" aria-current={i === current ? 'step' : undefined}>
          <span class="dot" aria-hidden="true">{i + 1}</span>
          <span class="label">{step.label}</span>
        </span>
      {/if}
    </li>
  {/each}
</ol>

<style>
  .stepper {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-1) var(--alfa-space-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .step {
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-2);
    min-height: 28px;
    padding: 0 var(--alfa-space-1);
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  button.step:hover {
    background: var(--alfa-color-surface2);
  }
  .dot {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-full);
    font-size: var(--alfa-font-size-xs);
    font-variant-numeric: tabular-nums;
  }
  .current .step {
    color: var(--alfa-color-text);
    font-weight: var(--alfa-weight-semibold);
  }
  .current .dot {
    border-color: var(--alfa-color-text);
    background: var(--alfa-color-text);
    color: var(--alfa-color-bg);
  }
  .done .dot {
    border-color: var(--alfa-color-success);
    color: var(--alfa-color-success);
  }
</style>
