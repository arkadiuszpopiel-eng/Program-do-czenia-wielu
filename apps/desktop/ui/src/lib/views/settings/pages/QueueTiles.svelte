<!--
  Kolejka modeli jako kafelki do przenoszenia (zgłoszenie właściciela, 2026-10-09: przestawianie
  strzałkami było niewygodne). Kafelek podnosi się wskaźnikiem i upuszcza w nowym miejscu; pozostałe
  rozsuwają się na bieżąco, a podniesiony jest wyraźnie oznaczony. Po upuszczeniu albo dodaniu
  kafelek przez chwilę świeci — widać, co się zmieniło. Z klawiatury: uchwyt, Spacja, strzałki,
  Spacja (Escape odkłada na miejsce). Bez systemowego „drag and drop” — jego podgląd rysuje system.
-->
<script lang="ts">
  import { IconButton, uiZoom } from '@alfa/ui-kit';
  import GripVertical from '@lucide/svelte/icons/grip-vertical';
  import X from '@lucide/svelte/icons/x';
  import type { ModelInfo } from '../../../api/types-hub';
  import { useApp } from '../../../state/context';
  import ModelBadges from './ModelBadges.svelte';

  interface Props {
    queue: readonly string[];
    models: ReadonlyMap<string, ModelInfo>;
    saving: boolean;
    /** Zapis nowej kolejności albo listy bez usuniętego modelu. */
    onsave: (models: string[]) => Promise<boolean>;
  }

  let { queue, models, saving, onsave }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;

  /** Ruch wskaźnika (px okna), po którym naciśnięcie staje się przenoszeniem. */
  const DRAG_START = 5;
  /** Jak długo kafelek świeci po zmianie (ms). */
  const MARK_MS = 2600;

  /** Kolejność w trakcie przenoszenia; poza nim — kolejka z konta. */
  let draft = $state<string[] | null>(null);
  /** Kafelek podniesiony wskaźnikiem albo z klawiatury. */
  let lifted = $state<string | null>(null);
  /** Kafelek właśnie przeniesiony albo dodany i rodzaj zmiany. */
  let marked = $state<{ id: string; kind: 'moved' | 'added' } | null>(null);
  let announce = $state('');
  let list = $state<HTMLElement | null>(null);
  let press: { id: string; x: number; y: number; pointer: number } | null = null;
  let markTimer: ReturnType<typeof setTimeout> | undefined;
  let known: readonly string[] = [];

  const order = $derived(draft ?? queue);

  function mark(id: string, kind: 'moved' | 'added') {
    marked = { id, kind };
    clearTimeout(markTimer);
    markTimer = setTimeout(() => (marked = null), MARK_MS);
  }

  // Model, który pojawił się w kolejce (dodany z listy albo dobrany automatycznie pojedynczo).
  $effect(() => {
    const added = queue.filter((id) => !known.includes(id));
    if (known.length > 0 && added.length === 1 && added[0]) mark(added[0], 'added');
    known = queue;
  });
  $effect(() => () => clearTimeout(markTimer));

  // Escape w trakcie przenoszenia wskaźnikiem odkłada kafelek. Nasłuch w fazie przechwytywania:
  // skrót okna (Escape zamyka Ustawienia) nie może zadziałać pierwszy.
  $effect(() => {
    const onEscape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || !lifted || !press) return;
      event.preventDefault();
      event.stopPropagation();
      cancel();
    };
    window.addEventListener('keydown', onEscape, true);
    return () => window.removeEventListener('keydown', onEscape, true);
  });

  function say(id: string, list: readonly string[]) {
    announce = t('api.announce', { model: id, n: list.indexOf(id) + 1, total: list.length });
  }

  function shift(id: string, to: number) {
    const current = [...order];
    const from = current.indexOf(id);
    if (from < 0 || to < 0 || to >= current.length || from === to) return;
    current.splice(from, 1);
    current.splice(to, 0, id);
    draft = current;
    say(id, current);
  }

  async function drop() {
    const [id, next] = [lifted, draft];
    lifted = null;
    press = null;
    if (!id || !next || next.join('\n') === queue.join('\n')) {
      draft = null;
      return;
    }
    const saved = await onsave(next);
    draft = null;
    if (saved) mark(id, 'moved');
  }

  function cancel() {
    lifted = null;
    press = null;
    draft = null;
    announce = t('api.cancelled');
  }

  function onpointerdown(event: PointerEvent, id: string) {
    if (saving || event.button !== 0) return;
    if (event.target instanceof Element && event.target.closest('button')) return;
    press = { id, x: event.clientX, y: event.clientY, pointer: event.pointerId };
  }

  function onpointermove(event: PointerEvent) {
    if (!press || event.pointerId !== press.pointer) return;
    if (!lifted) {
      if (Math.hypot(event.clientX - press.x, event.clientY - press.y) < DRAG_START) return;
      lifted = press.id;
      draft = [...queue];
    }
    // Wymiary kafelków są w jednostkach po skali interfejsu, wskaźnik — w pikselach okna.
    const zoom = uiZoom();
    const [x, y] = [event.clientX / zoom, event.clientY / zoom];
    const over = [...(list?.querySelectorAll<HTMLElement>('[data-tile]') ?? [])].find((el) => {
      const box = el.getBoundingClientRect();
      return x >= box.left && x <= box.right && y >= box.top && y <= box.bottom;
    });
    const target = over?.dataset.tile;
    if (target && target !== lifted) shift(lifted, order.indexOf(target));
  }

  function onpointerup(event: PointerEvent) {
    if (!press || event.pointerId !== press.pointer) return;
    if (lifted) void drop();
    else press = null;
  }

  function onkeydown(event: KeyboardEvent, id: string) {
    const toggle = event.key === ' ' || event.key === 'Enter';
    if (lifted !== id) {
      if (!toggle || saving) return;
      event.preventDefault();
      lifted = id;
      draft = [...queue];
      say(id, queue);
      return;
    }
    const step =
      event.key === 'ArrowRight' || event.key === 'ArrowDown'
        ? 1
        : event.key === 'ArrowLeft' || event.key === 'ArrowUp'
          ? -1
          : 0;
    if (step) shift(id, order.indexOf(id) + step);
    else if (toggle) void drop();
    else if (event.key === 'Escape') cancel();
    else return;
    // Escape odkłada kafelek — nie zamyka Ustawień.
    event.preventDefault();
    event.stopPropagation();
  }
</script>

<svelte:window {onpointermove} {onpointerup} onpointercancel={() => lifted && cancel()} />

<p class="hint">{t('api.dragHint')}</p>
<ol class="tiles" class:sorting={lifted !== null} bind:this={list}>
  {#each order as id, index (id)}
    {@const model = models.get(id)}
    {@const moving = lifted === id}
    {@const fresh = marked?.id === id && !moving ? marked.kind : null}
    <li
      class="tile"
      class:moving
      class:fresh={fresh !== null}
      data-tile={id}
      data-state={moving ? 'moving' : (fresh ?? 'idle')}
      onpointerdown={(event) => onpointerdown(event, id)}
    >
      <div class="top">
        <span class="pos">{index + 1}</span>
        <span class="role">
          {#if moving}
            {t('api.moving')}
          {:else if fresh}
            {t(fresh === 'moved' ? 'api.movedBadge' : 'api.addedBadge')}
          {:else}
            {index === 0 ? t('api.first') : t('api.fallback', { n: index })}
          {/if}
        </span>
        <button
          type="button"
          class="grip"
          aria-label={t('api.grab', { model: id })}
          aria-pressed={moving}
          disabled={saving}
          onkeydown={(event) => onkeydown(event, id)}
          onblur={() => {
            if (lifted === id && !press) cancel();
          }}
        >
          <GripVertical size={16} strokeWidth={1.75} aria-hidden="true" />
        </button>
        <IconButton
          size="sm"
          tone="danger"
          label={t('api.remove', { model: id })}
          disabled={saving || lifted !== null}
          onclick={() => void onsave(queue.filter((m) => m !== id))}
          ><X size={16} strokeWidth={1.75} /></IconButton
        >
      </div>
      <span class="name">{model?.name ?? id}</span>
      <code>{id}</code>
      {#if model}
        <ModelBadges {model} />
      {:else}
        <span class="missing" data-tip={t('api.missingHint')}>{t('api.missing')}</span>
      {/if}
    </li>
  {/each}
</ol>
<p class="alfa-visually-hidden" role="status">{announce}</p>

<style>
  .hint {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(210px, 1fr));
    gap: var(--alfa-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .tile {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    min-width: 0;
    padding: var(--alfa-space-2) var(--alfa-space-3) var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    cursor: grab;
    touch-action: none;
    user-select: none;
    transition:
      border-color 0.15s ease,
      box-shadow 0.15s ease,
      transform 0.15s ease;
  }
  .tile:hover {
    border-color: var(--alfa-color-border-strong);
  }
  /* Podniesiony kafelek: gruba ramka w kolorze akcentu, cień i lekkie powiększenie. */
  .tile.moving {
    position: relative;
    z-index: 2;
    border-color: var(--alfa-color-accent);
    box-shadow:
      0 0 0 2px var(--alfa-color-accent),
      var(--alfa-shadow-3);
    background: var(--alfa-color-surface2);
    transform: scale(1.03);
    cursor: grabbing;
  }
  .sorting .tile:not(.moving) {
    opacity: 0.72;
  }
  /* Właśnie przeniesiony albo dodany: ramka w kolorze powodzenia, dopóki oznaczenie nie zgaśnie. */
  .tile.fresh {
    border-color: var(--alfa-color-success);
    box-shadow: 0 0 0 2px var(--alfa-color-success);
  }
  .top {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
  }
  .pos {
    flex: none;
    width: 1.75em;
    height: 1.75em;
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface2);
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
    line-height: 1.75em;
    text-align: center;
  }
  .moving .pos {
    background: var(--alfa-color-text);
    color: var(--alfa-color-bg);
  }
  .role {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .moving .role,
  .fresh .role {
    color: var(--alfa-color-text);
    font-weight: var(--alfa-weight-semibold);
  }
  .grip {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text-muted);
    cursor: grab;
  }
  .grip:hover:not(:disabled),
  .grip[aria-pressed='true'] {
    background: var(--alfa-color-surface3);
    color: var(--alfa-color-text);
  }
  .grip:focus-visible {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
    outline-offset: 1px;
  }
  .name {
    overflow: hidden;
    font-weight: var(--alfa-weight-semibold);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  code {
    overflow: hidden;
    color: var(--alfa-color-text-muted);
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-xs);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .missing {
    color: var(--alfa-color-warning);
    font-size: var(--alfa-font-size-xs);
  }
  @media (prefers-reduced-motion: reduce) {
    .tile {
      transition: none;
    }
    .tile.moving {
      transform: none;
    }
  }
</style>
