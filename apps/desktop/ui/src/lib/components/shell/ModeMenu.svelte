<!--
  Menu „Tryb pracy” z paska tytułu (decyzja właściciela 27.10): cztery tryby jak w Claude Code, po
  polsku — Tylko podgląd, Pytaj o wszystko, Automatycznie, Pełna automatyka. Tryb obowiązuje we
  wszystkich rozmowach; podniesienie potwierdzasz raz w oknie Brokera i zostaje po restarcie.
  Klawisze 1–4 wybierają tryb, strzałki przesuwają zaznaczenie.
-->
<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import type { AutonomyLevel } from '../../api/types';
  import { modeItems, requestMode } from '../../logic/work-mode';
  import { showError } from '../../state/attempt';
  import { useApp } from '../../state/context';

  interface Props {
    /** Poziom obowiązujący teraz (rozmowa albo globalny). */
    current: AutonomyLevel;
    /** Zamyka menu po wyborze albo przejściu do Ustawień. */
    onclose: () => void;
  }
  let { current, onclose }: Props = $props();

  const app = useApp();
  const { t, tk } = app.i18n;
  const items = $derived(modeItems(current));
  let list = $state<HTMLDivElement | null>(null);

  $effect(() => {
    const checked = list?.querySelector<HTMLElement>('[aria-checked="true"]');
    (checked ?? list?.querySelector<HTMLElement>('[role="menuitemradio"]'))?.focus();
  });

  async function choose(level: AutonomyLevel) {
    onclose();
    if (level === current) return;
    try {
      await requestMode(level, {
        request: (l) => app.client.permissions.requestLevel(l, null),
        readGlobal: async () => (await app.client.permissions.get(null)).global,
        notify: (event, l) => {
          const name = tk(`mode.${l}.name`);
          const kind = event === 'requested' ? 'info' : 'success';
          app.toasts.show({ kind, message: t(`mode.${event}`, { mode: name }) });
        },
      });
    } catch (error) {
      showError(app.toasts, error);
    }
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
        onclick={() => void choose(level)}
      >
        <span class="text">
          <span class="name">{tk(`mode.${level}.name`)}</span>
          <span class="desc">{tk(`mode.${level}.desc`)}</span>
        </span>
        <span class="mark" aria-hidden="true">
          {#if level === current}<Check size={16} strokeWidth={2} />{/if}
        </span>
        <span class="key" aria-hidden="true">{i + 1}</span>
      </button>
    {/each}
  </div>
  <p class="note">{t('mode.note')}</p>
  <button type="button" class="more" onclick={settings}>{t('mode.more')}</button>
</div>

<style>
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
  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    outline: none;
  }
  .item {
    display: grid;
    grid-template-columns: 1fr 20px 16px;
    align-items: center;
    gap: var(--alfa-space-2);
    width: 100%;
    padding: var(--alfa-space-2);
    border: 0;
    border-radius: var(--alfa-radius-md);
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
