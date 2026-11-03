<!--
  Po odmowie w oknie Sędziego Cieni: „Powiedz agentce, co zrobić inaczej” (plan 50 §4.2, opcja 4
  jak w Claude Code). Wskazówka trafia do trwającego zadania (sterowanie w locie); gdy zadanie
  już się skończyło — czeka w polu pisania, a właściciel wysyła ją Enterem.
-->
<script lang="ts">
  import MessageSquareReply from '@lucide/svelte/icons/message-square-reply';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  let open = $state(false);
  let text = $state('');
  let busy = $state(false);
  let field = $state<HTMLTextAreaElement | null>(null);

  $effect(() => {
    if (open) field?.focus();
  });

  /** Wskazówka w polu pisania (zadanie już się zakończyło). */
  function toComposer(hint: string) {
    const composer = document.querySelector<HTMLTextAreaElement>('.composer textarea');
    if (!composer) return;
    composer.value = hint;
    composer.dispatchEvent(new Event('input', { bubbles: true }));
    composer.focus();
  }

  async function send(event: SubmitEvent) {
    event.preventDefault();
    const hint = text.trim();
    const session = app.sessions.active?.id;
    if (!hint || !session || busy) return;
    busy = true;
    try {
      await app.client.agents.steer(session, hint);
      app.toasts.show({ kind: 'success', message: t('approval.feedback.sent') });
    } catch {
      toComposer(hint);
      app.toasts.show({ kind: 'info', message: t('approval.feedback.toComposer') });
    } finally {
      busy = false;
      open = false;
      text = '';
    }
  }

  function keydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.stopPropagation();
      open = false;
    }
  }
</script>

{#if open}
  <form class="feedback" onsubmit={send}>
    <label class="label" for="deny-feedback">{t('approval.feedback.label')}</label>
    <textarea
      id="deny-feedback"
      bind:this={field}
      bind:value={text}
      rows="2"
      maxlength="2000"
      placeholder={t('approval.feedback.placeholder')}
      onkeydown={keydown}></textarea>
    <div class="actions">
      <button type="button" class="ghost" onclick={() => (open = false)}>
        {t('common.cancel')}
      </button>
      <button type="submit" class="send" disabled={busy || !text.trim()}>
        {t('approval.feedback.send')}
      </button>
    </div>
  </form>
{:else}
  <button type="button" class="open" onclick={() => (open = true)}>
    <MessageSquareReply size={14} strokeWidth={1.75} aria-hidden="true" />
    {t('approval.feedback.open')}
  </button>
{/if}

<style>
  .open {
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-1);
    margin-top: var(--alfa-space-2);
    padding: var(--alfa-space-1) var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-sm);
  }
  .open:hover,
  .ghost:hover {
    background: var(--alfa-color-surface2);
  }
  .feedback {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    margin-top: var(--alfa-space-2);
  }
  .label {
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  textarea {
    resize: vertical;
    padding: var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--alfa-space-2);
  }
  .ghost,
  .send {
    min-height: 30px;
    padding: 0 var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-sm);
  }
  .send {
    border-color: var(--alfa-color-accent, var(--alfa-color-info));
    background: var(--alfa-color-accent, var(--alfa-color-info));
    color: var(--alfa-color-text-on-accent);
  }
  .send:disabled {
    opacity: 0.6;
  }
</style>
