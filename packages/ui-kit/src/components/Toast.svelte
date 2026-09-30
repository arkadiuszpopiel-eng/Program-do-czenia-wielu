<script lang="ts">
  import Info from '@lucide/svelte/icons/info';
  import Check from '@lucide/svelte/icons/check';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import X from '@lucide/svelte/icons/x';
  import IconButton from './IconButton.svelte';
  import Button from './Button.svelte';
  import type { ToastKind } from '../types';

  interface Props {
    kind?: ToastKind;
    title?: string;
    message: string;
    /** Akcja np. „Cofnij" (§14.8 cofanie jednym kliknięciem). */
    actionLabel?: string;
    onaction?: () => void;
    onclose?: () => void;
  }

  let { kind = 'info', title, message, actionLabel, onaction, onclose }: Props = $props();
  const Icon = $derived(kind === 'success' ? Check : kind === 'info' ? Info : TriangleAlert);
  const role = $derived(kind === 'error' || kind === 'warning' ? 'alert' : 'status');
</script>

<div class="toast {kind}" {role}>
  <span class="icon" aria-hidden="true"><Icon size={16} strokeWidth={1.5} /></span>
  <div class="text">
    {#if title}<strong class="title">{title}</strong>{/if}
    <span class="msg">{message}</span>
  </div>
  {#if actionLabel && onaction}
    <Button size="sm" variant="ghost" onclick={onaction}>{actionLabel}</Button>
  {/if}
  {#if onclose}
    <IconButton label="Zamknij powiadomienie" size="sm" onclick={onclose}>
      <X size={14} strokeWidth={1.5} />
    </IconButton>
  {/if}
</div>

<style>
  .toast {
    --accent: var(--alfa-color-info);
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    width: min(360px, 100%);
    padding: var(--alfa-space-2) var(--alfa-space-2) var(--alfa-space-2) var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-left: 3px solid var(--accent);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-2);
    font-size: var(--alfa-font-size-sm);
  }
  .success {
    --accent: var(--alfa-color-success);
  }
  .warning {
    --accent: var(--alfa-color-warning);
  }
  .error {
    --accent: var(--alfa-color-error);
  }
  .icon {
    display: inline-flex;
    flex: none;
    color: var(--accent);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .title {
    font-weight: var(--alfa-weight-semibold);
  }
  .msg {
    color: var(--alfa-color-text-muted);
  }
</style>
