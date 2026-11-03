<!--
  Znak „Pełny dostęp” w belce. Tryb pracy wybiera się w pasku pisania (29.10), ale gdy agentki
  działają bez pytań, widać to zawsze — także w Ustawieniach. Klik otwiera menu trybów.
  Włączony „Max dostęp” (plan 50 §8.1) ma pierwszeństwo: czerwony znak z zasięgiem. Zamknięta
  kłódka profilu Dziecko (plan 50 §4.4) — znak z kłódką (Sędzia i tak obniża wtedy tryb).
-->
<script lang="ts">
  import { Popover } from '@alfa/ui-kit';
  import Lock from '@lucide/svelte/icons/lock';
  import ShieldAlert from '@lucide/svelte/icons/shield-alert';
  import Zap from '@lucide/svelte/icons/zap';
  import { useApp } from '../../state/context';
  import Lazy from './Lazy.svelte';

  const app = useApp();
  const { t, tk } = app.i18n;
  const loadModes = () => import('./ModeMenu.svelte');
  const level = $derived(app.sessions.active?.autonomy ?? app.autonomy.global ?? 'L3');
  const locked = $derived(app.autonomy.locked);
  const max = $derived(locked ? null : app.autonomy.maxAccess);
  const label = $derived(
    locked
      ? t('child.badge')
      : max
        ? t('maxAccess.badge', { where: t(`maxAccess.scope.${max}`) })
        : t('mode.badge'),
  );
  let open = $state(false);
</script>

<!-- Otwarte menu zostaje, gdy w nim wyłączono „Max dostęp” albo zszedł Pełny dostęp. -->
{#if level === 'L4' || max || locked || open}
  <Popover label={t('mode.title')} bind:open width={340}>
    {#snippet trigger(props)}
      <button
        type="button"
        class="badge"
        class:max
        class:locked
        {...props}
        aria-label={label}
        title={label}
      >
        {#if locked}
          <Lock size={14} strokeWidth={2} aria-hidden="true" />
          <span class="word">{t('child.badgeWord')}</span>
        {:else if max}
          <Zap size={14} strokeWidth={2} aria-hidden="true" />
          <span class="word">{t('maxAccess.name')}</span>
        {:else}
          <ShieldAlert size={14} strokeWidth={1.75} aria-hidden="true" />
          <span class="word">{tk('mode.L4.name')}</span>
        {/if}
      </button>
    {/snippet}
    <Lazy
      load={loadModes}
      props={{
        // Fokus wraca na znak (po zejściu z Pełnego dostępu znaku już nie ma — wtedy zostaje).
        onclose: () => {
          open = false;
          requestAnimationFrame(() =>
            document.querySelector<HTMLElement>('header.titlebar .badge')?.focus(),
          );
        },
      }}
      label={t('common.loading')}
    />
  </Popover>
{/if}

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: var(--bar-h, 32px);
    padding: 0 var(--alfa-space-3);
    border: 1px solid color-mix(in srgb, var(--alfa-color-warning) 55%, transparent);
    border-radius: var(--alfa-radius-full);
    background: color-mix(in srgb, var(--alfa-color-warning) 12%, transparent);
    /* Napis o ton bliżej tekstu niż samo ostrzeżenie — kontrast ≥ 4,5 na każdym tle belki. */
    color: color-mix(in srgb, var(--alfa-color-warning) 75%, var(--alfa-color-text));
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
    white-space: nowrap;
  }
  .badge:hover,
  .badge[aria-expanded='true'] {
    background: color-mix(in srgb, var(--alfa-color-warning) 20%, transparent);
  }
  .badge.locked {
    border-color: var(--alfa-color-border-strong);
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text);
  }
  .badge.max {
    border-color: color-mix(in srgb, var(--alfa-color-error) 60%, transparent);
    background: color-mix(in srgb, var(--alfa-color-error) 12%, transparent);
    color: color-mix(in srgb, var(--alfa-color-error) 75%, var(--alfa-color-text));
    box-shadow: 0 0 10px color-mix(in srgb, var(--alfa-color-error) 35%, transparent);
    text-shadow: 0 0 8px color-mix(in srgb, var(--alfa-color-error) 50%, transparent);
  }
  .badge.max:hover,
  .badge.max[aria-expanded='true'] {
    background: color-mix(in srgb, var(--alfa-color-error) 20%, transparent);
  }
  .badge:focus-visible {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
    outline-offset: 2px;
  }
  @media (max-width: 1100px) {
    .word {
      display: none;
    }
    .badge {
      width: var(--bar-h, 32px);
      padding: 0;
      justify-content: center;
    }
  }
</style>
