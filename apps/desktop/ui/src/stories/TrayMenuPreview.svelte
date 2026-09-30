<!--
  Makieta 20: menu zasobnika. W aplikacji to NATYWNE menu Windows (0 WebView, rdzeń Tauri);
  ta strona to tylko podgląd układu i tekstów do akceptacji. „STOP WSZYSTKIEGO" trafia do
  watchdog/Brokera, nie przez WebView.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { i18n } from '../lib/i18n/i18n.svelte';

  interface Props {
    theme?: 'light' | 'dark';
    listening?: boolean;
  }

  const props: Props = $props();
  const { t } = i18n;
  document.documentElement.setAttribute(
    'data-theme',
    untrack(() => props.theme ?? 'light'),
  );
  const items = [
    { key: 'tray.show', bold: true },
    { key: 'tray.new', sep: true },
    { key: 'tray.quick', hint: 'Ctrl+Alt+Space' },
    { key: 'tray.voice', check: true },
    { key: 'tray.dnd', check: false },
    { key: 'tray.stop', hint: 'Ctrl+Shift+F12', danger: true, sep: true },
    { key: 'tray.quit', sep: true },
  ] as const;
</script>

<section class="preview" aria-label={t('tray.title')}>
  <p class="status">
    <span class="dot" class:mic={props.listening}></span>
    {t(props.listening ? 'tray.status.listening' : 'tray.status.idle')}
  </p>
  <ul class="menu">
    {#each items as item (item.key)}
      {#if 'sep' in item && item.sep}<li class="sep"></li>{/if}
      <li class="item" class:bold={'bold' in item} class:danger={'danger' in item}>
        <span class="check">{'check' in item && item.check ? '✓' : ''}</span>
        <span class="label">{t(item.key)}</span>
        {#if 'hint' in item}<span class="hint">{item.hint}</span>{/if}
      </li>
    {/each}
  </ul>
</section>

<style>
  .preview {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    width: 280px;
    font-size: var(--alfa-font-size-sm);
  }
  .status {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    color: var(--alfa-color-text-muted);
  }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--alfa-color-text-subtle);
  }
  .dot.mic {
    background: var(--alfa-color-error);
  }
  .menu {
    margin: 0;
    padding: 4px;
    list-style: none;
    border: 1px solid var(--alfa-color-border);
    border-radius: 8px;
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-3);
  }
  .item {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    min-height: 32px;
    padding: 0 var(--alfa-space-3) 0 var(--alfa-space-1);
    border-radius: 4px;
  }
  .item:hover {
    background: var(--alfa-color-surface2);
  }
  .check {
    width: 16px;
    text-align: center;
  }
  .label {
    flex: 1;
  }
  .bold {
    font-weight: var(--alfa-weight-semibold);
  }
  .danger {
    color: var(--alfa-color-error);
    font-weight: var(--alfa-weight-semibold);
  }
  .hint {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .sep {
    height: 1px;
    margin: 4px 0;
    background: var(--alfa-color-border);
  }
</style>
