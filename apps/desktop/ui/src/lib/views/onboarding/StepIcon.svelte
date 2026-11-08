<!-- Znak kroku wprowadzenia: prosta ikona kreskowa w kolorze tekstu (bez plików graficznych). -->
<script lang="ts">
  interface Props {
    step: string;
    size?: number;
  }

  let { step, size = 20 }: Props = $props();

  /** Ścieżki ikon w siatce 24 × 24 (kreska, bez wypełnienia). */
  const PATHS: Record<string, readonly string[]> = {
    mic: [
      'M12 3a3 3 0 0 0-3 3v6a3 3 0 0 0 6 0V6a3 3 0 0 0-3-3z',
      'M6 11a6 6 0 0 0 12 0',
      'M12 17v4',
    ],
    voice: ['M4 10v4', 'M8 7v10', 'M12 4v16', 'M16 8v8', 'M20 10v4'],
    hardware: ['M7 7h10v10H7z', 'M10 3v4M14 3v4M10 17v4M14 17v4M3 10h4M3 14h4M17 10h4M17 14h4'],
    keys: ['M8 20a4 4 0 1 0 0-8 4 4 0 0 0 0 8z', 'M11 13l9-9', 'M16 8l3 3', 'M19 5l2 2'],
    autonomy: ['M12 3l8 3v6c0 4.5-3.2 7.9-8 9-4.8-1.1-8-4.5-8-9V6l8-3z', 'M9 12l2 2 4-4'],
    corpus: ['M5 4h11a3 3 0 0 1 3 3v13H8a3 3 0 0 1-3-3V4z', 'M9 9h6M9 13h4'],
    bridges: ['M4 5h16v14H4z', 'M8 10l3 2-3 2', 'M13 14h3'],
    import: ['M12 3v12', 'M7 10l5 5 5-5', 'M4 19h16'],
  };
  const paths = $derived(PATHS[step] ?? []);
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#each paths as d (d)}<path {d} />{/each}
</svg>
