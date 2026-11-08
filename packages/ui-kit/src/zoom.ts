/**
 * Skala interfejsu (CSS `zoom` na <html>, ustawiana przez aplikację w `--alfa-zoom`). Rozmiary okna
 * i ruch wskaźnika są w pikselach okna, a wymiary elementów — w jednostkach po skalowaniu; przy
 * przeliczaniu jednych na drugie trzeba podzielić przez skalę.
 */
export function uiZoom(): number {
  const raw = getComputedStyle(document.documentElement).getPropertyValue('--alfa-zoom');
  const zoom = Number.parseFloat(raw);
  return zoom > 0 ? zoom : 1;
}
