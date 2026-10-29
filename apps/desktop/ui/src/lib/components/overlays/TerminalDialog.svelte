<!--
  Wbudowany terminal (gest użytkownika): powłoka albo logowanie mostu CLI (`claude`, `codex`).
  Strumień wyłącznie przez kanał `terminal_open` (nie zdarzenia, nie logi); wejście z klawiatury
  tylko tutaj. Escape trafia do programu w terminalu — dialog zamyka przycisk „Zamknij terminal".
  Emulator (@xterm/xterm) ładowany leniwie razem z tym dialogiem.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { Dialog } from 'bits-ui';
  import { Button } from '@alfa/ui-kit';
  import type { TerminalFrame, TerminalSession } from '../../api/types-work';
  import { loginResult } from '../../logic/connections';
  import {
    GUIDE_STEPS,
    b64ToBytes,
    binaryToB64,
    loginBridge,
    loginSignal,
    textToB64,
    type LoginSignal,
  } from '../../logic/work';
  import { showError } from '../../state/attempt';
  import { useApp } from '../../state/context';
  import type { TerminalRequest } from '../../state/work.svelte';
  import { createTerminal, type TerminalHost } from './terminal-host';

  interface Props {
    request: TerminalRequest;
  }

  let { request }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  let container = $state<HTMLDivElement | null>(null);
  let session = $state<TerminalSession | null>(null);
  let exited = $state<number | null | undefined>(undefined);
  let error = $state<string | null>(null);
  /** Zalogowanie wykryte w wyjściu narzędzia (np. „(Google AI Pro)” w nagłówku Antigravity). */
  let detected = $state<LoginSignal | null>(null);
  /** Kroki instrukcji dla tego narzędzia (klucze `term.guide.<profil>.<n>`). */
  const steps = $derived(GUIDE_STEPS[request.profile] ?? 0);
  /** Logowanie do narzędzia dostawcy: okno prowadzi krok po kroku, zamiast zostawiać z terminalem. */
  const login = $derived(loginBridge(request.profile) !== null);
  const title = $derived(
    login
      ? t('term.loginTitle', { name: tk(`term.service.${request.profile}`) })
      : t('term.title', { name: tk(`term.profile.${request.profile}`) }),
  );

  function fail(e: unknown) {
    error = e instanceof Error ? e.message : String(e);
  }

  $effect(() => {
    const el = container;
    if (!el) return;
    const profile = request.profile;
    void request.seq;
    exited = undefined;
    session = null;
    error = null;
    detected = null;
    // Ostatnie znaki wyjścia (bez treści w logach — tylko do wykrycia zalogowania tutaj).
    const decoder = new TextDecoder();
    let tail = '';
    let host: TerminalHost | null = null;
    let id: number | null = null;
    let closed = false;
    // Wejście po kolei: każde wywołanie czeka na poprzednie. Równoległe wywołania pod obciążeniem
    // docierały w innej kolejności („Write” → „Witre”, e2e-app 2026-10-15).
    let queue: Promise<unknown> = Promise.resolve();
    const send = (b64: string) => {
      if (id === null || exited !== undefined) return;
      const target = id;
      queue = queue.then(() => app.client.terminal.input(target, b64)).catch(() => undefined);
    };
    const label = untrack(() => t('term.inputLabel', { name: tk(`term.profile.${profile}`) }));
    host = createTerminal(el, {
      label,
      onData: (data) => send(textToB64(data)),
      onBinary: (data) => send(binaryToB64(data)),
    });
    host.fit();
    const onFrame = (frame: TerminalFrame) => {
      // Spóźniona ramka zamkniętej sesji (po „Spróbuj ponownie”) nie zmienia stanu nowej.
      if (closed) return;
      if (frame.kind === 'output') {
        const bytes = b64ToBytes(frame.data_b64);
        host?.write(bytes);
        if (untrack(() => detected) === null) {
          tail = (tail + decoder.decode(bytes, { stream: true })).slice(-16_384);
          const signal = loginSignal(profile, tail);
          if (signal) detected = signal;
        }
      } else {
        exited = frame.code;
        const note = untrack(() => t('term.exited', { code: frame.code ?? '—' }));
        host?.write(`\r\n${note}\r\n`);
      }
    };
    app.client.terminal.open(profile, host.cols, host.rows, null, onFrame).then((s) => {
      if (closed) {
        void app.client.terminal.close(s.id).catch(() => undefined);
        return;
      }
      id = s.id;
      session = s;
      host?.focus();
    }, fail);
    const observer = new ResizeObserver(() => {
      if (host?.fit() && id !== null && exited === undefined)
        app.client.terminal.resize(id, host.cols, host.rows).catch(() => undefined);
    });
    observer.observe(el);
    return () => {
      closed = true;
      observer.disconnect();
      if (id !== null && exited === undefined)
        void app.client.terminal.close(id).catch(() => undefined);
      host?.dispose();
      host = null;
    };
  });

  function close() {
    // Most odczytany przed zamknięciem — zamknięcie zdejmuje żądanie (i `request`).
    const bridge = loginBridge(request.profile);
    const signal = detected;
    app.work.closeTerminal();
    // Po logowaniu od razu sprawdzamy stan konta — właściciel widzi, czy Alfa może korzystać
    // z narzędzia (np. „zalogowano · plan Max”), zamiast zgadywać po zamknięciu okna.
    if (bridge) void checkLogin(bridge, signal);
  }

  async function checkLogin(bridge: string, signal: LoginSignal | null) {
    try {
      // Antigravity nie ma polecenia stanu: zalogowanie wykryte w oknie zapamiętuje rdzeń.
      const card =
        bridge === 'antigravity' && signal
          ? await app.client.bridges.markLogin(bridge, signal.plan)
          : await app.client.bridges.check(bridge);
      app.work.bridgeChecked = card;
      const r = loginResult(card);
      const plan = r.withPlan ? t('conn.loginResult.planSuffix', { plan: r.vars.plan ?? '' }) : '';
      // Jeden wynik na most: kolejne „Gotowe” odświeża komunikat zamiast dokładać następny.
      app.toasts.show({
        key: `bridge-login:${bridge}`,
        kind: r.kind,
        message: tk(r.key, { ...r.vars, plan }),
        actionLabel: t('conn.openPage'),
        onAction: () => app.openSettings('connections'),
        timeoutMs: 12_000,
      });
    } catch (error) {
      showError(app.toasts, error);
    }
  }

  /** Gest użytkownika: wpisuje polecenie w terminal narzędzia (np. `/login` w Claude Code). */
  function type(command: string) {
    const id = session?.id;
    if (id === undefined || exited !== undefined) return;
    app.client.terminal
      .input(
        id,
        textToB64(`${command}
`),
      )
      .catch(() => undefined);
  }

  /** Ponowna próba: nowe żądanie tego samego profilu uruchamia terminal od początku. */
  function retry() {
    app.work.openTerminal(request.profile);
  }
</script>

<Dialog.Root open onOpenChange={(open) => !open && close()}>
  <Dialog.Portal>
    <Dialog.Overlay class="alfa-term-overlay" />
    <Dialog.Content
      class="alfa-term"
      escapeKeydownBehavior="ignore"
      interactOutsideBehavior="ignore"
    >
      <header class="head">
        <Dialog.Title class="alfa-term-title">{title}</Dialog.Title>
        {#if login}
          <Button
            size="sm"
            variant={exited === 0 || detected ? 'primary' : 'secondary'}
            onclick={close}>{t('term.done')}</Button
          >
        {:else}
          <Button size="sm" variant="secondary" onclick={close}>{t('term.close')}</Button>
        {/if}
      </header>
      <Dialog.Description class="alfa-term-desc">
        {login ? t('term.loginHint') : t('term.privacy')}
      </Dialog.Description>
      {#if login}
        <ol class="guide" aria-label={t('term.guide.label')}>
          {#each Array.from({ length: steps }, (_, i) => String(i + 1)) as n (n)}
            <li>
              <span class="num" aria-hidden="true">{n}</span>{tk(
                `term.guide.${request.profile}.${n}`,
              )}
            </li>
          {/each}
        </ol>
        <p class="hint">{tk(`term.guide.${request.profile}.note`)}</p>
        {#if request.profile === 'claude_login' && session && exited === undefined && !detected}
          <div class="quick">
            <Button size="sm" variant="secondary" onclick={() => type('/login')}
              >{t('term.typeLogin')}</Button
            >
            <span class="hint">{t('term.typeLoginHint')}</span>
          </div>
        {/if}
      {/if}
      {#if login && detected}
        <p class="note good" role="status">
          {detected.plan ? t('term.detectedPlan', { plan: detected.plan }) : t('term.detected')}
        </p>
      {/if}
      {#if error}
        <div class="note bad" role="alert">
          <span>{login ? t('term.openFailed', { error }) : error}</span>
          <Button size="sm" variant="secondary" onclick={retry}>{t('term.retry')}</Button>
        </div>
      {:else if login && exited === 0}
        <p class="note good" role="status">{t('term.loginOk')}</p>
      {:else if login && exited !== undefined}
        <div class="note bad" role="status">
          <span>{t('term.loginFailed', { code: exited ?? '—' })}</span>
          <Button size="sm" variant="secondary" onclick={retry}>{t('term.retry')}</Button>
        </div>
      {/if}
      <div class="screen" bind:this={container}></div>
      <p class="meta" role="status">
        {#if exited !== undefined}{t('term.exited', { code: exited ?? '—' })}
        {:else if session}{t('term.running', { pid: String(session.pid) })}
        {:else}{t('term.starting')}{/if}
      </p>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  /* Własne style (style arkusza ściągawki ładują się leniwie tylko z nią). */
  :global(.alfa-term-overlay) {
    position: fixed;
    inset: 0;
    z-index: 100;
    background: var(--alfa-color-scrim);
  }
  :global(.alfa-term) {
    position: fixed;
    top: 50%;
    left: 50%;
    z-index: 101;
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    width: min(1000px, calc(var(--alfa-vw, 100vw) - 32px));
    max-height: calc(var(--alfa-vh, 100vh) - 32px);
    padding: var(--alfa-space-4);
    transform: translate(-50%, -50%);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-overlay);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-3);
    color: var(--alfa-color-text);
  }
  :global(.alfa-term-title) {
    margin: 0;
    font-size: var(--alfa-font-size-lg);
    font-weight: var(--alfa-weight-semibold);
  }
  :global(.alfa-term-desc) {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-3);
  }
  .screen {
    flex: 1 1 auto;
    height: min(calc(var(--alfa-vh, 100vh) * 0.6), 520px);
    min-height: 120px;
    padding: var(--alfa-space-2);
    overflow: hidden;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-bg);
  }
  .meta {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .guide {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--alfa-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .guide li {
    display: flex;
    align-items: flex-start;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface2);
    font-size: var(--alfa-font-size-sm);
    line-height: 1.45;
  }
  .num {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-text);
    color: var(--alfa-color-text-on-accent);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
  }
  .hint {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .note {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-2);
    margin: 0;
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border: 1px solid currentColor;
    border-radius: var(--alfa-radius-card);
    font-size: var(--alfa-font-size-sm);
  }
  .note.good {
    color: var(--alfa-color-success);
  }
  .note.bad {
    color: var(--alfa-color-error);
  }
  @media (max-width: 760px) {
    .guide {
      grid-template-columns: 1fr;
    }
  }
  .quick {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-2);
  }
</style>
