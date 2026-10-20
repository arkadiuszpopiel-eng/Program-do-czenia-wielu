// Kanwa artefaktów (C2.8): który artefakt jest otwarty obok rozmowy i na której karcie.
// Osobny moduł — stan aplikacji jest u limitu długości pliku.
import type { CanvasItem } from '../logic/canvas';

export type CanvasTab = 'preview' | 'source';

export const canvas = $state<{ item: CanvasItem | null; tab: CanvasTab }>({
  item: null,
  tab: 'preview',
});

/** Element, z którego otwarto kanwę — po zamknięciu fokus wraca tam (klawiatura, czytnik ekranu). */
let opener: HTMLElement | null = null;

export function openCanvas(item: CanvasItem): void {
  // Bez DOM (testy logiki) nie ma czego przywracać.
  const active = typeof document === 'undefined' ? null : document.activeElement;
  opener = active && 'focus' in active ? (active as HTMLElement) : null;
  if (canvas.item?.key !== item.key) canvas.tab = 'preview';
  canvas.item = item;
}

export function closeCanvas(): void {
  canvas.item = null;
  const back = opener;
  opener = null;
  if (back?.isConnected) requestAnimationFrame(() => back.focus());
}
