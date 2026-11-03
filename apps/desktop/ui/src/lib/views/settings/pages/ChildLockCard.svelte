<!--
  Profil Dziecko (Ustawienia → Uprawnienia, plan 50 §4.4): PIN rodzica, włączenie profilu, kłódka
  otwierana na czas i zamykana od razu. PIN wpisany tutaj idzie wyłącznie do Sędziego Cieni (on go
  sprawdza i trzyma tylko skrót); pola czyścimy zaraz po wysłaniu, niczego nie zapisujemy w Alfie.
-->
<script lang="ts">
  import { Button, SegmentedControl, TextField } from '@alfa/ui-kit';
  import Lock from '@lucide/svelte/icons/lock';
  import LockOpen from '@lucide/svelte/icons/lock-open';
  import type { ChildLockRequest, ChildLockState } from '../../../api/types-hub';
  import { showError } from '../../../state/attempt';
  import { useApp } from '../../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  const PIN = /^\d{4,8}$/;
  let lock = $state<ChildLockState | null>(null);
  let pin = $state('');
  let oldPin = $state('');
  let newPin = $state('');
  let repeat = $state('');
  let duration = $state<string>('60');
  let busy = $state(false);
  let formError = $state<string | null>(null);

  async function reload() {
    try {
      lock = await app.client.permissions.childLock(null);
    } catch {
      lock = null;
    }
  }

  $effect(() => {
    void app.broker.view;
    void reload();
  });

  function when(ms: number): string {
    const iso = new Date(ms).toISOString();
    return new Date().toDateString() === new Date(ms).toDateString()
      ? app.i18n.time(iso)
      : `${app.i18n.day(iso)}, ${app.i18n.time(iso)}`;
  }

  const until = $derived(
    lock?.open_until_ms
      ? t('child.until.time', { when: when(lock.open_until_ms) })
      : t('child.until.close'),
  );
  const status = $derived(
    !lock
      ? ''
      : !lock.pin_set
        ? t('child.state.none')
        : !lock.enabled
          ? t('child.state.off')
          : lock.open
            ? t('child.state.open', { until })
            : t('child.state.locked'),
  );

  /** Wysyła zmianę do Sędziego; pola PIN-u zawsze czyszczone. */
  async function send(change: ChildLockRequest, done: () => string) {
    busy = true;
    formError = null;
    try {
      lock = await app.client.permissions.childLock(change);
      app.toasts.show({ kind: 'success', message: done() });
      await app.autonomy.load(app.client.permissions);
    } catch (error) {
      showError(app.toasts, error);
      await reload();
    } finally {
      busy = false;
      pin = oldPin = newPin = repeat = '';
    }
  }

  function setPin(event: SubmitEvent) {
    event.preventDefault();
    if (!PIN.test(newPin)) return void (formError = t('child.format'));
    if (newPin !== repeat) return void (formError = t('child.mismatch'));
    const old = lock?.pin_set ? oldPin : null;
    void send({ op: 'set_pin', old, new: newPin }, () => t('child.pinSet'));
  }

  function withPin(event: SubmitEvent) {
    event.preventDefault();
    if (!lock) return;
    const submitter = (event.submitter as HTMLButtonElement | null)?.value;
    if (!PIN.test(pin)) return void (formError = t('child.format'));
    if (submitter === 'open') {
      const minutes = duration === 'close' ? null : Number(duration);
      void send({ op: 'open', pin, minutes }, () => t('child.opened', { until }));
    } else {
      const on = !lock.enabled;
      void send({ op: 'enable', pin, on }, () => t(on ? 'child.enabled' : 'child.disabled'));
    }
  }
</script>

{#if lock}
  <section class="card" aria-labelledby="child-title">
    <h3 id="child-title">{t('child.title')}</h3>
    <p class="desc">{t('child.intro')}</p>
    <p class="state" class:on={lock.enabled} role="status">
      {#if lock.enabled && !lock.open}
        <Lock size={16} strokeWidth={1.75} aria-hidden="true" />
      {:else}
        <LockOpen size={16} strokeWidth={1.75} aria-hidden="true" />
      {/if}
      {status}
    </p>
    {#if lock.blocked_until_ms}
      <p class="warn" role="alert">{t('child.blocked', { when: when(lock.blocked_until_ms) })}</p>
    {:else if lock.pin_set && lock.attempts_left < 5}
      <p class="warn">{t('child.attempts', { n: lock.attempts_left })}</p>
    {/if}

    {#if lock.pin_set}
      <form class="row" onsubmit={withPin}>
        <TextField
          label={t('child.pin')}
          type="password"
          inputmode="numeric"
          maxlength={8}
          bind:value={pin}
        />
        {#if lock.enabled}
          <SegmentedControl
            label={t('child.for')}
            bind:value={duration}
            options={[
              { value: '15', label: t('child.for.15') },
              { value: '60', label: t('child.for.60') },
              { value: 'close', label: t('child.for.close') },
            ]}
          />
        {/if}
        <div class="actions">
          {#if lock.enabled}
            <Button type="submit" value="open" variant="primary" disabled={busy || !pin}
              >{t('child.open')}</Button
            >
          {/if}
          <Button type="submit" value="enable" disabled={busy || !pin}
            >{t(lock.enabled ? 'child.disable' : 'child.enable')}</Button
          >
          {#if lock.enabled && lock.open}
            <Button
              disabled={busy}
              onclick={() => void send({ op: 'close' }, () => t('child.closed'))}
              >{t('child.close')}</Button
            >
          {/if}
        </div>
      </form>
    {/if}

    <form class="row" onsubmit={setPin}>
      {#if lock.pin_set}
        <TextField
          label={t('child.oldPin')}
          type="password"
          inputmode="numeric"
          maxlength={8}
          bind:value={oldPin}
        />
      {/if}
      <TextField
        label={t('child.newPin')}
        type="password"
        inputmode="numeric"
        maxlength={8}
        bind:value={newPin}
      />
      <TextField
        label={t('child.repeat')}
        type="password"
        inputmode="numeric"
        maxlength={8}
        bind:value={repeat}
        error={formError ?? undefined}
      />
      <div class="actions">
        <Button type="submit" disabled={busy || !newPin || (lock.pin_set && !oldPin)}
          >{t(lock.pin_set ? 'child.changePin' : 'child.setPin')}</Button
        >
      </div>
    </form>
  </section>
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
  .state {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    font-weight: var(--alfa-weight-semibold);
  }
  .warn {
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-sm);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--alfa-space-3);
    padding-top: var(--alfa-space-3);
    border-top: 1px solid var(--alfa-color-border);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
  }
</style>
