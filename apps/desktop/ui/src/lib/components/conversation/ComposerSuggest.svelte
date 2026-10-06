<!--
  Podpowiedzi composera (@agentka, /komenda) jako listbox powiązany z polem (aria-activedescendant).
  Wybór wskaźnikiem nie zabiera fokusu z pola (preventDefault na pointerdown).
-->
<script lang="ts" module>
  import type { AgentId } from '@alfa/ui-kit';

  export interface Suggestion {
    readonly id: string;
    readonly label: string;
    readonly hint: string;
    readonly insert: string;
    readonly agent?: AgentId;
  }
</script>

<script lang="ts">
  interface Props {
    suggestions: readonly Suggestion[];
    active: number;
    listId: string;
    label: string;
    onaccept: (s: Suggestion) => void;
  }

  let { suggestions, active, listId, label, onaccept }: Props = $props();
</script>

<ul class="suggest" role="listbox" id={listId} aria-label={label}>
  {#each suggestions as s, i (s.id)}
    <li
      id="{listId}-{i}"
      role="option"
      aria-selected={i === active}
      class:active={i === active}
      onpointerdown={(e) => {
        e.preventDefault();
        onaccept(s);
      }}
    >
      <span class="s-label" style:color={s.agent ? `var(--alfa-agent-${s.agent})` : undefined}
        >{s.label}</span
      >
      <span class="s-hint">{s.hint}</span>
    </li>
  {/each}
</ul>

<style>
  .suggest {
    position: absolute;
    left: 0;
    right: 0;
    bottom: calc(100% + 4px);
    z-index: 20;
    max-height: 260px;
    margin: 0;
    padding: var(--alfa-space-1);
    overflow: auto;
    list-style: none;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-3);
  }
  .suggest li {
    display: flex;
    align-items: baseline;
    gap: var(--alfa-space-3);
    min-height: 32px;
    padding: var(--alfa-space-1) var(--alfa-space-3);
    border-radius: var(--alfa-radius-control);
    font-size: var(--alfa-font-size-sm);
  }
  .suggest li.active {
    background: var(--alfa-color-surface3);
  }
  .s-label {
    font-weight: var(--alfa-weight-semibold);
  }
  .s-hint {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
</style>
