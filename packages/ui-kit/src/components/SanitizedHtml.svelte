<!--
  JEDYNE miejsce w UI, które wstawia HTML. Przyjmuje wyłącznie pole `html_sanitized` z backendu:
  Markdown z LLM renderuje Rust (pulldown-cmark + ammonia, ADR 0009) — UI niczego nie parsuje
  ani nie sanitizuje. Nie przekazuj tu treści z innych źródeł.
-->
<script lang="ts">
  interface Props {
    /** Gotowy, zsanitowany HTML z rdzenia (`RenderedBlock.html_sanitized`). */
    html_sanitized: string;
  }

  let { html_sanitized }: Props = $props();
</script>

<div class="alfa-prose" data-selectable>
  <!-- eslint-disable-next-line svelte/no-at-html-tags -- HTML zsanitowany w Rust (ammonia), ADR 0009 -->
  {@html html_sanitized}
</div>

<style>
  .alfa-prose {
    overflow-wrap: anywhere;
  }
  .alfa-prose :global(p + p),
  .alfa-prose :global(ul),
  .alfa-prose :global(ol) {
    margin-top: var(--alfa-space-2);
  }
  .alfa-prose :global(ul),
  .alfa-prose :global(ol) {
    margin-bottom: 0;
    padding-left: var(--alfa-space-6);
  }
  .alfa-prose :global(h1),
  .alfa-prose :global(h2),
  .alfa-prose :global(h3),
  .alfa-prose :global(h4) {
    margin: var(--alfa-space-3) 0 var(--alfa-space-1);
    font-size: var(--alfa-font-size-lg);
  }
  .alfa-prose :global(code) {
    padding: 0 4px;
    border-radius: 4px;
    background: var(--alfa-color-surface2);
  }
  .alfa-prose :global(pre) {
    margin: 0;
    padding: var(--alfa-space-3);
    overflow-x: auto;
    border-radius: 0 0 var(--alfa-radius-card) var(--alfa-radius-card);
    background: var(--alfa-color-surface2);
    line-height: 1.45;
  }
  .alfa-prose :global(pre code) {
    padding: 0;
    background: transparent;
  }
  .alfa-prose :global(table) {
    display: block;
    max-width: 100%;
    overflow-x: auto;
    border-collapse: collapse;
  }
  .alfa-prose :global(th),
  .alfa-prose :global(td) {
    padding: var(--alfa-space-1) var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
  }
  .alfa-prose :global(.tok-kw) {
    color: var(--alfa-agent-gama);
  }
  .alfa-prose :global(.tok-str) {
    color: var(--alfa-color-success);
  }
  .alfa-prose :global(.tok-num) {
    color: var(--alfa-color-warning);
  }
  .alfa-prose :global(.tok-com) {
    color: var(--alfa-color-text-subtle);
    font-style: italic;
  }
</style>
