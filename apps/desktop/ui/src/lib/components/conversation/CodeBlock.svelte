<!--
  Blok kodu z paska akcji: kopiuj, zapisz jako plik, uruchom w terminalu (przez Brokera), zawijanie;
  dla HTML, SVG i Markdown — „Otwórz w kanwie” (C2.8).
-->
<script lang="ts">
  import { IconButton, SanitizedHtml } from '@alfa/ui-kit';
  import Copy from '@lucide/svelte/icons/copy';
  import Download from '@lucide/svelte/icons/download';
  import PanelRightOpen from '@lucide/svelte/icons/panel-right-open';
  import SquareTerminal from '@lucide/svelte/icons/square-terminal';
  import WrapText from '@lucide/svelte/icons/wrap-text';
  import type { RenderedBlock } from '../../api/types';
  import { blockArtifact } from '../../logic/canvas';
  import { openCanvas } from '../../state/canvas.svelte';
  import { attempt } from '../../state/attempt';
  import { useApp } from '../../state/context';

  interface Props {
    block: RenderedBlock;
    turnId: string;
    /** Podświetlanie dopiero po zamknięciu bloku i końcu strumienia. */
    settled: boolean;
  }

  let { block, turnId, settled }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  let wrap = $state(false);
  let host = $state<HTMLElement | null>(null);
  // Artefakt dopiero po końcu strumienia (pełne źródło); zwykły kod — bez akcji kanwy.
  const artifact = $derived(settled ? blockArtifact(turnId, block) : null);

  $effect(() => {
    if (!settled || !block.closed || !host) return;
    const code = host.querySelector<HTMLElement>('pre code');
    if (!code) return;
    void import('../../highlight/highlight').then((m) => m.highlightElement(code, block.lang));
  });

  // Kod przewijany w poziomie (wąskie okno, długie linie) — dostępny z klawiatury (strzałki).
  $effect(() => {
    void block.html_sanitized;
    host?.querySelector('pre')?.setAttribute('tabindex', '0');
  });

  async function copy() {
    const text = host?.querySelector('pre')?.textContent ?? '';
    try {
      await navigator.clipboard.writeText(text);
      app.toasts.show({ kind: 'success', message: t('common.copied'), timeoutMs: 2500 });
    } catch {
      app.toasts.show({ kind: 'warning', message: t('msg.copyFailed') });
    }
  }

  async function run() {
    const ok = await attempt(app.toasts, () => app.client.turns.runCode(turnId, block.index));
    if (ok) app.toasts.show({ kind: 'info', message: t('msg.brokerOpened') });
  }

  async function save() {
    await attempt(app.toasts, () => app.client.turns.saveCode(turnId, block.index));
  }
</script>

<figure class="code" class:wrap aria-label={t('msg.code', { lang: block.lang ?? '' })}>
  <figcaption class="bar">
    <span class="lang">{block.lang ?? 'text'}</span>
    <span class="tools">
      {#if artifact}
        <IconButton
          label={t('canvas.openNamed', {
            title: artifact.title ?? t(`canvas.kind.${artifact.kind}`),
          })}
          size="sm"
          onclick={() => openCanvas(artifact)}
        >
          <PanelRightOpen size={14} strokeWidth={1.5} />
        </IconButton>
      {/if}
      <IconButton label={t('msg.codeWrap')} size="sm" pressed={wrap} onclick={() => (wrap = !wrap)}>
        <WrapText size={14} strokeWidth={1.5} />
      </IconButton>
      <IconButton label={t('msg.codeCopy')} size="sm" onclick={copy}>
        <Copy size={14} strokeWidth={1.5} />
      </IconButton>
      <IconButton label={t('msg.codeSave')} size="sm" onclick={() => void save()}>
        <Download size={14} strokeWidth={1.5} />
      </IconButton>
      <IconButton label={t('msg.codeRun')} size="sm" onclick={run}>
        <SquareTerminal size={14} strokeWidth={1.5} />
      </IconButton>
    </span>
  </figcaption>
  <div bind:this={host}>
    <SanitizedHtml html_sanitized={block.html_sanitized} />
  </div>
</figure>

<style>
  .code {
    margin: var(--alfa-space-2) 0;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    overflow: hidden;
  }
  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 32px;
    padding: 0 var(--alfa-space-1) 0 var(--alfa-space-3);
    border-bottom: 1px solid var(--alfa-color-border);
    background: var(--alfa-color-surface);
  }
  .lang {
    color: var(--alfa-color-text-muted);
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-xs);
  }
  .tools {
    display: inline-flex;
  }
  .code :global(pre:focus-visible) {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
    outline-offset: -2px;
  }
  .wrap :global(pre) {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
