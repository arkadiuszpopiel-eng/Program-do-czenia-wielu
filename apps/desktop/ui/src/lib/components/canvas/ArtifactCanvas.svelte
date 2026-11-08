<!--
  Kanwa artefaktów (C2.8, ADR 0009): artefakt z rozmowy obok czatu — karty Podgląd / Źródło
  (i Zmiany, gdy artefakt ma wcześniejszą wersję w rozmowie), przełącznik wersji, kopiowanie, zapis istniejącymi ścieżkami (blok kodu, wiadomość, plik sesji), limit rozmiaru.
  Markdown: HTML z Rust wstawiany jako zwykły DOM (SanitizedHtml). HTML i SVG: dokument
  zsanitowany w Rust wyłącznie w ramce `sandbox=""` — bez skryptów, bez `allow-same-origin`,
  więc bez dostępu do okna Alfy, IPC i `__TAURI_INTERNALS__`; dokument dziedziczy CSP okna.
-->
<script lang="ts">
  import { IconButton, SanitizedHtml, sanitizedHtml } from '@alfa/ui-kit';
  import Copy from '@lucide/svelte/icons/copy';
  import Download from '@lucide/svelte/icons/download';
  import X from '@lucide/svelte/icons/x';
  import { CANVAS_MAX_BYTES, byteSize, clipBytes, tooLarge } from '../../logic/canvas';
  import { artifactVersions } from '../../logic/canvas-versions';
  import { attempt, load } from '../../state/attempt';
  import { canvas, closeCanvas, showVersion, type CanvasTab } from '../../state/canvas.svelte';
  import { useApp } from '../../state/context';
  import { exportConversation } from '../../state/exports';
  import LoadFailed from '../shell/LoadFailed.svelte';
  import CanvasDiff from './CanvasDiff.svelte';
  import CanvasVersions from './CanvasVersions.svelte';

  const app = useApp();
  const { t } = app.i18n;
  const uid = $props.id();
  const item = $derived(canvas.item);
  const title = $derived(item ? (item.title ?? t(`canvas.kind.${item.kind}`)) : '');
  const size = $derived(item ? byteSize(item.source) : 0);
  const big = $derived(item ? tooLarge(item) : false);
  // Źródło ponad limit pokazujemy ucięte do limitu bajtów na granicy znaku, z uwagą (cały
  // tekst idzie do schowka i do pliku).
  const shown = $derived(clipBytes(item?.source ?? '', CANVAS_MAX_BYTES));
  const framed = $derived(item?.kind === 'html' || item?.kind === 'svg');
  let html = $state<string | null>(null);
  let error = $state<string | null>(null);
  let frame = $state<HTMLIFrameElement | null>(null);
  let root = $state<HTMLElement | null>(null);
  let heading = $state<HTMLElement | null>(null);
  // Historia wersji: odpowiedzi na widocznej ścieżce rozmowy z tym samym artefaktem (bez zapisu —
  // rozmowa jest append-only, więc wersje wynikają z tur).
  const versions = $derived.by(() => {
    const conv = app.conversation;
    if (!item) return [];
    return conv?.loaded ? artifactVersions(conv.path, item) : [item];
  });
  const index = $derived(
    Math.max(
      0,
      versions.findIndex((v) => v.key === item?.key),
    ),
  );
  const previous = $derived(index > 0 ? versions[index - 1] : undefined);
  const tabs = $derived<readonly CanvasTab[]>(
    previous ? ['preview', 'source', 'changes'] : ['preview', 'source'],
  );
  // Karta Zmiany bez poprzedniej wersji (pierwsza wersja, inny artefakt) — Podgląd.
  const tab = $derived<CanvasTab>(canvas.tab === 'changes' && !previous ? 'preview' : canvas.tab);

  async function render() {
    const current = canvas.item;
    html = null;
    error = null;
    if (!current || tooLarge(current)) return;
    if (current.kind === 'markdown' && current.html_sanitized !== undefined) {
      html = current.html_sanitized;
      return;
    }
    const result = await load(() =>
      app.client.conversation.renderArtifact(current.kind, current.source, app.i18n.locale),
    );
    if (canvas.item !== current) return;
    if (result.status === 'ready') html = result.value;
    else if (result.status === 'failed') error = result.error;
  }

  $effect(() => {
    void item;
    void render();
  });

  // `srcdoc` to ujście Trusted Types: wyłącznie dokument zsanitowany w Rust (polityka
  // `alfa-sanitized-html`), przypisany właściwości — bez parsowania HTML w oknie głównym.
  $effect(() => {
    if (frame && html !== null) frame.srcdoc = sanitizedHtml(html);
  });

  // Otwarcie (albo inny artefakt) przenosi fokus na nagłówek kanwy: czytnik ekranu ogłasza region,
  // a w wąskim oknie fokus trafia do arkusza (moduł ładuje się po otwarciu dialogu).
  let focusedKey: string | null = null;
  $effect(() => {
    const key = item?.key ?? null;
    const node = heading;
    if (!key || !node || key === focusedKey) return;
    focusedKey = key;
    requestAnimationFrame(() => node.focus());
  });

  function selectTab(tab: CanvasTab, focus = false) {
    canvas.tab = tab;
    if (focus) document.getElementById(`${uid}-tab-${tab}`)?.focus();
  }

  // Wersja z przełącznika: fokus zostaje na przełączniku (nie skacze na nagłówek jak przy otwarciu).
  function selectVersion(version: (typeof versions)[number]) {
    focusedKey = version.key;
    showVersion(version);
  }

  function tabKey(event: KeyboardEvent, index: number) {
    const step = event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0;
    const edge = event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1 : -1;
    const next = step ? (index + step + tabs.length) % tabs.length : edge;
    const target = tabs[next];
    if (next < 0 || !target) return;
    event.preventDefault();
    selectTab(target, true);
  }

  // Esc z dowolnego miejsca kanwy zamyka ją (przed skrótami okna — `preventDefault`).
  function onKeydown(event: KeyboardEvent) {
    if (event.key !== 'Escape' || event.defaultPrevented) return;
    event.preventDefault();
    closeCanvas();
  }

  $effect(() => {
    const node = root;
    if (!node) return;
    node.addEventListener('keydown', onKeydown);
    return () => node.removeEventListener('keydown', onKeydown);
  });

  async function copy() {
    if (!item) return;
    try {
      await navigator.clipboard.writeText(item.source);
      app.toasts.show({ kind: 'success', message: t('canvas.copied'), timeoutMs: 2500 });
    } catch {
      app.toasts.show({ kind: 'warning', message: t('msg.copyFailed') });
    }
  }

  async function save() {
    const origin = item?.origin;
    if (!origin) return;
    if (origin.type === 'turn') {
      await exportConversation(app, origin.sessionId, 'markdown', origin.turnId);
    } else if (origin.type === 'block') {
      await attempt(app.toasts, () => app.client.turns.saveCode(origin.turnId, origin.blockIndex));
    } else {
      await attempt(app.toasts, () => app.client.files.act(origin.artifactId, 'save_as'));
    }
  }
</script>

{#if item}
  <section
    class="canvas"
    aria-labelledby="{uid}-title"
    data-testid="artifact-canvas"
    bind:this={root}
  >
    <header class="head">
      <div class="names">
        <h2 id="{uid}-title" class="title" tabindex="-1" bind:this={heading}>{title}</h2>
        <p class="meta">
          {t(`canvas.kind.${item.kind}`)} · {app.i18n.bytes(size)}
        </p>
      </div>
      <span class="tools">
        <IconButton label={t('canvas.copy')} size="sm" onclick={() => void copy()}>
          <Copy size={16} strokeWidth={1.5} />
        </IconButton>
        <IconButton label={t('canvas.save')} size="sm" onclick={() => void save()}>
          <Download size={16} strokeWidth={1.5} />
        </IconButton>
        <IconButton label={t('canvas.close')} size="sm" onclick={closeCanvas}>
          <X size={16} strokeWidth={1.5} />
        </IconButton>
      </span>
    </header>

    {#if versions.length > 1}
      <CanvasVersions {versions} {index} onselect={selectVersion} />
    {/if}

    <div class="tabs" role="tablist" aria-label={t('canvas.views')}>
      {#each tabs as name, i (name)}
        <button
          type="button"
          role="tab"
          class="tab"
          id="{uid}-tab-{name}"
          aria-selected={tab === name}
          aria-controls="{uid}-panel"
          tabindex={tab === name ? 0 : -1}
          onclick={() => selectTab(name)}
          onkeydown={(e) => tabKey(e, i)}>{t(`canvas.${name}`)}</button
        >
      {/each}
    </div>

    <div class="panel" id="{uid}-panel" role="tabpanel" aria-labelledby="{uid}-tab-{tab}">
      {#if tab === 'changes' && previous}
        <CanvasDiff before={previous.source} after={item.source} previous={index} />
      {:else if tab === 'source'}
        {#if shown.clipped}
          <p class="notice" role="status">
            {t('canvas.truncated', { limit: app.i18n.bytes(CANVAS_MAX_BYTES) })}
          </p>
        {/if}
        <!-- tabindex: przewijane źródło dostępne z klawiatury (strzałki / PgDn). -->
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <pre class="source" tabindex="0">{shown.text}</pre>
      {:else if big}
        <p class="notice" role="status">
          {t('canvas.tooLarge', {
            size: app.i18n.bytes(size),
            limit: app.i18n.bytes(CANVAS_MAX_BYTES),
          })}
        </p>
      {:else if error}
        <div class="pad"><LoadFailed {error} onretry={() => void render()} /></div>
      {:else if html === null}
        <p class="notice" role="status">{t('canvas.loading')}</p>
      {:else if framed}
        <iframe
          bind:this={frame}
          class="frame"
          sandbox=""
          referrerpolicy="no-referrer"
          title={t('canvas.frameTitle', { title })}
          aria-label={t('canvas.frameTitle', { title })}
        ></iframe>
        <p class="isolated">{t('canvas.isolated')}</p>
      {:else}
        <div class="doc" role="document" aria-label={title}>
          <SanitizedHtml html_sanitized={html} />
        </div>
      {/if}
    </div>
  </section>
{/if}

<style>
  .canvas {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-width: 0;
    min-height: 0;
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
  }
  .head {
    display: flex;
    align-items: flex-start;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-3) var(--alfa-space-2) var(--alfa-space-2) var(--alfa-space-4);
  }
  .names {
    flex: 1;
    min-width: 0;
  }
  .title {
    margin: 0;
    overflow: hidden;
    font-size: var(--alfa-font-size-md, 1rem);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    margin: 2px 0 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .tools {
    display: inline-flex;
    flex: none;
  }
  .tabs {
    display: flex;
    gap: var(--alfa-space-1);
    padding: 0 var(--alfa-space-4);
    border-bottom: 1px solid var(--alfa-color-border);
  }
  .tab {
    min-height: 32px;
    padding: 0 var(--alfa-space-3);
    border: 0;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--alfa-color-text-muted);
    font: inherit;
    font-size: var(--alfa-font-size-sm);
    cursor: pointer;
  }
  .tab[aria-selected='true'] {
    border-bottom-color: var(--alfa-color-accent);
    color: var(--alfa-color-text);
    font-weight: 600;
  }
  .tab:focus-visible {
    outline: 2px solid var(--alfa-color-focus, var(--alfa-color-accent));
    outline-offset: -2px;
  }
  .panel {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
  }
  .source,
  .doc {
    flex: 1;
    min-height: 0;
    margin: 0;
    padding: var(--alfa-space-3) var(--alfa-space-4);
    overflow: auto;
  }
  .source {
    background: var(--alfa-color-surface2);
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-xs);
    line-height: 1.5;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .title:focus-visible,
  .source:focus-visible,
  .doc:focus-visible {
    outline: 2px solid var(--alfa-color-focus, var(--alfa-color-accent));
    outline-offset: -2px;
  }
  .frame {
    flex: 1;
    width: 100%;
    min-height: 240px;
    border: 0;
  }
  .isolated,
  .notice {
    margin: 0;
    padding: var(--alfa-space-2) var(--alfa-space-4);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .isolated {
    border-top: 1px solid var(--alfa-color-border);
  }
  .pad {
    padding: var(--alfa-space-3) var(--alfa-space-4);
  }
</style>
