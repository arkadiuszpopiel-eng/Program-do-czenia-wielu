<!-- Błąd na poziomie wiadomości z akcjami „Ponów / inny model" (PLAN §14.8). -->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import type { Turn, TurnError } from '../../api/types';
  import { attempt } from '../../state/attempt';
  import { now } from '../../state/clock.svelte';
  import { useApp } from '../../state/context';
  import type { ConversationState } from '../../state/conversation.svelte';

  interface Props {
    turn: Turn;
    error: TurnError;
    conv: ConversationState;
  }

  let { turn, error, conv }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;

  let busy = $state(false);

  // Ponowienie, które samo się nie uda, daje toast — przycisk nie „nic nie robi".
  async function retry(profile: string | null) {
    busy = true;
    await attempt(app.toasts, () => conv.regenerate(turn, profile));
    busy = false;
  }

  const text = $derived.by(() => {
    if (error.code === 'rate_limited' && error.retry_at) {
      return t('conv.error.rate_limited', {
        provider: error.provider ?? '',
        time: app.i18n.time(error.retry_at),
        relative: app.i18n.relative(error.retry_at, now()),
      });
    }
    return error.code === 'provider'
      ? t('conv.error.provider', { message: error.message })
      : t(`conv.error.${error.code}`);
  });
</script>

<div class="error" role="alert">
  <TriangleAlert size={16} strokeWidth={1.5} aria-hidden="true" />
  <span class="text">{text}</span>
  <Button size="sm" variant="secondary" disabled={busy} onclick={() => void retry(null)}>
    {t('conv.retry')}
  </Button>
  <Button size="sm" variant="ghost" disabled={busy} onclick={() => void retry('local')}>
    {t('conv.otherModel')}
  </Button>
</div>

<style>
  .error {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border: 1px solid color-mix(in srgb, var(--alfa-color-error) 40%, var(--alfa-color-border));
    border-radius: var(--alfa-radius-card);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-sm);
  }
  .error > :global(svg) {
    color: var(--alfa-color-error);
  }
  .text {
    flex: 1;
    min-width: 12ch;
  }
</style>
