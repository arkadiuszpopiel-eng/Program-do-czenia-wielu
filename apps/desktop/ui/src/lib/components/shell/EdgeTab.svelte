<!--
  Znacznik schowanego panelu przy krawędzi okna (właściciel 27.10: panele „mają się chować,
  wysuwać i mieć znaczniki, że tam są”). Spoczynek: świecąca kreska. Najechanie albo fokus: kreska
  rośnie w uchwyt ze strzałką do środka, a po chwili panel wysuwa się nad treść; kliknięcie
  przełącza panel jak przycisk w pasku tytułu (dokuje go).
-->
<script lang="ts">
  import ChevronLeft from '@lucide/svelte/icons/chevron-left';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import { fade } from 'svelte/transition';
  import { panelMs } from '../../logic/motion';

  interface Props {
    side: 'left' | 'right';
    label: string;
    onenter: () => void;
    onleave: () => void;
    onclick: () => void;
  }

  let { side, label, onenter, onleave, onclick }: Props = $props();
</script>

<button
  type="button"
  class="edge-tab {side}"
  aria-label={label}
  title={label}
  onmouseenter={onenter}
  onmouseleave={onleave}
  {onclick}
  transition:fade={{ duration: panelMs() }}
>
  <span class="handle" aria-hidden="true">
    <span class="led"></span>
    <span class="chev">
      {#if side === 'left'}<ChevronRight size={14} strokeWidth={2} />{:else}<ChevronLeft
          size={14}
          strokeWidth={2}
        />{/if}
    </span>
  </span>
</button>

<style>
  .edge-tab {
    position: absolute;
    top: 50%;
    z-index: 31;
    display: flex;
    align-items: center;
    width: 22px;
    height: 96px;
    padding: 0;
    border: 0;
    background: transparent;
    cursor: pointer;
    transform: translateY(-50%);
  }
  .edge-tab.left {
    left: 0;
    justify-content: flex-start;
  }
  .edge-tab.right {
    right: 0;
    justify-content: flex-end;
  }
  .handle {
    /* Styl klasyczny nie ma akcentu — wtedy kolor tekstu. */
    --led: var(--alfa-color-accent, var(--alfa-color-text));
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 4px;
    height: 44px;
    border: 1px solid transparent;
    background: transparent;
    transition:
      width var(--alfa-duration-fast) var(--alfa-ease-out),
      height var(--alfa-duration-fast) var(--alfa-ease-out),
      background-color var(--alfa-duration-fast) var(--alfa-ease-out),
      border-color var(--alfa-duration-fast) var(--alfa-ease-out),
      box-shadow var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .left .handle {
    border-radius: 0 var(--alfa-radius-full) var(--alfa-radius-full) 0;
  }
  .right .handle {
    border-radius: var(--alfa-radius-full) 0 0 var(--alfa-radius-full);
  }
  .led {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 4px;
    border-radius: var(--alfa-radius-full);
    background: var(--led);
    box-shadow: 0 0 8px color-mix(in srgb, var(--led) 70%, transparent);
    opacity: 0.6;
    transition: opacity var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .left .led {
    left: 0;
  }
  .right .led {
    right: 0;
  }
  /* W spoczynku strzałki nie ma — nie wystaje poza krawędź okna (wąskie okno). */
  .chev {
    display: none;
    color: var(--alfa-color-text);
  }
  .edge-tab:hover .handle,
  .edge-tab:focus-visible .handle {
    width: 20px;
    height: 64px;
    border-color: var(--alfa-color-border);
    background: var(--alfa-color-overlay, var(--alfa-color-surface));
    /* Bez `--alfa-shadow-2` w liście: w ciemnych motywach to `none`, a lista z `none` jest
       nieważna i cień znikał w całości. */
    box-shadow:
      0 4px 14px rgb(0 0 0 / 0.22),
      0 0 12px color-mix(in srgb, var(--led) 35%, transparent);
  }
  .edge-tab:hover .led,
  .edge-tab:focus-visible .led {
    opacity: 1;
  }
  .edge-tab:hover .chev,
  .edge-tab:focus-visible .chev {
    display: inline-flex;
  }
  .edge-tab:focus-visible {
    outline: none;
  }
  .edge-tab:focus-visible .handle {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
    outline-offset: 2px;
  }
  @media (forced-colors: active) {
    .led {
      background: Highlight;
      box-shadow: none;
    }
  }
</style>
