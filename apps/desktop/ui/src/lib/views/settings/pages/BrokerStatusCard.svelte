<!--
  Stan Brokera (Ustawienia → Uprawnienia i bezpieczeństwo; ADR 0003): tryb (usługa na osobnym
  koncie / tryb przenośny / w procesie / brak), łącze, okno zatwierdzeń, kill-switch, izolacja
  i wyjaśnienie. Tylko odczyt — instalację usługi (jednorazowy UAC, osobne konto) wykonuje
  administrator wg przewodnika (bramka ludzka #10), a zatwierdzenia — wyłącznie okno Brokera.
-->
<script lang="ts">
  import ShieldAlert from '@lucide/svelte/icons/shield-alert';
  import ShieldCheck from '@lucide/svelte/icons/shield-check';
  import { useApp } from '../../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  const view = $derived(app.broker.view);
  const ok = $derived(view !== null && view.state === 'connected' && view.mode !== 'unavailable');
</script>

{#if view}
  <section class="card" aria-labelledby="broker-title">
    <h3 id="broker-title">{t('broker.title')}</h3>
    <p class="mode">
      <span class="icon {ok ? 'ok' : 'bad'}" aria-hidden="true">
        {#if ok}<ShieldCheck size={16} strokeWidth={1.5} />{:else}<ShieldAlert
            size={16}
            strokeWidth={1.5}
          />{/if}
      </span>
      <span>
        <strong>{t(`broker.mode.${view.mode}`)}</strong>
        <span class="state" role="status" aria-live="polite"
          >· {t(`broker.state.${view.state}`)}</span
        >
      </span>
    </p>
    {#if view.detail}<p class="desc">{app.i18n.text(view.detail)}</p>{/if}
    <ul class="facts">
      <li>{t(view.approval_window ? 'broker.window.on' : 'broker.window.off')}</li>
      <li>{t(view.watchdog ? 'broker.watchdog.on' : 'broker.watchdog.off')}</li>
      <li>{t(view.isolated ? 'broker.isolation.full' : 'broker.isolation.weak')}</li>
    </ul>
    {#if !view.isolated}<p class="desc">{t('broker.service.hint')}</p>{/if}
  </section>
{/if}

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin-bottom: var(--alfa-space-4);
    padding: var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
  }
  h3 {
    font-size: var(--alfa-font-size-md);
  }
  .mode {
    display: flex;
    align-items: flex-start;
    gap: var(--alfa-space-2);
  }
  .icon {
    display: inline-flex;
    margin-top: 2px;
  }
  .icon.ok {
    color: var(--alfa-color-success);
  }
  .icon.bad {
    color: var(--alfa-color-error);
  }
  .state {
    color: var(--alfa-color-text-muted);
  }
  .desc,
  .facts {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .facts {
    margin: 0;
    padding-left: var(--alfa-space-4);
  }
</style>
