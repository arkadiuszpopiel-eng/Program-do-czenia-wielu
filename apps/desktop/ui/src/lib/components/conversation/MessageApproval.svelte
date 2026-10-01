<!-- Karta „czeka na zatwierdzenie" w wątku: tylko przenosi do okna Brokera (PLAN §8.2). -->
<script lang="ts">
  import { ApprovalCard, agents, type AgentId } from '@alfa/ui-kit';
  import type { ApprovalPending } from '../../api/types';
  import { useApp } from '../../state/context';

  interface Props {
    agent: AgentId;
    approval: ApprovalPending;
  }

  let { agent, approval }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;

  async function openBroker() {
    try {
      await app.client.permissions.openApproval(approval.id);
      app.toasts.show({ kind: 'info', message: t('msg.brokerOpened') });
    } catch (error) {
      app.toasts.show({
        kind: 'warning',
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  const expires = $derived(approval.expires_at ? app.i18n.time(approval.expires_at) : null);
</script>

<ApprovalCard
  {agent}
  what={approval.what}
  why={approval.why}
  reversible={approval.reversible}
  risk={approval.risk}
  status={approval.status}
  onopenbroker={openBroker}
  labels={{
    title: t('approval.title', { name: agents[agent].name }),
    what: t('approval.what'),
    why: t('approval.why'),
    reversible: t('approval.reversible'),
    reversibleYes: t('approval.reversibleYes'),
    reversibleNo: t('approval.reversibleNo'),
    hint: approval.broker_window ? t('approval.hint') : t('approval.noWindow'),
    open: t('approval.open'),
    risk: {
      low: t('approval.risk.low'),
      medium: t('approval.risk.medium'),
      high: t('approval.risk.high'),
    },
    status: {
      approved: t('approval.status.approved'),
      denied: t('approval.status.denied'),
      expired: t('approval.status.expired'),
    },
  }}
/>

{#if !approval.broker_window && approval.status === 'pending'}
  <p class="dev" role="note">
    {expires ? t('approval.noWindowExpires', { time: expires }) : t('approval.noWindowDeny')}
  </p>
{/if}

<style>
  .dev {
    margin-top: var(--alfa-space-1);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
</style>
