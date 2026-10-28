<!--
  Objaśnienie stanu z paska tytułu: połączenie (i kolejka wiadomości bez sieci), profil modeli,
  tryb pracy — po ludzku. Profil zmienia się tu na miejscu, tryb — w menu „Tryb pracy”
  (właściciel 27.10: klik, który tylko przenosi do Ustawień, „nie ma sensu”).
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import { useApp } from '../../state/context';

  interface Props {
    /** Wywoływane przed przejściem do Ustawień (zamyka okienko). */
    onnavigate: () => void;
    /** Otwiera menu „Tryb pracy” zamiast tego okienka. */
    onmode?: () => void;
  }
  let { onnavigate, onmode }: Props = $props();

  const app = useApp();
  const { t, tk } = app.i18n;
  const session = $derived(app.sessions.active);
  // Wybór z tego okienka widać od razu, zanim rdzeń odświeży stan.
  const saved = $derived(
    app.str('models.default_profile', app.system?.profile ?? session?.profile ?? 'hybrid'),
  );
  const PROFILES = ['local', 'hybrid', 'cloud'] as const;
  const noKeys = $derived(app.system?.keys_configured === false);
  // Bez kluczy działa tylko profil lokalny — zaznaczony jest ten, który działa (jak w pasku pisania).
  const profileId = $derived(noKeys ? 'local' : saved);
  // Grupa wyboru wg wzorca ARIA: jeden przystanek Tab, strzałki przesuwają wybór (bez wyłączonych).
  const usable = $derived(PROFILES.filter((p) => p === 'local' || !noKeys));
  const stop = $derived(usable.find((p) => p === profileId) ?? usable[0]);
  function arrows(event: KeyboardEvent) {
    const dir = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];
    if (!dir) return;
    event.preventDefault();
    const at = usable.indexOf(stop ?? 'local');
    const next = usable[(at + dir + usable.length) % usable.length] ?? 'local';
    void app.setSetting('models.default_profile', next);
    const group = (event.currentTarget as HTMLElement).parentElement;
    group?.querySelector<HTMLElement>(`[data-profile="${next}"]`)?.focus();
  }
  const level = $derived(session?.autonomy ?? 'L3');
  const offline = $derived(app.system?.online === false);
  const queued = $derived(app.system?.queued_messages ?? 0);

  function go(page: string) {
    onnavigate();
    app.openSettings(page);
  }
</script>

<div class="details">
  <h3 class="h">{t('status.title')}</h3>
  <dl>
    <dt>{t('status.connection')}</dt>
    <dd>
      <strong class:warn={offline}>{t(offline ? 'status.offline' : 'status.online')}</strong>
      <span class="desc">
        {offline ? t('status.offlineDesc', { n: queued }) : t('status.onlineDesc')}
      </span>
    </dd>
    <dt>{t('status.profile')}</dt>
    <dd>
      <div class="seg" role="radiogroup" aria-label={t('status.profile')}>
        {#each PROFILES as p (p)}
          <button
            type="button"
            role="radio"
            aria-checked={profileId === p}
            tabindex={p === stop ? 0 : -1}
            data-profile={p}
            disabled={p !== 'local' && noKeys}
            onkeydown={arrows}
            onclick={() => void app.setSetting('models.default_profile', p)}
            >{tk(`profile.${p}`)}</button
          >
        {/each}
      </div>
      <span class="desc">{tk(`status.profileDesc.${profileId}`)}</span>
      {#if noKeys && saved !== 'local'}
        <span class="desc warn">{t('status.noKeys')}</span>
      {/if}
    </dd>
    <dt>{t('mode.title')}</dt>
    <dd>
      <strong>{tk(`mode.${level}.name`)} <span class="lvl">({level})</span></strong>
      <span class="desc">{tk(`mode.${level}.desc`)}</span>
    </dd>
  </dl>
  <div class="actions">
    {#if onmode}
      <Button size="sm" variant="secondary" onclick={onmode}>{t('status.changeMode')}</Button>
    {:else}
      <Button size="sm" variant="secondary" onclick={() => go('permissions')}
        >{t('status.openPermissions')}</Button
      >
    {/if}
    <Button size="sm" variant="secondary" onclick={() => go('providers')}
      >{t('status.openAccounts')}</Button
    >
  </div>
</div>

<style>
  .details {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    padding: var(--alfa-space-3);
    font-size: var(--alfa-font-size-sm);
    white-space: normal;
  }
  .h {
    margin: 0;
    font-size: var(--alfa-font-size-md);
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--alfa-space-2) var(--alfa-space-3);
    margin: 0;
  }
  dt {
    color: var(--alfa-color-text-muted);
  }
  dd {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
  }
  .desc {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    line-height: 1.4;
  }
  .warn {
    color: var(--alfa-color-warning);
  }
  .lvl {
    color: var(--alfa-color-text-muted);
    font-weight: normal;
  }
  /* Profil jako trzy przyciski obok siebie — wybór od razu, bez przechodzenia do Ustawień. */
  .seg {
    display: inline-flex;
    align-self: flex-start;
    padding: 2px;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface2);
  }
  .seg button {
    min-height: 26px;
    padding: 0 var(--alfa-space-2);
    border: 0;
    border-radius: calc(var(--alfa-radius-control) - 2px);
    background: transparent;
    color: var(--alfa-color-text-muted);
    font: inherit;
    font-size: var(--alfa-font-size-xs);
    cursor: pointer;
  }
  /* Obwódka: w jasnym Szkle tło wybranego i grupy mają prawie ten sam kolor. */
  .seg button[aria-checked='true'] {
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
    font-weight: var(--alfa-weight-semibold);
    box-shadow: var(--alfa-shadow-1);
    outline: 1px solid var(--alfa-color-border);
    outline-offset: -1px;
  }
  .seg button:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }
  .seg button:focus-visible {
    outline: 2px solid var(--alfa-color-focus);
    outline-offset: 1px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
  }
</style>
