<script lang="ts">
  import ShieldAlert from '@lucide/svelte/icons/shield-alert';
  import Undo2 from '@lucide/svelte/icons/undo-2';
  import Avatar from './Avatar.svelte';
  import Button from './Button.svelte';
  import { agents, type AgentId, type RiskLevel } from '../tokens';

  interface Props {
    agent: AgentId;
    /** Co agentka chce zrobić. */
    what: string;
    /** Dlaczego. */
    why: string;
    reversible: boolean;
    risk: RiskLevel;
    /** Tylko wygląd: samo zatwierdzenie odbywa się w oknie Brokera (§8.2). */
    onopenbroker?: () => void;
  }

  let { agent, what, why, reversible, risk, onopenbroker }: Props = $props();
  const riskLabel: Record<RiskLevel, string> = {
    low: 'niskie ryzyko',
    medium: 'średnie ryzyko',
    high: 'wysokie ryzyko',
  };
</script>

<article
  class="card risk-{risk}"
  aria-label="Prośba o zatwierdzenie od agentki {agents[agent].name}"
>
  <header class="head">
    <Avatar {agent} size={24} />
    <span class="who">{agents[agent].name} prosi o zatwierdzenie</span>
    <span class="risk">
      <ShieldAlert size={14} strokeWidth={1.5} aria-hidden="true" />
      {riskLabel[risk]}
    </span>
  </header>
  <dl class="grid">
    <dt>Co</dt>
    <dd>{what}</dd>
    <dt>Dlaczego</dt>
    <dd>{why}</dd>
    <dt>Cofalne</dt>
    <dd class="rev">
      {#if reversible}
        <Undo2 size={14} strokeWidth={1.5} aria-hidden="true" /> tak — jednym kliknięciem
      {:else}
        nie — zmiana trwała
      {/if}
    </dd>
  </dl>
  <footer class="foot">
    <span class="hint">Zatwierdzasz tylko w oknie Brokera.</span>
    <Button variant="primary" size="sm" onclick={onopenbroker}>Otwórz w oknie Brokera</Button>
  </footer>
</article>

<style>
  .card {
    --risk: var(--alfa-color-risk-low);
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    padding: var(--alfa-space-3) var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-left: 3px solid var(--risk);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-1);
    font-size: var(--alfa-font-size-sm);
  }
  .risk-medium {
    --risk: var(--alfa-color-risk-medium);
  }
  .risk-high {
    --risk: var(--alfa-color-risk-high);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
  }
  .who {
    flex: 1;
    font-weight: var(--alfa-weight-semibold);
  }
  .risk {
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-1);
    color: var(--risk);
    font-weight: var(--alfa-weight-semibold);
    font-size: var(--alfa-font-size-xs);
  }
  .grid {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--alfa-space-1) var(--alfa-space-3);
    margin: 0;
  }
  dt {
    color: var(--alfa-color-text-muted);
  }
  dd {
    margin: 0;
  }
  .rev {
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-1);
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-2);
  }
  .hint {
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
  }
</style>
