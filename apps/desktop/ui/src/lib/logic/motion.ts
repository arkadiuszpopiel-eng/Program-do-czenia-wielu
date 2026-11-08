// Czas animacji paneli z tokenu `--alfa-duration-panel` (0 przy ograniczonym ruchu i przy
// wyłączonych animacjach), żeby przejścia Svelte słuchały tych samych ustawień co CSS.
let instantUntil = 0;

/** Najbliższe przejścia paneli bez animacji (np. przypięcie wysuniętego panelu). */
export function skipPanelMotion(): void {
  instantUntil = performance.now() + 50;
}

/** Czas wysuwania panelu (ms); trochę dłuższy od tokenu — panel przebywa całą swoją szerokość. */
export function panelMs(): number {
  if (typeof document === 'undefined' || performance.now() < instantUntil) return 0;
  const raw = getComputedStyle(document.documentElement)
    .getPropertyValue('--alfa-duration-panel')
    .trim();
  const ms = raw.endsWith('ms') ? parseFloat(raw) : parseFloat(raw) * 1000;
  return Number.isFinite(ms) ? Math.round(ms * 1.3) : 0;
}
