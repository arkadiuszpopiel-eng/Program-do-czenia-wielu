<!--
  Znacznik schowanego panelu przy krawędzi okna (właściciel 27.10: panele „mają się chować,
  wysuwać i mieć znaczniki, że tam są”). Świecąca kreska; najechanie wysuwa panel nad rozmowę,
  kliknięcie przełącza go jak przycisk w pasku tytułu.
-->
<script lang="ts">
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
>
  <span class="led" aria-hidden="true"></span>
</button>

<style>
  .edge-tab {
    position: absolute;
    top: 50%;
    z-index: 31;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 72px;
    padding: 0;
    border: 0;
    background: transparent;
    cursor: pointer;
    transform: translateY(-50%);
  }
  .edge-tab.left {
    left: 0;
  }
  .edge-tab.right {
    right: 0;
  }
  .led {
    /* Styl klasyczny nie ma akcentu — wtedy kolor tekstu. */
    --led: var(--alfa-color-accent, var(--alfa-color-text));
    width: 4px;
    height: 40px;
    border-radius: var(--alfa-radius-full);
    background: var(--led);
    box-shadow: 0 0 6px color-mix(in srgb, var(--led) 70%, transparent);
    opacity: 0.55;
    transition:
      opacity var(--alfa-duration-fast, 120ms) var(--alfa-ease-out),
      height var(--alfa-duration-fast, 120ms) var(--alfa-ease-out),
      box-shadow var(--alfa-duration-fast, 120ms) var(--alfa-ease-out);
  }
  .edge-tab:hover .led,
  .edge-tab:focus-visible .led {
    height: 56px;
    opacity: 1;
    box-shadow: 0 0 10px var(--led);
  }
  .edge-tab:focus-visible {
    outline: 2px solid var(--alfa-color-focus);
    outline-offset: -2px;
    border-radius: var(--alfa-radius-control);
  }
  @media (forced-colors: active) {
    .led {
      background: Highlight;
      box-shadow: none;
    }
  }
</style>
