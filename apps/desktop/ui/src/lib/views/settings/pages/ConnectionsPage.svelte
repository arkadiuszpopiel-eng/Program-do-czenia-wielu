<!--
  Ustawienia → Stan połączeń (zgłoszenie właściciela 2026-10-14): jedno przejrzyste okno z tym, co
  jest zalogowane i działa — konta API (wynik ostatniego testu klucza), mosty CLI (zalogowano? plan?
  most włączony?), modele lokalne, silniki głosu, Broker, sieć i mikrofon. „Sprawdź wszystko” testuje
  klucze i logowanie narzędzi; każdy wiersz ma własne „Sprawdź” / „Zaloguj” / „Szczegóły”.
  Logowanie narzędzi sprawdza się samo przy otwarciu strony (polecenie stanu CLI, bez danych konta).
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import { Button } from '@alfa/ui-kit';
  import { errorText } from '../../../api/command-error';
  import type { Account } from '../../../api/types-hub';
  import type { BrokerStatusView } from '../../../api/types-broker';
  import type { ModelItem } from '../../../api/types-models';
  import type { SystemStatus } from '../../../api/types-system';
  import type { BridgeCard } from '../../../api/types-tasks';
  import Loading from '../../../components/shell/Loading.svelte';
  import {
    CONN_GROUPS,
    connectionRows,
    connectionSummary,
    type ConnAction,
    type ConnRow,
  } from '../../../logic/connections';
  import { loginProfile } from '../../../logic/work';
  import { showError } from '../../../state/attempt';
  import { useApp } from '../../../state/context';

  const app = useApp();
  const { t, tk } = app.i18n;

  let accounts = $state<readonly Account[] | null>(null);
  let bridges = $state<readonly BridgeCard[] | null>(null);
  let engines = $state<readonly ModelItem[] | null>(null);
  let broker = $state<BrokerStatusView | null>(null);
  let system = $state<SystemStatus | null>(null);
  let failures = $state<string[]>([]);
  let loaded = $state(false);
  const checking = new SvelteSet<string>();
  let checkingAll = $state(false);

  const rows = $derived(connectionRows({ accounts, bridges, engines, broker, system, checking }));
  const summary = $derived(connectionSummary(rows));

  async function settle<T>(what: string, call: () => Promise<T>): Promise<T | null> {
    try {
      return await call();
    } catch (error) {
      failures = [...failures, `${what} (${errorText(error)})`];
      return null;
    }
  }

  async function load(refresh = false) {
    failures = [];
    const [a, b, e, br, s] = await Promise.all([
      settle(t('conn.group.accounts'), () => app.client.accounts.list()),
      settle(t('conn.group.bridges'), () => app.client.bridges.list(refresh)),
      settle(t('conn.group.local'), () => app.client.engines.list()),
      settle(t('conn.broker.name'), () => app.client.broker.status()),
      settle(t('conn.network.name'), () => app.client.system.status()),
    ]);
    accounts = a;
    bridges = b;
    engines = e?.items ?? null;
    broker = br;
    system = s;
    loaded = true;
  }

  function mark(id: string, on: boolean) {
    if (on) checking.add(id);
    else checking.delete(id);
  }

  function putBridge(card: BridgeCard) {
    bridges = (bridges ?? []).map((c) => (c.route_id === card.route_id ? card : c));
  }

  async function checkBridge(bridge: string, quiet = false) {
    const id = `bridge:${bridge}`;
    mark(id, true);
    try {
      putBridge(await app.client.bridges.check(bridge));
    } catch (error) {
      if (!quiet) showError(app.toasts, error);
    } finally {
      mark(id, false);
    }
  }

  async function testAccount(accountId: string) {
    const id = `account:${accountId}`;
    mark(id, true);
    try {
      await app.client.accounts.test(accountId);
      accounts = await app.client.accounts.list();
    } catch (error) {
      showError(app.toasts, error);
    } finally {
      mark(id, false);
    }
  }

  async function checkAll() {
    checkingAll = true;
    try {
      await load(true);
      const tests = (accounts ?? [])
        .filter((a) => a.key_stored && a.state !== 'disabled')
        .map((a) => testAccount(a.id));
      const checks = (bridges ?? [])
        .filter((c) => c.bridge && c.detected)
        .map((c) => checkBridge(c.bridge ?? '', true));
      await Promise.all([...tests, ...checks]);
      const [br, s] = await Promise.all([
        settle(t('conn.broker.name'), () => app.client.broker.status()),
        settle(t('conn.network.name'), () => app.client.system.status()),
      ]);
      broker = br;
      system = s;
    } finally {
      checkingAll = false;
    }
  }

  function run(action: ConnAction) {
    if (action.kind === 'test') void testAccount(action.accountId);
    else if (action.kind === 'check') void checkBridge(action.bridge);
    else if (action.kind === 'open') app.openSettings(action.page);
    else {
      const profile = loginProfile(action.bridge);
      if (profile) app.work.openTerminal(profile);
      else app.openSettings('bridges');
    }
  }

  // Pierwsze wczytanie, potem ciche sprawdzenie logowania wykrytych narzędzi bez wyniku.
  $effect(() => {
    void load().then(() => {
      for (const c of bridges ?? []) {
        if (c.bridge && c.detected && !c.account) void checkBridge(c.bridge, true);
      }
    });
  });

  // Wynik sprawdzenia po „Gotowe” w oknie logowania trafia od razu do wiersza.
  $effect(() => {
    const card = app.work.bridgeChecked;
    // Tylko ta karta wyzwala efekt — `putBridge` czyta i zapisuje listę (bez śledzenia).
    if (card) untrack(() => bridges && putBridge(card));
  });

  const name = (r: ConnRow) => (r.nameKey ? tk(r.nameKey) : r.name);
</script>

<section class="conn" aria-labelledby="conn-title">
  <h3 id="conn-title" class="sr-only">{t('conn.openPage')}</h3>
  <p class="intro">{t('conn.intro')}</p>
  <div class="bar">
    <p class="summary" role="status" aria-live="polite">
      {#if loaded}
        {#if summary.attention === 0 && summary.unknown === 0}
          <span class="dot" data-state="ok" aria-hidden="true"></span>{t('conn.allGood')}
        {/if}
        <span class="counts">
          {[
            t('conn.summary.ok', { n: summary.ok }),
            ...(summary.attention ? [t('conn.summary.attention', { n: summary.attention })] : []),
            ...(summary.off ? [t('conn.summary.off', { n: summary.off })] : []),
            ...(summary.unknown ? [t('conn.summary.unknown', { n: summary.unknown })] : []),
          ].join(' · ')}
        </span>
      {/if}
    </p>
    <Button size="sm" variant="primary" loading={checkingAll} onclick={() => void checkAll()}
      >{t('conn.checkAll')}</Button
    >
  </div>
  <p class="hint">{t('conn.checkAllHint')}</p>
  {#each failures as f (f)}
    <p class="fail" role="alert">{t('conn.loadFailed', { what: f })}</p>
  {/each}
  {#if !loaded}
    <Loading />
  {:else}
    {#each CONN_GROUPS as group (group)}
      {@const items = rows.filter((r) => r.group === group)}
      <section class="group" aria-labelledby={`conn-g-${group}`}>
        <h4 id={`conn-g-${group}`}>{tk(`conn.group.${group}`)}</h4>
        {#if items.length === 0}
          <p class="empty">
            {group === 'accounts' ? t('conn.group.accountsEmpty') : t('conn.group.empty')}
          </p>
        {:else}
          <ul class="rows">
            {#each items as r (r.id)}
              <li class="row" data-state={r.state} data-conn={r.id}>
                <span class="dot" data-state={r.state} aria-hidden="true"></span>
                <div class="body">
                  <div class="top">
                    <span class="name">{name(r)}</span>
                    <span class="state">{tk(`conn.state.${r.state}`)}</span>
                  </div>
                  <p class="text">{tk(r.text, r.vars)}</p>
                  {#if r.group === 'accounts' || r.group === 'bridges'}
                    <p class="when">
                      {r.checkedAt
                        ? t('conn.checked', { when: app.i18n.relative(r.checkedAt) })
                        : t('conn.neverChecked')}
                    </p>
                  {/if}
                </div>
                <div class="actions">
                  {#if r.action && r.action.kind !== 'open'}
                    {@const action = r.action}
                    <Button
                      size="sm"
                      variant={action.kind === 'login' ? 'primary' : 'secondary'}
                      disabled={r.state === 'checking'}
                      onclick={() => run(action)}>{tk(`conn.action.${action.kind}`)}</Button
                    >
                  {/if}
                  <Button
                    size="sm"
                    variant="ghost"
                    aria-label={t('conn.details', { name: name(r) })}
                    onclick={() => app.openSettings(r.page)}>{t('conn.action.open')}</Button
                  >
                </div>
              </li>
            {/each}
          </ul>
        {/if}
      </section>
    {/each}
  {/if}
</section>

<style>
  .conn {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
  }
  .intro,
  .hint {
    font-size: var(--alfa-font-size-sm);
  }
  .hint,
  .when,
  .empty {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-3);
    padding: var(--alfa-space-3) var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
  }
  .summary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-2);
    font-weight: var(--alfa-weight-semibold);
  }
  .counts {
    color: var(--alfa-color-text-muted);
    font-weight: normal;
    font-size: var(--alfa-font-size-sm);
  }
  .fail {
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border-left: 3px solid var(--alfa-color-error);
    font-size: var(--alfa-font-size-sm);
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
  }
  h4 {
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  .rows {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    list-style: none;
  }
  .row {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: var(--alfa-space-3);
    padding: var(--alfa-space-3) var(--alfa-space-4);
  }
  .row + .row {
    border-top: 1px solid var(--alfa-color-border);
  }
  .dot {
    display: inline-block;
    flex: none;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--alfa-color-text-subtle);
  }
  .dot[data-state='ok'] {
    background: var(--alfa-color-success);
  }
  .dot[data-state='attention'] {
    background: var(--alfa-color-warning);
  }
  .dot[data-state='checking'] {
    background: var(--alfa-color-info);
  }
  .dot[data-state='off'] {
    background: transparent;
    box-shadow: inset 0 0 0 2px var(--alfa-color-border-strong);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .top {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--alfa-space-2);
  }
  .name {
    font-weight: var(--alfa-weight-semibold);
    overflow-wrap: anywhere;
  }
  .state {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .text {
    font-size: var(--alfa-font-size-sm);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--alfa-space-2);
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  @media (max-width: 640px) {
    .row {
      grid-template-columns: auto 1fr;
    }
    .actions {
      grid-column: 2;
      justify-content: flex-start;
    }
  }
</style>
