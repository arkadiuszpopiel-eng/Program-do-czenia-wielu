<!--
  Konto mostu CLI na karcie (Ustawienia → Mosty CLI): wynik ostatniego sprawdzenia logowania
  („Zalogowano · plan Max — Alfa może zlecać mu zadania”) i „Sprawdź logowanie” (`bridges_check` —
  oficjalne polecenie stanu narzędzia; Alfa nie czyta danych logowania).
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { BridgeCard } from '../../../api/types-tasks';
  import { bridgeRow } from '../../../logic/connections';
  import { showError } from '../../../state/attempt';
  import { useApp } from '../../../state/context';

  interface Props {
    card: BridgeCard;
    onchange: (card: BridgeCard) => void;
  }

  let { card, onchange }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  let checking = $state(false);
  const row = $derived(card.account ? bridgeRow(card) : null);

  async function check() {
    const bridge = card.bridge;
    if (!bridge) return;
    checking = true;
    try {
      onchange(await app.client.bridges.check(bridge));
    } catch (error) {
      showError(app.toasts, error);
    } finally {
      checking = false;
    }
  }
</script>

<div class="account" data-account={row?.state ?? 'unknown'}>
  <p role="status">
    {#if row && card.account}
      {tk(row.text, row.vars)}
      <span class="when"
        >· {t('conn.checked', { when: app.i18n.relative(card.account.checked_at) })}</span
      >
    {:else}
      {t('conn.bridge.unchecked')}
    {/if}
  </p>
  <Button size="sm" variant="secondary" loading={checking} onclick={check}
    >{t('bridges.checkLogin')}</Button
  >
</div>

<style>
  .account {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border-left: 3px solid var(--alfa-color-text-subtle);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface2);
  }
  .account[data-account='ok'] {
    border-left-color: var(--alfa-color-success);
  }
  .account[data-account='attention'] {
    border-left-color: var(--alfa-color-warning);
  }
  .when {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
</style>
