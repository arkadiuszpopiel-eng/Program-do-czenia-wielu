<!-- Potwierdzenie decyzji (alertdialog z pułapką fokusu — bits-ui). Nie służy do zatwierdzeń akcji agentek. -->
<script lang="ts">
  import { AlertDialog } from 'bits-ui';
  import Button from './Button.svelte';

  interface Props {
    open?: boolean;
    title: string;
    description: string;
    confirmLabel: string;
    cancelLabel?: string;
    danger?: boolean;
    onconfirm: () => void;
  }

  let {
    open = $bindable(false),
    title,
    description,
    confirmLabel,
    cancelLabel = 'Anuluj',
    danger = false,
    onconfirm,
  }: Props = $props();
</script>

<AlertDialog.Root bind:open>
  <AlertDialog.Portal>
    <AlertDialog.Overlay class="alfa-confirm-overlay" />
    <AlertDialog.Content class="alfa-confirm">
      <AlertDialog.Title class="alfa-confirm-title">{title}</AlertDialog.Title>
      <AlertDialog.Description class="alfa-confirm-desc">{description}</AlertDialog.Description>
      <div class="actions">
        <AlertDialog.Cancel>
          {#snippet child({ props })}
            <Button {...props} variant="secondary">{cancelLabel}</Button>
          {/snippet}
        </AlertDialog.Cancel>
        <AlertDialog.Action onclick={onconfirm}>
          {#snippet child({ props })}
            <Button {...props} variant={danger ? 'danger' : 'primary'}>{confirmLabel}</Button>
          {/snippet}
        </AlertDialog.Action>
      </div>
    </AlertDialog.Content>
  </AlertDialog.Portal>
</AlertDialog.Root>

<style>
  :global(.alfa-confirm-overlay) {
    position: fixed;
    inset: 0;
    z-index: 110;
    background: var(--alfa-color-scrim);
  }
  :global(.alfa-confirm) {
    position: fixed;
    z-index: 111;
    top: 50%;
    left: 50%;
    width: min(440px, calc(100vw - 32px));
    padding: var(--alfa-space-6);
    transform: translate(-50%, -50%);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-overlay);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-3);
    color: var(--alfa-color-text);
  }
  :global(.alfa-confirm-title) {
    margin: 0 0 var(--alfa-space-2);
    font-size: var(--alfa-font-size-lg);
    font-weight: var(--alfa-weight-semibold);
  }
  :global(.alfa-confirm-desc) {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--alfa-space-2);
    margin-top: var(--alfa-space-6);
  }
</style>
