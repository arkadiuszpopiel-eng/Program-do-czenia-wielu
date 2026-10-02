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
  import { b64ToBytes, binaryToB64, textToB64 } from '../../logic/work';
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
  const title = $derived(tk(`term.profile.${request.profile}`));

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
    let host: TerminalHost | null = null;
    let id: number | null = null;
    let closed = false;
    const send = (b64: string) => {
      if (id !== null && exited === undefined)
        app.client.terminal.input(id, b64).catch(() => undefined);
    };
    const label = untrack(() => t('term.inputLabel', { name: tk(`term.profile.${profile}`) }));
    host = createTerminal(el, {
      label,
      onData: (data) => send(textToB64(data)),
      onBinary: (data) => send(binaryToB64(data)),
    });
    host.fit();
    const onFrame = (frame: TerminalFrame) => {
      if (frame.kind === 'output') host?.write(b64ToBytes(frame.data_b64));
      else {
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
    app.work.closeTerminal();
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
        <Dialog.Title class="alfa-term-title">{t('term.title', { name: title })}</Dialog.Title>
        <Button size="sm" variant="secondary" onclick={close}>{t('term.close')}</Button>
      </header>
      <Dialog.Description class="alfa-term-desc">
        {request.profile === 'claude_login' || request.profile === 'codex_login'
          ? t('term.loginHint')
          : t('term.privacy')}
      </Dialog.Description>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <div class="screen" bind:this={container}></div>
      <p class="meta" role="status">
        {#if exited !== undefined}{t('term.exited', { code: exited ?? '—' })}
        {:else if session}{t('term.running', { pid: session.pid })}
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
    width: min(1000px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
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
    height: min(60vh, 520px);
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
  .error {
    color: var(--alfa-color-error);
    font-size: var(--alfa-font-size-sm);
  }
</style>
