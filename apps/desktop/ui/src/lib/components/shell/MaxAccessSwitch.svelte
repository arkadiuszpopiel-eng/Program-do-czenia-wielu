<!--
  „Max dostęp” na dole menu „Tryb pracy” (decyzja właściciela 3.11, plan 50 §8.1): czerwony
  przełącznik z poświatą i zasięgiem — modele lokalne, w chmurze albo wszystkie. Włączony znaczy:
  agentki na modelach z zasięgu nie pytają o nic (zostają STOP, dziennik i twarde blokady Jądra).
  Włączenie i zmianę zasięgu potwierdzasz raz w oknie Sędziego Cieni; wyłączenie działa od razu.
-->
<script lang="ts">
  import Lock from '@lucide/svelte/icons/lock';
  import Zap from '@lucide/svelte/icons/zap';
  import type { MaxAccess } from '../../api/types-hub';
  import { showError } from '../../state/attempt';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  const SCOPES: readonly MaxAccess[] = ['local', 'cloud', 'all'];

  const on = $derived(app.autonomy.maxAccess);
  // Wybrany zasięg: włączony z Sędziego, a przy wyłączonym — ostatni kliknięty (domyślnie lokalne).
  let picked = $state<MaxAccess>('local');
  const scope = $derived(on ?? picked);
  let busy = $state(false);

  async function request(next: MaxAccess | null) {
    if (busy) return;
    busy = true;
    try {
      const result = await app.client.permissions.requestMaxAccess(next);
      if (result.status === 'opened_broker') {
        const where = t(`maxAccess.scope.${next ?? scope}`);
        app.toasts.show({ kind: 'info', message: t('maxAccess.requested', { where }) });
        return;
      }
      await app.autonomy.load(app.client.permissions);
      app.toasts.show({ kind: 'success', message: t('maxAccess.off') });
    } catch (error) {
      showError(app.toasts, error);
    } finally {
      busy = false;
    }
  }

  // Profil Dziecko z zamkniętą kłódką: włączenie otwiera tylko PIN rodzica (wyłączyć wolno zawsze).
  const locked = $derived(app.autonomy.locked);

  function toggle() {
    if (locked && !on) {
      app.toasts.show({ kind: 'info', message: t('child.lockedNote') });
      return;
    }
    void request(on ? null : scope);
  }

  function choose(next: MaxAccess) {
    picked = next;
    // Zmiana zasięgu włączonego przełącznika to nowa prośba do Sędziego.
    if (on && on !== next) void request(next);
  }

  function keydown(event: KeyboardEvent) {
    if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft') return;
    event.preventDefault();
    const step = event.key === 'ArrowRight' ? 1 : -1;
    const next = SCOPES[(SCOPES.indexOf(scope) + step + SCOPES.length) % SCOPES.length];
    if (next === undefined) return;
    choose(next);
    // `currentTarget` znika po obsłudze zdarzenia — grupę bierzemy od razu.
    const group = event.currentTarget as HTMLElement;
    requestAnimationFrame(() => group.querySelector<HTMLElement>('[aria-checked="true"]')?.focus());
  }
</script>

<section class="max" class:on={on !== null} aria-labelledby="max-title">
  <div class="row">
    <span class="title-wrap">
      <Zap size={16} strokeWidth={2} aria-hidden="true" />
      <span class="title" id="max-title">{t('maxAccess.name')}</span>
      {#if locked && !on}<Lock size={14} strokeWidth={2} aria-hidden="true" />{/if}
    </span>
    <button
      type="button"
      role="switch"
      class="switch"
      aria-checked={on !== null}
      aria-disabled={locked && !on ? true : undefined}
      aria-labelledby="max-title"
      aria-describedby="max-desc"
      aria-busy={busy}
      onclick={toggle}
    >
      <span class="knob" aria-hidden="true"></span>
    </button>
  </div>
  <p class="desc" id="max-desc">
    {on
      ? t('maxAccess.onDesc', { where: t(`maxAccess.scope.${on}`) })
      : locked
        ? t('child.askParent')
        : t('maxAccess.desc')}
  </p>
  <div
    class="scopes"
    role="radiogroup"
    tabindex="-1"
    aria-label={t('maxAccess.scopeLabel')}
    onkeydown={keydown}
  >
    {#each SCOPES as s (s)}
      <button
        type="button"
        role="radio"
        class="scope"
        aria-checked={s === scope}
        tabindex={s === scope ? 0 : -1}
        onclick={() => choose(s)}>{t(`maxAccess.scope.${s}`)}</button
      >
    {/each}
  </div>
  <!-- Co obejmuje zasięg (plan 50 §8.2: działania bez modelu tylko „Wszystkie”). -->
  <p class="desc scope-hint">{t(`maxAccess.hint.${scope}`)}</p>
</section>

<style>
  .max {
    --max: var(--alfa-color-error);
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    margin-top: var(--alfa-space-1);
    padding: var(--alfa-space-2);
    border-top: 1px solid var(--alfa-color-border);
  }
  .max.on {
    border: 1px solid color-mix(in srgb, var(--max) 55%, transparent);
    border-radius: var(--alfa-radius-control);
    background: color-mix(in srgb, var(--max) 8%, transparent);
    box-shadow: 0 0 14px color-mix(in srgb, var(--max) 30%, transparent);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-2);
  }
  .title-wrap {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    /* Czerwień o ton bliżej tekstu — kontrast ≥ 4,5 w każdym stylu i motywie. */
    color: color-mix(in srgb, var(--max) 80%, var(--alfa-color-text));
  }
  .title {
    font-weight: var(--alfa-weight-semibold);
    letter-spacing: 0.02em;
    text-transform: uppercase;
    text-shadow: 0 0 8px color-mix(in srgb, var(--max) 55%, transparent);
  }
  .on .title {
    animation: glow 2.4s ease-in-out infinite;
  }
  @keyframes glow {
    50% {
      text-shadow: 0 0 14px color-mix(in srgb, var(--max) 85%, transparent);
    }
  }
  .switch {
    position: relative;
    flex: none;
    width: 38px;
    height: 22px;
    padding: 0;
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface2);
    transition: background 0.15s ease;
  }
  .switch[aria-checked='true'] {
    border-color: var(--max);
    background: var(--max);
    box-shadow: 0 0 10px color-mix(in srgb, var(--max) 60%, transparent);
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--alfa-color-text-muted);
    transition: transform 0.15s ease;
  }
  .switch[aria-checked='true'] .knob {
    transform: translateX(16px);
    background: var(--alfa-color-text-on-accent);
  }
  .switch:focus-visible,
  .scope:focus-visible {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
    outline-offset: 2px;
  }
  .desc {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    line-height: 1.4;
  }
  .on .desc {
    color: var(--alfa-color-text);
  }
  .scope-hint {
    margin-top: var(--alfa-space-1);
  }
  .scopes {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 2px;
    padding: 2px;
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface2);
    outline: none;
  }
  .scope {
    padding: var(--alfa-space-1) var(--alfa-space-2);
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .scope:hover {
    color: var(--alfa-color-text);
  }
  .scope[aria-checked='true'] {
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
    font-weight: var(--alfa-weight-semibold);
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.15);
  }
  /* Czerwień tylko w obwódce — napis zostaje w kolorze tekstu (kontrast w Szkle ciemnym). */
  .on .scope[aria-checked='true'] {
    box-shadow:
      inset 0 0 0 1px color-mix(in srgb, var(--max) 70%, transparent),
      0 0 8px color-mix(in srgb, var(--max) 35%, transparent);
  }
  @media (prefers-reduced-motion: reduce) {
    .on .title {
      animation: none;
    }
    .switch,
    .knob {
      transition: none;
    }
  }
</style>
