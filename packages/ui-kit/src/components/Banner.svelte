<script lang="ts">
  import type { Snippet } from 'svelte';
  import Info from '@lucide/svelte/icons/info';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import WifiOff from '@lucide/svelte/icons/wifi-off';
  import KeyRound from '@lucide/svelte/icons/key-round';
  import MicOff from '@lucide/svelte/icons/mic-off';
  import HardDrive from '@lucide/svelte/icons/hard-drive';
  import Clock from '@lucide/svelte/icons/clock';
  import X from '@lucide/svelte/icons/x';
  import IconButton from './IconButton.svelte';
  import type { ToastKind } from '../types';

  type BannerIcon = 'info' | 'warning' | 'offline' | 'key' | 'mic' | 'disk' | 'clock';

  interface Props {
    kind?: ToastKind;
    icon?: BannerIcon;
    children: Snippet;
    actions?: Snippet;
    ondismiss?: () => void;
    dismissLabel?: string;
  }

  let {
    kind = 'info',
    icon = 'info',
    children,
    actions,
    ondismiss,
    dismissLabel = 'Ukryj',
  }: Props = $props();

  const ICONS = {
    info: Info,
    warning: TriangleAlert,
    offline: WifiOff,
    key: KeyRound,
    mic: MicOff,
    disk: HardDrive,
    clock: Clock,
  } as const;
  const Icon = $derived(ICONS[icon]);
</script>

<div class="banner {kind}" role={kind === 'error' ? 'alert' : 'status'}>
  <span class="icon" aria-hidden="true"><Icon size={16} strokeWidth={1.5} /></span>
  <div class="text">{@render children()}</div>
  {#if actions}<div class="actions">{@render actions()}</div>{/if}
  {#if ondismiss}
    <IconButton label={dismissLabel} size="sm" onclick={ondismiss}>
      <X size={14} strokeWidth={1.5} />
    </IconButton>
  {/if}
</div>

<style>
  .banner {
    --accent: var(--alfa-color-info);
    display: flex;
    align-items: center;
    gap: var(--alfa-space-3);
    padding: var(--alfa-space-2) var(--alfa-space-2) var(--alfa-space-2) var(--alfa-space-3);
    border: 1px solid color-mix(in srgb, var(--accent) 35%, var(--alfa-color-border));
    border-left: 3px solid var(--accent);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    font-size: var(--alfa-font-size-sm);
  }
  .warning {
    --accent: var(--alfa-color-warning);
  }
  .error {
    --accent: var(--alfa-color-error);
  }
  .success {
    --accent: var(--alfa-color-success);
  }
  .icon {
    display: inline-flex;
    flex: none;
    color: var(--accent);
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
  }
</style>
