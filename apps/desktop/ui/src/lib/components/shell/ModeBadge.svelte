<!--
  Znak „Pełny dostęp” w belce. Tryb pracy wybiera się w pasku pisania (29.10), ale gdy agentki
  działają bez pytań, widać to zawsze — także w Ustawieniach. Klik otwiera menu trybów.
-->
<script lang="ts">
  import { Popover } from '@alfa/ui-kit';
  import ShieldAlert from '@lucide/svelte/icons/shield-alert';
  import { useApp } from '../../state/context';
  import Lazy from './Lazy.svelte';

  const app = useApp();
  const { t, tk } = app.i18n;
  const loadModes = () => import('./ModeMenu.svelte');
  const level = $derived(app.sessions.active?.autonomy ?? app.autonomy.global ?? 'L3');
  let open = $state(false);
</script>

{#if level === 'L4'}
  <Popover label={t('mode.title')} bind:open width={340}>
    {#snippet trigger(props)}
      <button
        type="button"
        class="badge"
        {...props}
        aria-label={t('mode.badge')}
        title={t('mode.badge')}
      >
        <ShieldAlert size={14} strokeWidth={1.75} aria-hidden="true" />
        <span class="word">{tk('mode.L4.name')}</span>
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
