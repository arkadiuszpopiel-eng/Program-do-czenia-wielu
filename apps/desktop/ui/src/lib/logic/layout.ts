// Responsywność okna (PLAN §14.2) i ograniczenia szerokości paneli.

export type LayoutMode = 'wide' | 'medium' | 'narrow' | 'compact';
export type PanelPlacement = 'docked' | 'drawer' | 'sheet' | 'hidden';

export const LEFT_MIN = 240;
export const LEFT_MAX = 320;
export const LEFT_RAIL = 48;
export const RIGHT_MIN = 300;
export const RIGHT_MAX = 520;
export const ZOOM_MIN = 80;
export const ZOOM_MAX = 200;
export const ZOOM_STEP = 10;

/** Szerokość efektywna (po zoomie) → tryb układu. */
export function layoutMode(width: number): LayoutMode {
  if (width >= 1440) return 'wide';
  if (width >= 1100) return 'medium';
  if (width >= 720) return 'narrow';
  return 'compact';
}

export interface PanelLayout {
  readonly left: PanelPlacement;
  readonly right: PanelPlacement;
}

/**
 * - ≥ 1440: oba panele zadokowane;
 * - 1100–1440: jeden zadokowany (lewy ma pierwszeństwo), drugi jako szuflada;
 * - 720–1100: oba jako szuflady nad treścią;
 * - < 720: arkusze pełnoekranowe (najwyżej jeden naraz — prawy wygrywa jako ostatnio otwarty).
 */
export function resolvePanels(
  mode: LayoutMode,
  leftOpen: boolean,
  rightOpen: boolean,
  focus: boolean,
): PanelLayout {
  if (focus) return { left: 'hidden', right: 'hidden' };
  const hide = (open: boolean, placement: PanelPlacement): PanelPlacement =>
    open ? placement : 'hidden';
  switch (mode) {
    case 'wide':
      return { left: hide(leftOpen, 'docked'), right: hide(rightOpen, 'docked') };
    case 'medium':
      return {
        left: hide(leftOpen, 'docked'),
        right: hide(rightOpen, leftOpen ? 'drawer' : 'docked'),
      };
    case 'narrow':
      return {
        left: hide(leftOpen && !rightOpen, 'drawer'),
        right: hide(rightOpen, 'drawer'),
      };
    case 'compact':
      return {
        left: hide(leftOpen && !rightOpen, 'sheet'),
        right: hide(rightOpen, 'sheet'),
      };
  }
}

export const clamp = (value: number, min: number, max: number): number =>
  Math.min(max, Math.max(min, Math.round(value)));

export const clampLeft = (width: number): number => clamp(width, LEFT_MIN, LEFT_MAX);
export const clampRight = (width: number): number => clamp(width, RIGHT_MIN, RIGHT_MAX);
export const clampZoom = (zoom: number): number => clamp(zoom, ZOOM_MIN, ZOOM_MAX);

export function stepZoom(zoom: number, direction: 1 | -1 | 0): number {
  if (direction === 0) return 100;
  return clampZoom(Math.round(zoom / ZOOM_STEP) * ZOOM_STEP + direction * ZOOM_STEP);
}
