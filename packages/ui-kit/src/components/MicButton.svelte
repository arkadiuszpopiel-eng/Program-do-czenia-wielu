<script lang="ts">
  import Mic from '@lucide/svelte/icons/mic';
  import MicOff from '@lucide/svelte/icons/mic-off';
  import Ear from '@lucide/svelte/icons/ear';
  import LoaderCircle from '@lucide/svelte/icons/loader-circle';
  import Volume2 from '@lucide/svelte/icons/volume-2';
  import VolumeX from '@lucide/svelte/icons/volume-x';
  import BellOff from '@lucide/svelte/icons/bell-off';
  import type { MicState } from '../types';
  import type { AgentId } from '../tokens';

  interface Props {
    state: MicState;
    /** Agentka, która mówi (dla stanu speaking) — kolor akcentu. */
    agent?: AgentId;
    /** Poziom głośności 0–1 (dla stanu hearing) — wypełnienie tła. */
    level?: number;
    /** Pełna etykieta obok ikony (domyślnie tak; w composerze — tylko ikona + tooltip). */
    showLabel?: boolean;
    onclick?: (event: MouseEvent) => void;
    disabled?: boolean;
    /** Teksty per stan (i18n); domyślnie po polsku. */
    labels?: Partial<Record<MicState, { label: string; hint: string }>>;
  }

  let {
    state,
    agent,
    level = 0,
    showLabel = true,
    onclick,
    disabled = false,
    labels = {},
  }: Props = $props();

  const STATES = {
    off: { label: 'Mikrofon wyłączony', icon: MicOff, hint: 'Włącz mikrofon' },
    listening: { label: 'Słucha', icon: Mic, hint: 'Wycisz' },
    hearing: { label: 'Słyszy Cię', icon: Ear, hint: 'Wycisz' },
    processing: { label: 'Przetwarza', icon: LoaderCircle, hint: 'Przerwij' },
    speaking: { label: 'Agentka mówi', icon: Volume2, hint: 'Zatrzymaj mowę' },
    muted: { label: 'Wyciszony', icon: VolumeX, hint: 'Włącz słuchanie' },
    dnd: { label: 'Nie przeszkadzać', icon: BellOff, hint: 'Wyłącz „nie przeszkadzać"' },
  } as const;

  const s = $derived({ ...STATES[state], ...labels[state] });
  const Icon = $derived(s.icon);
  const active = $derived(state === 'listening' || state === 'hearing');
  const accent = $derived(
    state === 'speaking' && agent
      ? `var(--alfa-agent-${agent})`
      : active
        ? 'var(--alfa-color-success)'
        : state === 'processing'
          ? 'var(--alfa-color-info)'
          : 'var(--alfa-color-text-muted)',
  );
</script>

<button
  type="button"
  class="mic {state}"
  class:with-label={showLabel}
  style:--accent={accent}
  style:--level={Math.max(0, Math.min(1, level))}
  aria-label="{s.label}. {s.hint}"
  title="{s.label} — {s.hint}"
  aria-pressed={active || state === 'speaking' || state === 'processing'}
  {onclick}
  {disabled}
>
  <span class="fill" aria-hidden="true"></span>
  <span class="icon" class:spin={state === 'processing'} aria-hidden="true">
    <Icon size={showLabel ? 16 : 18} strokeWidth={1.5} />
  </span>
  {#if showLabel}<span class="label">{s.label}</span>{/if}
</button>

<style>
  .mic {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--alfa-space-2);
    height: var(--alfa-size-control);
    min-width: var(--alfa-size-control);
    padding: 0;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface);
    color: var(--accent);
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
    overflow: hidden;
    isolation: isolate;
    transition: transform var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .with-label {
    padding: 0 var(--alfa-space-3) 0 var(--alfa-space-2);
  }
  .mic:hover:not(:disabled) {
    background: var(--alfa-color-surface2);
  }
  .mic:active:not(:disabled) {
    transform: scale(0.97);
  }
  .mic:disabled {
    opacity: 0.5;
  }
  .listening,
  .hearing,
  .speaking,
  .processing {
    border-color: color-mix(in srgb, var(--accent) 50%, transparent);
  }
  .off,
  .muted,
  .dnd {
    color: var(--alfa-color-text-muted);
  }
  /* Poziom głośności jako wypełnienie od lewej (transform → tanie). */
  .fill {
    position: absolute;
    inset: 0;
    z-index: -1;
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    transform: scaleX(var(--level));
    transform-origin: left;
    transition: transform 60ms linear;
  }
  .hearing .fill {
    display: block;
  }
  .mic:not(.hearing) .fill {
    display: none;
  }
  .icon {
    display: inline-flex;
  }
  .spin {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (forced-colors: active) {
    .mic {
      forced-color-adjust: none;
      background: ButtonFace;
      color: ButtonText;
      border-color: ButtonText;
    }
    .mic[aria-pressed='true'] {
      background: Highlight;
      color: HighlightText;
    }
  }
</style>
