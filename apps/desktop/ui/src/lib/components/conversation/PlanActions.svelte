<!--
  Trzy odpowiedzi pod planem (tryb „Plan”, plan 50 §4.1, Z.5) — jak w Claude Code: działaj
  automatycznie, zatwierdzaj ręcznie albo planuj dalej. Klawisze 1–3, gdy fokus jest na przycisku.
-->
<script lang="ts">
  import ClipboardCheck from '@lucide/svelte/icons/clipboard-check';
  import type { PersonaId } from '@alfa/ui-kit';
  import type { AutonomyLevel } from '../../api/types';
  import { acceptPlan, type PlanAccept } from '../../logic/plan-mode';
  import { showError } from '../../state/attempt';
  import { useApp } from '../../state/context';
  import type { ConversationState } from '../../state/conversation.svelte';

  interface Props {
    conv: ConversationState;
    /** Agentka, która przygotowała plan — dostaje polecenie wykonania. */
    agent: PersonaId | null;
  }
  let { conv, agent }: Props = $props();

  const app = useApp();
  const { t, tk } = app.i18n;
  let busy = $state(false);

  async function readLevel(): Promise<AutonomyLevel | null> {
    const state = await app.client.permissions.get(conv.sessionId);
    return state.session ?? state.global;
  }

  async function accept(choice: PlanAccept) {
    if (busy) return;
    busy = true;
    try {
      await acceptPlan(choice, {
        request: (level) => app.client.permissions.requestLevel(level, conv.sessionId),
        readLevel,
        execute: () => conv.send(t('plan.go'), agent, null),
        notify: (event, level) => {
          const mode = tk(`mode.${level}.name`);
          const kind = event === 'requested' ? 'info' : 'success';
          app.toasts.show({ kind, message: t(`plan.${event}`, { mode }) });
        },
      });
    } catch (error) {
      showError(app.toasts, error);
    } finally {
      busy = false;
    }
  }

  function refine() {
    document.querySelector<HTMLTextAreaElement>('.composer textarea')?.focus();
    app.toasts.show({ kind: 'info', message: t('plan.refineHint') });
  }

  function keydown(event: KeyboardEvent) {
    const pick = { '1': 'auto', '2': 'manual' } as const;
    if (event.key === '1' || event.key === '2') {
      event.preventDefault();
      void accept(pick[event.key]);
    } else if (event.key === '3') {
      event.preventDefault();
      refine();
    }
  }
</script>

<!-- Wpis kanału rozmowy (`role=feed` dopuszcza tylko artykuły). -->
<div class="plan" role="article" aria-labelledby="plan-title">
  <p class="title" id="plan-title">
    <ClipboardCheck size={16} strokeWidth={1.75} aria-hidden="true" />
    {t('plan.title')}
  </p>
  <div class="choices">
    <button
      type="button"
      class="choice primary"
      disabled={busy}
      onkeydown={keydown}
      onclick={() => accept('auto')}
    >
      <span class="key" aria-hidden="true">1</span>{t('plan.auto')}
    </button>
    <button
      type="button"
      class="choice"
      disabled={busy}
      onkeydown={keydown}
      onclick={() => accept('manual')}
    >
      <span class="key" aria-hidden="true">2</span>{t('plan.manual')}
    </button>
    <button type="button" class="choice" onkeydown={keydown} onclick={refine}>
      <span class="key" aria-hidden="true">3</span>{t('plan.refine')}
    </button>
  </div>
</div>

<style>
  .plan {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: var(--alfa-space-2) 0 var(--alfa-space-4);
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-left: 3px solid var(--alfa-color-accent, var(--alfa-color-info));
    border-radius: var(--alfa-radius-card, var(--alfa-radius-control));
    background: var(--alfa-color-surface);
  }
  .title {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    margin: 0;
    font-weight: var(--alfa-weight-semibold);
  }
  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
  }
  .choice {
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-2);
    min-height: 32px;
    padding: 0 var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-sm);
  }
  .choice:hover:not(:disabled) {
    background: var(--alfa-color-surface2);
  }
  .choice.primary {
    border-color: var(--alfa-color-accent, var(--alfa-color-info));
    background: var(--alfa-color-accent, var(--alfa-color-info));
    color: var(--alfa-color-text-on-accent);
  }
  .choice.primary:hover:not(:disabled) {
    background: color-mix(
      in srgb,
      var(--alfa-color-accent, var(--alfa-color-info)) 88%,
      var(--alfa-color-text)
    );
  }
  .choice:disabled {
    opacity: 0.6;
  }
  .choice:focus-visible {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
    outline-offset: 2px;
  }
  .key {
    color: inherit;
    font-size: var(--alfa-font-size-xs);
    font-variant-numeric: tabular-nums;
    opacity: 0.75;
  }
</style>
