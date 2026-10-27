<!--
  Stan w pasku tytułu jako jedna kapsuła z trzema częściami. Lewa: połączenie i profil modeli —
  klik otwiera objaśnienie z przejściem do właściwych ustawień. Środkowa: tryb pracy (poziom
  autonomii) — klik otwiera menu trybów (`ModeMenu`). Prawa: koszt sesji — klik otwiera szczegóły
  kosztów; przy 80 % limitu miesięcznego część kosztu jest oznaczona ostrzeżeniem.
-->
<script lang="ts">
  import { Popover } from '@alfa/ui-kit';
  import Cloud from '@lucide/svelte/icons/cloud';
  import Coins from '@lucide/svelte/icons/coins';
  import Combine from '@lucide/svelte/icons/combine';
  import Laptop from '@lucide/svelte/icons/laptop';
  import Shield from '@lucide/svelte/icons/shield';
  import WifiOff from '@lucide/svelte/icons/wifi-off';
  import { useApp } from '../../state/context';
  import Lazy from './Lazy.svelte';

  const app = useApp();
  const { t, tk } = app.i18n;
  // Treść okienek ładowana dopiero po otwarciu — poza budżetem startowym okna.
  const loadStatus = () => import('./StatusDetails.svelte');
  const loadCosts = () => import('./CostDetails.svelte');
  const loadModes = () => import('./ModeMenu.svelte');
  const session = $derived(app.sessions.active);
  const profileId = $derived(app.system?.profile ?? session?.profile ?? 'hybrid');
  const profile = $derived(t(`profile.${profileId}`));
  const level = $derived(session?.autonomy ?? 'L3');
  const offline = $derived(app.system?.online === false);
  const cost = $derived(app.costs ? app.i18n.money(app.costs.session) : '—');
  const nearLimit = $derived(
    app.costs !== null &&
      app.costs.limit.enabled &&
      app.costs.month.minor / Math.max(1, app.costs.limit.monthly.minor) >= 0.8,
  );
  const ProfileIcon = $derived(
    profileId === 'local' ? Laptop : profileId === 'cloud' ? Cloud : Combine,
  );
  let stateOpen = $state(false);
  let costOpen = $state(false);
  let modeOpen = $state(false);
</script>

<div class="cluster">
  <Popover label={t('status.title')} bind:open={stateOpen} width={320}>
    {#snippet trigger(props)}
      <button
        type="button"
        class="seg state"
        class:offline
        {...props}
        aria-label={t('titlebar.state', {
          connection: t(offline ? 'status.offline' : 'status.online'),
          profile,
          level,
        })}
        title={t('titlebar.stateTip')}
      >
        <span class="conn" aria-hidden="true">
          {#if offline}<WifiOff size={14} strokeWidth={1.75} />{:else}<span class="dot"></span>{/if}
        </span>
        <span class="ico" aria-hidden="true"><ProfileIcon size={14} strokeWidth={1.75} /></span>
        <span class="word">{profile}</span>
      </button>
    {/snippet}
    <Lazy
      load={loadStatus}
      props={{ onnavigate: () => (stateOpen = false) }}
      label={t('common.loading')}
    />
  </Popover>
  <Popover label={t('mode.title')} bind:open={modeOpen} width={340}>
    {#snippet trigger(props)}
      <button
        type="button"
        class="seg mode"
        class:full={level === 'L4'}
        {...props}
        aria-label={t('titlebar.mode', { mode: tk(`mode.${level}.name`) })}
        title={t('titlebar.modeTip')}
      >
        <span class="level">
          <Shield size={13} strokeWidth={1.75} aria-hidden="true" />
          <span class="mode-name">{tk(`mode.${level}.name`)}</span>
          <span class="mode-short">{level}</span>
        </span>
      </button>
    {/snippet}
    <Lazy
      load={loadModes}
      props={{ current: level, onclose: () => (modeOpen = false) }}
      label={t('common.loading')}
    />
  </Popover>
  <span class="rule" aria-hidden="true"></span>
  <Popover label={t('costs.title')} bind:open={costOpen}>
    {#snippet trigger(props)}
      <button
        type="button"
        class="seg cost"
        class:warn={nearLimit}
        {...props}
        aria-label={t('titlebar.cost', { cost })}
        title={t('titlebar.costTip')}
      >
        <span class="ico" aria-hidden="true"><Coins size={14} strokeWidth={1.75} /></span>
        <span class="num">{cost}</span>
      </button>
    {/snippet}
    <Lazy load={loadCosts} props={{}} label={t('common.loading')} />
  </Popover>
</div>

<style>
  .cluster {
    display: inline-flex;
    align-items: center;
    height: 34px;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-bg);
    white-space: nowrap;
  }
  .cluster :global(.anchor) {
    display: inline-flex;
    height: 100%;
  }
  .rule {
    width: 1px;
    height: 16px;
    background: var(--alfa-color-border);
  }
  .seg {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 100%;
    padding: 0 var(--alfa-space-3);
    border: 0;
    background: transparent;
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-sm);
  }
  .seg.state {
    border-radius: var(--alfa-radius-full) 0 0 var(--alfa-radius-full);
  }
  .seg.cost {
    border-radius: 0 var(--alfa-radius-full) var(--alfa-radius-full) 0;
    font-variant-numeric: tabular-nums;
  }
  .seg:hover,
  .seg[aria-expanded='true'] {
    background: var(--alfa-color-surface2);
  }
  .seg:focus-visible {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
    outline-offset: -2px;
  }
  .conn,
  .ico {
    display: inline-flex;
    color: var(--alfa-color-text-muted);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-success);
  }
  .offline .conn {
    color: var(--alfa-color-warning);
  }
  .word {
    font-weight: var(--alfa-weight-semibold);
  }
  .level {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 0 6px;
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-full);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
    line-height: 18px;
  }
  .seg.mode {
    padding: 0 var(--alfa-space-2);
  }
  .mode-short {
    display: none;
  }
  .full .level {
    border-color: var(--alfa-color-warning);
    color: var(--alfa-color-warning);
  }
  .cost.warn {
    color: var(--alfa-color-text);
    box-shadow: inset 0 -2px 0 var(--alfa-color-warning);
  }
  .cost.warn .ico {
    color: var(--alfa-color-warning);
  }
  /* Wąskie okno: zamiast nazwy trybu skrót „L3” (pełna nazwa zostaje w etykiecie przycisku). */
  @media (max-width: 900px) {
    .word,
    .mode-name {
      display: none;
    }
    .mode-short {
      display: inline;
    }
  }
  @media (max-width: 720px) {
    .state .ico,
    .level :global(svg),
    .cost .ico {
      display: none;
    }
    .seg {
      padding: 0 var(--alfa-space-2);
    }
  }
  /* Bardzo wąskie okno: zostaje sam znak połączenia — reszta jest w objaśnieniu po kliknięciu. */
  @media (max-width: 520px) {
    .cluster > :global(.anchor:nth-child(2)),
    .rule,
    .cluster > :global(.anchor:last-child) {
      display: none;
    }
    .seg.state {
      border-radius: var(--alfa-radius-full);
    }
  }
</style>
