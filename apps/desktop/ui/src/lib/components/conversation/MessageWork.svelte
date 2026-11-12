<!-- Rzadkie szczegóły pracy: zgoda, karty narzędzi, kontynuacja i błąd; widoki nadal leniwe. -->
<script lang="ts">
  import type { Turn } from '../../api/types';
  import type { ConversationState } from '../../state/conversation.svelte';
  import { useApp } from '../../state/context';
  import { attempt } from '../../state/attempt';
  import { isContinued } from '../../logic/turn-tree';
  import Lazy from '../shell/Lazy.svelte';
  let { turn, conv }: { turn: Turn; conv: ConversationState } = $props();
  const app = useApp();
  const { t } = app.i18n;
  const agent = $derived(turn.author === 'user' ? null : turn.author);
  const act = (action: () => Promise<unknown>) => void attempt(app.toasts, action);
  const loadApproval = () => import('./MessageApproval.svelte');
  const loadError = () => import('./MessageError.svelte');
  const loadAgentCards = () => import('./AgentCards.svelte');
  const loadContinuation = () => import('./ContinuationCard.svelte');
</script>

{#if turn.approval && agent}
  <div class="indent">
    <Lazy
      load={loadApproval}
      label={t('common.loading')}
      props={{ agent, approval: turn.approval }}
    />
  </div>
{/if}

{#if agent && turn.tools.length}
  <div class="indent">
    <Lazy load={loadAgentCards} props={{ turn }} label={t('common.loading')} />
  </div>
{/if}

{#if turn.error?.code === 'task_paused' && !isContinued(conv.tree, turn)}
  <Lazy
    load={loadContinuation}
    label={t('common.loading')}
    props={{ oncontinue: () => void act(() => conv.continueTurn(turn)) }}
  />
{:else if turn.error}
  <div class="indent">
    <Lazy load={loadError} props={{ turn, error: turn.error, conv }} label={t('common.loading')} />
  </div>
{/if}

<style>
  .indent {
    margin-left: 32px;
  }
</style>
