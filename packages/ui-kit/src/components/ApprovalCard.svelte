<script lang="ts">
  import ShieldAlert from '@lucide/svelte/icons/shield-alert';
  import Undo2 from '@lucide/svelte/icons/undo-2';
  import Avatar from './Avatar.svelte';
  import Button from './Button.svelte';
  import { type RiskLevel } from '../tokens';
  import { personInfo } from '../person-info';

  interface Props {
    agent: string;
    /** Co agentka chce zrobić. */
    what: string;
    /** Dlaczego. */
    why: string;
    reversible: boolean;
    risk: RiskLevel;
    /** Tylko wygląd: samo zatwierdzenie odbywa się w oknie Brokera (§8.2). */
    onopenbroker?: () => void;
    /** Stan prośby; po decyzji w Brokerze karta pokazuje wynik zamiast przycisku. */
    status?: 'pending' | 'approved' | 'denied' | 'expired';
    labels?: Partial<ApprovalLabels>;
  }

  interface ApprovalLabels {
    title: string;
    what: string;
    why: string;
    reversible: string;
    reversibleYes: string;
    reversibleNo: string;
    hint: string;
    open: string;
    risk: Record<RiskLevel, string>;
    status: Record<'approved' | 'denied' | 'expired', string>;
  }

  let {
    agent,
    what,
    why,
    reversible,
    risk,
    onopenbroker,
    status = 'pending',
    labels = {},
  }: Props = $props();

  const text = $derived<ApprovalLabels>({
    title: `${personInfo(agent).name} prosi o zatwierdzenie`,
    what: 'Co',
    why: 'Dlaczego',
    reversible: 'Cofalne',
    reversibleYes: 'tak — jednym kliknięciem',
    reversibleNo: 'nie — zmiana trwała',
    hint: 'Zatwierdzasz tylko w oknie Sędziego Cieni.',
    open: 'Otwórz w oknie Sędziego Cieni',
    risk: { low: 'niskie ryzyko', medium: 'średnie ryzyko', high: 'wysokie ryzyko' },
    status: {
      approved: 'Zatwierdzone w oknie Sędziego Cieni',
      denied: 'Odmówiono w oknie Sędziego Cieni',
      expired: 'Prośba wygasła',
    },
    ...labels,
  });
</script>

<article class="card risk-{risk}" aria-label={text.title}>
  <header class="head">
    <Avatar {agent} size={24} />
    <span class="who">{text.title}</span>
    <span class="risk">
      <ShieldAlert size={14} strokeWidth={1.5} aria-hidden="true" />
      {text.risk[risk]}
    </span>
  </header>
  <dl class="grid">
    <dt>{text.what}</dt>
    <dd>{what}</dd>
    <dt>{text.why}</dt>
    <dd>{why}</dd>
    <dt>{text.reversible}</dt>
    <dd class="rev">
      {#if reversible}
        <Undo2 size={14} strokeWidth={1.5} aria-hidden="true" /> {text.reversibleYes}
      {:else}
        {text.reversibleNo}
      {/if}
    </dd>
  </dl>
  <footer class="foot">
    {#if status === 'pending'}
      <span class="hint">{text.hint}</span>
      {#if onopenbroker}
        <Button variant="primary" size="sm" wrap onclick={onopenbroker}>{text.open}</Button>
      {/if}
    {:else}
      <span class="done" role="status">{text.status[status]}</span>
    {/if}
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
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
  }
  .who {
    flex: 1;
    min-width: min(100%, 12ch);
    overflow-wrap: anywhere;
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
  /* W wąskiej karcie przycisk schodzi pod podpowiedź (przy prawej krawędzi), zamiast z niej wystawać. */
  .foot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-2);
  }
  .foot > :global(.btn) {
    margin-inline-start: auto;
  }
  .hint {
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
  }
  .done {
    color: var(--alfa-color-text-muted);
    font-weight: var(--alfa-weight-semibold);
  }
  @media (max-width: 640px) {
    .foot > :global(.btn) {
      min-height: 44px;
    }
  }
</style>
