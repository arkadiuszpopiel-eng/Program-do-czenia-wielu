// Wspólne dla menu kontekstowego i podpowiedzi: pokazanie elementu w warstwie wierzchniej
// (`popover`) i ustawienie go przy punkcie bez wychodzenia poza okno.
import { uiZoom } from './zoom';

export function show(el: HTMLElement): void {
  try {
    el.showPopover();
  } catch {
    // Starszy silnik bez `popover`: element i tak jest pozycjonowany (`position: fixed`).
    el.dataset.open = '';
  }
}

/** Ustawia element przy punkcie (jednostki okna), nie wychodząc poza okno. */
export function place(el: HTMLElement, x: number, y: number, above?: number): void {
  const zoom = uiZoom();
  const [width, height] = [window.innerWidth / zoom, window.innerHeight / zoom];
  const box = el.getBoundingClientRect();
  const left = Math.max(4, Math.min(x, width - box.width - 4));
  const fits = y + box.height + 4 <= height;
  const top = fits ? y : Math.max(4, (above ?? y) - box.height);
  el.style.left = `${left}px`;
  el.style.top = `${top}px`;
}
