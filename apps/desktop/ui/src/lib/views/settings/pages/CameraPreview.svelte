<!--
  Obraz kamery na stronie Kamery: klatka podglądu, gdy podgląd działa, inaczej ikona; znak „na
  żywo” w rogu. Wydzielone z CamerasPage.svelte (limit 400 linii na plik).
-->
<script lang="ts">
  import Camera from '@lucide/svelte/icons/camera';
  import type { CameraFrame } from '../../../api/types-cameras';
  import { useApp } from '../../../state/context';

  interface Props {
    /** Podgląd tej kamery działa. */
    live: boolean;
    /** Ostatnia klatka (gdy podgląd działa). */
    frame?: CameraFrame;
    /** Nazwa kamery (opis obrazu). */
    name: string;
    /** Kamera niedostępna — obraz przygaszony. */
    dim?: boolean;
  }
  let { live, frame, name, dim = false }: Props = $props();
  const { t } = useApp().i18n;
</script>

<div class="view" class:live class:dim>
  {#if live && frame}
    <img
      src="data:image/png;base64,{frame.png_base64}"
      alt={t('cam.previewAlt', { name })}
      width={frame.width}
      height={frame.height}
    />
  {:else}
    <span class="placeholder" aria-hidden="true">
      <Camera size={28} strokeWidth={1.25} />
    </span>
  {/if}
  {#if live}<span class="rec" aria-hidden="true"></span>{/if}
</div>

<style>
  .view {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    aspect-ratio: 16 / 9;
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text-muted);
  }
  .view img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .dim {
    opacity: 0.6;
  }
  /* Znak „podgląd działa” w rogu obrazu. */
  .rec {
    position: absolute;
    top: var(--alfa-space-2);
    right: var(--alfa-space-2);
    width: 10px;
    height: 10px;
    border: 2px solid var(--alfa-color-bg);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-error);
  }
</style>
