<!--
  Lista wirtualizowana o zmiennych wysokościach (PLAN §14.7): renderowane są tylko elementy
  widoczne ± 1 ekran; wysokości mierzy wspólny ResizeObserver. Przyklejenie do dołu przy
  strumieniu wyłącza się, gdy przewiniesz w górę — wtedy licznik „nowe (n)".
-->
<script lang="ts" generics="T">
  import type { Snippet } from 'svelte';
  import { onMount, untrack } from 'svelte';
  import { VirtualModel, isAtBottom } from '../../logic/virtual';

  interface Props {
    items: readonly T[];
    keyOf: (item: T) => string;
    estimate?: number;
    label: string;
    busy?: boolean;
    row: Snippet<[T, number]>;
    /** Stan przyklejenia do dołu i liczba nowych elementów poniżej widoku (bindable). */
    atBottom?: boolean;
    newBelow?: number;
    scroller?: HTMLElement | null;
  }

  let {
    items,
    keyOf,
    estimate = 140,
    label,
    busy = false,
    row,
    atBottom = $bindable(true),
    newBelow = $bindable(0),
    scroller = $bindable(null),
  }: Props = $props();

  const model = new VirtualModel(untrack(() => estimate));
  let version = $state(0);
  let scrollTop = $state(0);
  let viewport = $state(800);
  let observer: ResizeObserver | null = null;
  let lastCount = 0;

  const keys = $derived(items.map(keyOf));
  const layout = $derived.by(() => {
    void version;
    model.setKeys(keys);
    const range = model.range(scrollTop, viewport);
    return {
      range,
      top: model.offsetOf(range.start),
      bottom: model.totalHeight - model.offsetOf(range.end),
    };
  });
  const visible = $derived(items.slice(layout.range.start, layout.range.end));

  function stickIfNeeded() {
    if (atBottom && scroller) scroller.scrollTop = scroller.scrollHeight;
  }

  function onScroll() {
    if (!scroller) return;
    scrollTop = scroller.scrollTop;
    atBottom = isAtBottom(scroller.scrollTop, scroller.clientHeight, scroller.scrollHeight);
    if (atBottom) newBelow = 0;
  }

  /** Pomiar elementu (akcja): wysokość trafia do modelu per klucz. */
  function measure(node: HTMLElement, key: string) {
    node.dataset['vkey'] = key;
    observer?.observe(node);
    return {
      update(next: string) {
        node.dataset['vkey'] = next;
      },
      destroy() {
        observer?.unobserve(node);
      },
    };
  }

  export function scrollToIndex(index: number): void {
    if (!scroller) return;
    atBottom = false;
    scroller.scrollTop = Math.max(0, model.offsetOf(index) - 48);
  }

  export function scrollToBottom(): void {
    atBottom = true;
    newBelow = 0;
    stickIfNeeded();
  }

  onMount(() => {
    observer = new ResizeObserver((entries) => {
      let changed = false;
      for (const entry of entries) {
        const key = (entry.target as HTMLElement).dataset['vkey'];
        if (key)
          changed =
            model.setHeight(key, entry.borderBoxSize[0]?.blockSize ?? entry.contentRect.height) ||
            changed;
      }
      if (changed) {
        version++;
        stickIfNeeded();
      }
    });
    const viewportObserver = new ResizeObserver(() => {
      if (scroller) viewport = scroller.clientHeight;
      stickIfNeeded();
    });
    if (scroller) {
      viewportObserver.observe(scroller);
      for (const node of scroller.querySelectorAll<HTMLElement>('[data-vkey]'))
        observer.observe(node);
    }
    lastCount = items.length;
    requestAnimationFrame(stickIfNeeded);
    return () => {
      observer?.disconnect();
      viewportObserver.disconnect();
    };
  });

  $effect(() => {
    const count = items.length;
    untrack(() => {
      if (count > lastCount && !atBottom) newBelow += count - lastCount;
    });
    lastCount = count;
    queueMicrotask(stickIfNeeded);
  });
</script>

<div
  class="scroller"
  bind:this={scroller}
  onscroll={onScroll}
  role="feed"
  aria-label={label}
  aria-busy={busy}
  tabindex="-1"
>
  <div style:height="{layout.top}px" aria-hidden="true"></div>
  {#each visible as item, i (keyOf(item))}
    <div class="row" use:measure={keyOf(item)}>
      {@render row(item, layout.range.start + i)}
    </div>
  {/each}
  <div style:height="{layout.bottom}px" aria-hidden="true"></div>
</div>

<style>
  .scroller {
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    overscroll-behavior: contain;
    overflow-anchor: auto;
  }
  .scroller:focus {
    outline: none;
  }
  .row {
    display: flow-root;
  }
</style>
