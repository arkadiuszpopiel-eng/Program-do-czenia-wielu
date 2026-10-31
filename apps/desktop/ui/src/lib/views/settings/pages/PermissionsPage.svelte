<!--
  Uprawnienia (PLAN §8.3): poziomy L0–L4 prostymi słowami. Zmiana poziomu to INTENCJA — okno
  Brokera prosi o potwierdzenie; to UI niczego nie zatwierdza i nie zmienia polityk samo.
-->
<script lang="ts">
  import { Button, SegmentedControl } from '@alfa/ui-kit';
  import ShieldCheck from '@lucide/svelte/icons/shield-check';
  import type { AutonomyLevel } from '../../../api/types';
  import type { PermissionsState } from '../../../api/types-hub';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import { watchRaise } from '../../../logic/raise-watch';
  import { load, showError } from '../../../state/attempt';
  import { useApp } from '../../../state/context';
  import BrokerStatusCard from './BrokerStatusCard.svelte';
  import GrantsCard from './GrantsCard.svelte';

  const app = useApp();
  const { t } = app.i18n;
  const LEVELS: readonly AutonomyLevel[] = ['L0', 'L1', 'L2', 'L3', 'L4'];
  let perms = $state<PermissionsState | null>(null);
  let wanted = $state<string>('L3');
  let scope = $state<string>('global');
  let loadError = $state<string | null>(null);

  async function reload(sessionId: string | null) {
    const result = await load(() => app.client.permissions.get(sessionId));
    if (result.status === 'ready') {
      perms = result.value;
      wanted = result.value.session ?? result.value.global;
      loadError = null;
    } else if (result.status === 'failed') loadError = result.error;
  }

  // Poziomy wczytujemy też przy każdej zmianie stanu Brokera (restart, zerwanie i powrót łącza):
  // wcześniej strona pokazywała poziom sprzed zmiany jako „obecny".
  $effect(() => {
    void app.broker.view;
    void reload(app.activeId);
  });

  // Bez stanu z rdzenia nie udajemy żadnego poziomu (wcześniej: „L3" na ślepo).
  const current = $derived(
    perms ? (scope === 'session' && perms.session ? perms.session : perms.global) : null,
  );

  // Prośba czekająca na zgodę w oknie Brokera (`logic/raise-watch`): po zatwierdzeniu strona sama
  // pokazuje nowy poziom; czekanie kończy się przy opuszczeniu strony albo nowej prośbie o
  // podniesienie. Obniżenie go nie kończy — karta poprzedniej prośby nadal żyje w oknie Brokera.
  let stopWatch: (() => void) | null = null;
  let asked = 0;
  let left = false;
  $effect(() => () => {
    left = true;
    stopWatch?.();
  });

  async function request() {
    const level = wanted as AutonomyLevel;
    const sessionId = scope === 'session' ? app.activeId : null;
    const mine = ++asked;
    try {
      const result = await app.client.permissions.requestLevel(level, sessionId);
      // Nowsza prośba albo opuszczenie strony w trakcie — ta odpowiedź niczego już nie zmienia.
      if (mine !== asked || left) return;
      if (result.status === 'applied') {
        perms = await app.client.permissions.get(app.activeId);
        app.toasts.show({ kind: 'success', message: t('perm.applied', { level }) });
      } else {
        app.toasts.show({ kind: 'info', message: t('perm.requested', { level }) });
        stopWatch?.();
        stopWatch = watchRaise({
          level,
          // Poziom zakresu z prośby, nie aktywnej sesji (można ją zmienić w trakcie czekania);
          // Broker nie podaje poziomu sesji równego globalnemu — wtedy obowiązuje globalny.
          read: async () => {
            const asked = sessionId ?? app.activeId;
            const next = await app.client.permissions.get(asked);
            if (asked === app.activeId) perms = next;
            return sessionId ? (next.session ?? next.global) : next.global;
          },
          onApplied: () => {
            stopWatch = null;
            if (sessionId === null || sessionId === app.activeId) wanted = level;
            app.toasts.show({ kind: 'success', message: t('perm.raised', { level }) });
          },
        });
      }
    } catch (error) {
      showError(app.toasts, error);
    }
  }
</script>

<BrokerStatusCard />
{#if loadError}
  <LoadFailed error={loadError} onretry={() => void reload(app.activeId)} />
{/if}
{#if perms}
  <section class="card" aria-labelledby="perm-title">
    <h3 id="perm-title">{t('perm.title')}</h3>
    <p class="desc">{t('perm.intro')}</p>
    <SegmentedControl
      label={t('perm.scope')}
      bind:value={scope}
      options={[
        { value: 'global', label: t('perm.scope.global') },
        { value: 'session', label: t('perm.scope.session') },
      ]}
    />
    <SegmentedControl
      label={t('perm.title')}
      variant="cards"
      bind:value={wanted}
      options={LEVELS.map((l) => ({
        value: l,
        label: `${l} · ${t(`perm.${l}.name`)}${l === current ? ` (${t('perm.current')})` : ''}`,
        description: t(`perm.${l}.desc`),
      }))}
    />
    <div class="foot">
      <p class="kept">
        <ShieldCheck size={14} strokeWidth={1.5} aria-hidden="true" />
        {t('perm.kept')}
      </p>
      <Button variant="primary" disabled={wanted === current} onclick={request}
        >{t('perm.request', { level: wanted })}</Button
      >
    </div>
    <p class="desc">
      {t('perm.hello', { state: t(perms.hello_enabled ? 'common.on' : 'common.off') })}
    </p>
  </section>
  <GrantsCard />
{/if}

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    margin-bottom: var(--alfa-space-4);
    padding: var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
  }
  h3 {
    font-size: var(--alfa-font-size-md);
  }
  .desc {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: var(--alfa-space-3);
  }
  .kept {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-1);
    flex: 1;
    min-width: 24ch;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
</style>
