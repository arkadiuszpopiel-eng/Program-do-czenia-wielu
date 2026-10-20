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
/** Aktywna sesja według powłoki (`syncCanvas`) i sesja, w której otwarto artefakt. */
let activeSession: string | null = null;
let openedIn: string | null = null;

export function openCanvas(item: CanvasItem): void {
  // Bez DOM (testy logiki) nie ma czego przywracać.
  const active = typeof document === 'undefined' ? null : document.activeElement;
  opener = active && 'focus' in active ? (active as HTMLElement) : null;
  if (canvas.item?.key !== item.key) canvas.tab = 'preview';
  canvas.item = item;
  openedIn = activeSession;
}

/** Zamyka kanwę bez przenoszenia fokusu (zmiana sesji, zniknięcie tury źródłowej). */
export function dismissCanvas(): void {
  canvas.item = null;
  opener = null;
}

export function closeCanvas(): void {
  const back = opener;
  dismissCanvas();
  if (back?.isConnected) requestAnimationFrame(() => back.focus());
}

/**
 * Wywoływane przez powłokę przy zmianie sesji i ścieżki rozmowy: kanwa należy do sesji, w której
 * ją otwarto, i do tury na widocznej ścieżce — inna sesja, nowa rozmowa, usunięta albo ponowiona
 * tura (inny wariant) zamyka ją, żeby „Zapisz” nie eksportował cudzej treści.
 * `onPath` — `null`, gdy rozmowa aktywnej sesji nie jest jeszcze wczytana (bez sprawdzania tur).
 */
export function syncCanvas(
  sessionId: string | null,
  onPath: ((turnId: string) => boolean) | null,
): void {
  if (sessionId !== activeSession) {
    activeSession = sessionId;
    if (canvas.item && openedIn !== sessionId) dismissCanvas();
    return;
  }
  const origin = canvas.item?.origin;
  if (!origin || origin.type === 'file' || !onPath) return;
  if (!onPath(origin.turnId)) dismissCanvas();
}
