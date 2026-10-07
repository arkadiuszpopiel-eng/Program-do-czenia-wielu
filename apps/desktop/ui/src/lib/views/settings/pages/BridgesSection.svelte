<!--
  Modele i dostawcy → Mosty CLI: karty zgodności tras (stan zielona / szara / zabroniona, wpis
  nieświeży, wersja wykryta vs przypięta, regulamin i źródła, data weryfikacji), wyłącznik trasy,
  jawna zgoda na uruchomienia z harmonogramu (z ostrzeżeniem) i „Zaloguj w terminalu" — wbudowany
  terminal z profilem logowania CLI (gest użytkownika; logowanie wykonuje człowiek) albo polecenie
  do skopiowania. Alfa nie czyta tokenów CLI.
-->
<script lang="ts">
  import { Button, Checkbox, Switch, TextField } from '@alfa/ui-kit';
  import { errorText } from '../../../api/command-error';
  import type { BridgeCard, BridgeLogin } from '../../../api/types-tasks';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import { loginProfile } from '../../../logic/work';
  import { showError } from '../../../state/attempt';
  import { useApp } from '../../../state/context';

  const app = useApp();
  const { t, tk } = app.i18n;
  let cards = $state<readonly BridgeCard[]>([]);
  let refreshing = $state(false);
  let loaded = $state(false);
  let loadError = $state<string | null>(null);
  let logins = $state<Record<string, BridgeLogin>>({});

  async function load(refresh = false) {
    refreshing = refresh;
    try {
      cards = await app.client.bridges.list(refresh);
      loaded = true;
      loadError = null;
    } catch (error) {
      loadError = errorText(error);
    } finally {
      refreshing = false;
    }
  }

  $effect(() => {
    void load();
  });

  function replace(card: BridgeCard) {
    cards = cards.map((c) => (c.route_id === card.route_id ? card : c));
  }

  async function run(action: () => Promise<BridgeCard>) {
    try {
      replace(await action());
    } catch (error) {
      showError(app.toasts, error);
      // Przełącznik / pole wyboru same zmieniają swój stan; nowe obiekty przywracają stan rdzenia.
      cards = cards.map((c) => ({ ...c }));
    }
  }

  async function login(bridge: string) {
    try {
      const result = await app.client.bridges.openLogin(bridge);
      logins = { ...logins, [bridge]: result };
    } catch (error) {
      showError(app.toasts, error);
    }
  }

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      app.toasts.show({ kind: 'success', message: t('bridges.copied') });
    } catch (error) {
      showError(app.toasts, error);
    }
  }
</script>

<section class="bridges" aria-labelledby="br-title">
  <div class="head">
    <div>
      <h3 id="br-title">{t('bridges.title')}</h3>
      <p class="desc">{t('bridges.intro')}</p>
    </div>
    <Button size="sm" variant="secondary" loading={refreshing} onclick={() => load(true)}
      >{t('bridges.refresh')}</Button
    >
  </div>
  {#if loadError}
    <LoadFailed error={loadError} onretry={() => void load()} />
  {:else if loaded && cards.length === 0}
    <p class="meta">{t('bridges.empty')}</p>
  {/if}
  <ul class="cards" aria-label={t('bridges.list')}>
    {#each cards as card (card.route_id)}
      {@const signin = card.bridge ? logins[card.bridge] : undefined}
      <li class="card" data-status={card.status}>
        <div class="title">
          <h4 id={`br-${card.route_id}`}>{card.name}</h4>
          <span class="status">{tk(`bridges.status.${card.status}`)}</span>
        </div>
        <p class="meta">
          {card.provider} · {t('bridges.mode', { mode: card.mode })}{#if card.verified_at}
            · {t('bridges.verified', { date: card.verified_at })}{/if}
        </p>
        {#if card.stale}<p class="note">{t('bridges.stale')}</p>{/if}
        {#if !card.bridge}
          <p class="note">{t('bridges.registryOnly')}</p>
        {:else if !card.detected}
          <p class="note">{t('bridges.notDetected', { program: card.program ?? '' })}</p>
        {:else}
          <dl class="grid">
            <dt>{t('bridges.detected')}</dt>
            <dd data-selectable>{card.version ?? '—'} · {card.path}</dd>
            <dt>{t('bridges.pinned')}</dt>
            <dd>{card.pinned.length ? card.pinned.join(', ') : t('bridges.pinnedNone')}</dd>
            {#if card.registry_pin}
              <dt>{t('bridges.registryPin')}</dt>
              <dd>{card.registry_pin}</dd>
            {/if}
          </dl>
          <p class={card.version_ok ? 'ok' : 'warn'}>
            {card.version_ok ? t('bridges.versionOk') : t('bridges.versionBad')}
          </p>
        {/if}
        {#if card.sources.length}
          <details>
            <summary>{t('bridges.sources')}</summary>
            <ul class="plain">
              {#each card.sources as s (s.url)}
                <li>
                  <span class="url" data-selectable>{s.url}</span>
                  <Button size="sm" variant="ghost" onclick={() => copy(s.url)}
                    >{t('bridges.copyUrl')}</Button
                  >
                  {#if s.quote}<q>{s.quote}</q>{/if}
                </li>
              {/each}
            </ul>
            {#if card.allowed.length}<p>{t('bridges.allowed')}: {card.allowed.join('; ')}</p>{/if}
            {#if card.forbidden.length}<p>
                {t('bridges.forbidden')}: {card.forbidden.join('; ')}
              </p>{/if}
          </details>
        {/if}
        <div class="row">
          <span>{t('bridges.enabled')}</span>
          <Switch
            checked={card.enabled}
            disabled={!card.can_enable}
            label={`${t('bridges.enabled')}: ${card.name}`}
            onchange={(on) => run(() => app.client.bridges.setEnabled(card.route_id, on))}
          />
        </div>
        {#if card.bridge}
          {@const bridge = card.bridge}
          <div class="actions">
            {#if card.detected && card.version && !card.version_ok}
              <Button
                size="sm"
                variant="secondary"
                onclick={() => run(() => app.client.bridges.pin(bridge, card.version))}
                >{t('bridges.pin')}</Button
              >
            {:else if card.pinned.length}
              <Button
                size="sm"
                variant="ghost"
                onclick={() => run(() => app.client.bridges.pin(bridge, null))}
                >{t('bridges.unpin')}</Button
              >
            {/if}
            {#if card.login_command}
              {@const profile = loginProfile(bridge)}
              {#if profile}
                <Button size="sm" variant="primary" onclick={() => app.work.openTerminal(profile)}
                  >{t('bridges.loginTerminal')}</Button
                >
              {/if}
              <Button size="sm" variant="secondary" onclick={() => login(bridge)}
                >{t('bridges.login')}</Button
              >
            {/if}
          </div>
          {#if signin}
            <div class="login" role="status">
              <p>{signin.opened ? t('bridges.loginHint') : t('bridges.loginNoTerminal')}</p>
              <code data-selectable>{signin.command}</code>
              <Button size="sm" variant="ghost" onclick={() => copy(signin.command)}
                >{t('bridges.copy')}</Button
              >
            </div>
          {/if}
          <fieldset class="schedule" disabled={!card.can_enable}>
            <legend>{t('bridges.schedule')}</legend>
            <p class="warn">{t('bridges.scheduleWarn')}</p>
            <Checkbox
              label={t('bridges.schedule')}
              checked={card.schedule_per_day > 0}
              onchange={(on) => run(() => app.client.bridges.setSchedule(bridge, on ? 2 : 0))}
            />
            {#if card.schedule_per_day > 0}
              <TextField
                label={t('bridges.perDay')}
                type="number"
                min="1"
                max="24"
                value={card.schedule_per_day}
                onchange={(e) =>
                  run(() =>
                    app.client.bridges.setSchedule(
                      bridge,
                      Number((e.currentTarget as HTMLInputElement).value) || 0,
                    ),
                  )}
              />
            {/if}
          </fieldset>
        {/if}
      </li>
    {/each}
  </ul>
</section>

<style>
  .bridges {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    margin-top: var(--alfa-space-6);
  }
  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--alfa-space-3);
  }
  h3 {
    font-size: var(--alfa-font-size-md);
  }
  h4 {
    font-size: var(--alfa-font-size-sm);
  }
  .desc {
    font-size: var(--alfa-font-size-sm);
  }
  .cards {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-left-width: 4px;
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    font-size: var(--alfa-font-size-sm);
  }
  .card[data-status='green'] {
    border-left-color: var(--alfa-color-success);
  }
  .card[data-status='forbidden'] {
    border-left-color: var(--alfa-color-error);
  }
  .title,
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-2);
  }
  .status {
    font-weight: var(--alfa-weight-semibold);
  }
  .grid {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--alfa-space-1) var(--alfa-space-3);
    margin: 0;
  }
  dt {
    color: var(--alfa-color-text-muted);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .meta {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .note,
  .warn {
    font-size: var(--alfa-font-size-xs);
  }
  .note {
    font-style: italic;
  }
  .warn {
    color: var(--alfa-color-text);
  }
  .ok {
    color: var(--alfa-color-success);
    font-size: var(--alfa-font-size-xs);
  }
  .plain {
    margin: var(--alfa-space-1) 0;
    padding-left: var(--alfa-space-4);
  }
  .url {
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-xs);
    overflow-wrap: anywhere;
  }
  q {
    display: block;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
  }
  .login {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-2);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface2);
  }
  code {
    font-family: var(--alfa-font-mono);
  }
  .schedule {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: 0;
    padding: var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
  }
  legend {
    font-weight: var(--alfa-weight-semibold);
  }
</style>
