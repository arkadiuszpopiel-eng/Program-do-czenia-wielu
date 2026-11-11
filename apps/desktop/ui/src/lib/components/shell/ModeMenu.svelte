<!--
  Menu „Tryb pracy” (decyzja właściciela 27.10, od 29.10 w pasku pisania, od 3.11 pięć trybów na
  zasadzie Claude Code i Codex — plan 50): Plan, Ręcznie, Akceptuj edycje,
  Automatycznie, Pełny dostęp. Tryb
  może obowiązywać w tej rozmowie lub globalnie; podniesienie potwierdzasz w oknie Brokera.
  Zaznaczony jest poziom wybranego zakresu odczytany z Brokera, a komunikat o zmianie pojawia
  się dopiero, gdy Broker ją przyjął. Klawisze 1–5 wybierają tryb, strzałki przesuwają zaznaczenie.
  Na dole czerwony przełącznik „Max dostęp” (plan 50 §8.1). „Plan” (L0) obowiązuje tylko otwartą
  rozmowę (Z.5); w rozmowie w trybie Plan wybór innego trybu zmienia poziom tej rozmowy.
-->
<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import Lock from '@lucide/svelte/icons/lock';
  import type { AutonomyLevel } from '../../api/types';
  import { modeItems } from '../../logic/work-mode';
  import { useApp } from '../../state/context';
  import { behindLock as locked, modeView, switchMode } from '../../state/mode-switch';
  import MaxAccessSwitch from './MaxAccessSwitch.svelte';

  interface Props {
    /** Zamyka menu po wyborze albo przejściu do Ustawień. */
    onclose: () => void;
  }
  let { onclose }: Props = $props();

  const app = useApp();
  const { t, tk } = app.i18n;
  const en = $derived(app.i18n.locale === 'en');
  let scope = $state<'global' | 'session'>('global');
  const session = $derived(app.sessions.active);
  // Bieżący tryb i zakres zmiany: Plan i „do końca rozmowy” dotyczą tej rozmowy (Z.5, Z.7).
  const view = $derived(modeView(app, scope));
  const current = $derived(view.current);
  const here = $derived(view.here);
  $effect(() => {
    if (view.planning || here) scope = 'session';
  });
  // Rozmowa z własnym poziomem (Ustawienia → Uprawnienia) — menu zmienia tryb globalny.
  const own = $derived(
    scope === 'global' &&
      session &&
      !view.planning &&
      !here &&
      app.autonomy.global &&
      session.autonomy !== app.autonomy.global
      ? session.autonomy
      : null,
  );
  const items = $derived(modeItems(current));
  // Świeży odczyt przy otwarciu — zgoda mogła przyjść z okna Brokera, gdy menu było zamknięte.
  $effect(() => {
    void app.autonomy.load(app.client.permissions);
  });
  let list = $state<HTMLDivElement | null>(null);

  $effect(() => {
    const checked = list?.querySelector<HTMLElement>('[aria-checked="true"]');
    (checked ?? list?.querySelector<HTMLElement>('[role="menuitemradio"]'))?.focus();
  });

  const behindLock = (level: AutonomyLevel) => locked(app, view, level);

  function choose(level: AutonomyLevel) {
    if (!behindLock(level)) onclose();
    void switchMode(app, view, level);
  }

  function keydown(event: KeyboardEvent) {
    const digit = Number(event.key);
    const picked = Number.isInteger(digit) ? items[digit - 1] : undefined;
    if (picked !== undefined) {
      event.preventDefault();
      void choose(picked);
      return;
    }
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    event.preventDefault();
    const all = [...(list?.querySelectorAll<HTMLElement>('[role="menuitemradio"]') ?? [])];
    const at = all.indexOf(document.activeElement as HTMLElement);
    const step = event.key === 'ArrowDown' ? 1 : -1;
    all[(at + step + all.length) % all.length]?.focus();
  }

  function settings() {
    onclose();
    app.openSettings('permissions');
  }
</script>

<div class="menu">
  <h3 class="h" id="mode-title">{t('mode.title')}</h3>
  <fieldset class="scope">
    <legend>{en ? 'Apply mode to' : 'Zakres trybu'}</legend>
    <label
      ><input
        type="radio"
        name="mode-scope"
        value="session"
        bind:group={scope}
        disabled={!session}
      />{en ? 'This conversation' : 'Ta rozmowa'}</label
    >
    <label
      ><input
        type="radio"
        name="mode-scope"
        value="global"
        bind:group={scope}
        disabled={view.planning || here}
      />{en ? 'All conversations' : 'Wszystkie rozmowy'}</label
    >
  </fieldset>
  {#if view.planning || here}<p class="note">
      {en ? 'This change applies to the current conversation.' : 'Zmiana dotyczy bieżącej rozmowy.'}
    </p>{/if}
  <div
    class="list"
    role="menu"
    tabindex="-1"
    aria-labelledby="mode-title"
    bind:this={list}
    onkeydown={keydown}
  >
    {#each items as level, i (level)}
      <button
        type="button"
        role="menuitemradio"
        aria-checked={level === current}
        class="item"
        class:full={level === 'L4'}
        class:locked={behindLock(level)}
        aria-disabled={behindLock(level) ? true : undefined}
        onclick={() => void choose(level)}
      >
        <span class="text">
          <span class="name">{tk(`mode.${level}.name`)}</span>
          <span class="desc"
            >{behindLock(level) ? t('child.askParent') : tk(`mode.${level}.desc`)}</span
          >
        </span>
        <span class="mark" aria-hidden="true">
          {#if level === current}<Check
              size={16}
              strokeWidth={2}
            />{:else if behindLock(level)}<Lock size={14} strokeWidth={2} />{/if}
        </span>
        <span class="key" aria-hidden="true">{i + 1}</span>
      </button>
    {/each}
  </div>
  {#if app.autonomy.locked && !here}<p class="note own">{t('child.lockedNote')}</p>{/if}
  {#if own}<p class="note own">{t('mode.sessionOwn', { mode: tk(`mode.${own}.name`) })}</p>{/if}
  <MaxAccessSwitch />
  <p class="note">{t('mode.note')}</p>
  <button type="button" class="more" onclick={settings}>{t('mode.more')}</button>
</div>

<style>
  .scope {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
    border: 0;
    margin: 0;
    padding: var(--alfa-space-2);
  }
  .scope legend {
    font-size: var(--alfa-font-size-xs);
    color: var(--alfa-color-text-muted);
  }
  .scope label {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-1);
    font-size: var(--alfa-font-size-xs);
  }
  .menu {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    padding: var(--alfa-space-2);
    font-size: var(--alfa-font-size-sm);
    white-space: normal;
  }
  .h {
    margin: 0;
    padding: var(--alfa-space-1) var(--alfa-space-2);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
  }
  /* Lista leży w okienku belki: bez tła i cienia, które styl Szkło daje samodzielnym `[role=menu]`. */
  .menu .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    outline: none;
    background: transparent;
    box-shadow: none;
  }
  .item {
    display: grid;
    grid-template-columns: 1fr 20px 16px;
    align-items: center;
    gap: var(--alfa-space-2);
    width: 100%;
    padding: var(--alfa-space-2);
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text);
    text-align: left;
  }
  .item:hover,
  .item:focus-visible {
    background: var(--alfa-color-surface2);
    outline: none;
  }
  .item:focus-visible {
    box-shadow: inset 0 0 0 var(--alfa-size-focus-ring) var(--alfa-color-focus);
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .name {
    font-weight: var(--alfa-weight-semibold);
  }
  .full .name {
    color: var(--alfa-color-warning);
  }
  /* Profil Dziecko: tryb za kłódką rodzica — wyciszony, z kłódką zamiast znacznika. */
  .locked .name,
  .locked .mark {
    color: var(--alfa-color-text-muted);
  }
  .desc,
  .note {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    line-height: 1.4;
  }
  .mark {
    display: inline-flex;
    color: var(--alfa-color-accent);
  }
  .key {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  .note {
    margin: var(--alfa-space-1) var(--alfa-space-2) 0;
  }
  .note.own {
    padding: var(--alfa-space-2);
    border-radius: var(--alfa-radius-control);
    background: color-mix(in srgb, var(--alfa-color-warning) 12%, transparent);
    color: var(--alfa-color-text);
  }
  .more {
    align-self: flex-start;
    margin: 0 var(--alfa-space-1);
    padding: var(--alfa-space-1);
    border: 0;
    background: transparent;
    color: var(--alfa-color-accent);
    font-size: var(--alfa-font-size-xs);
    text-decoration: underline;
  }
  .more:focus-visible {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
  }
</style>
