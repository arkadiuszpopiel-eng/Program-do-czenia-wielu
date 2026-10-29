// Responsywność okna (PLAN §14.2) i ograniczenia szerokości paneli.

export type LayoutMode = 'wide' | 'medium' | 'narrow' | 'compact';
/**
 * `peek` — panel wysunięty chwilowo nad rozmową (najechanie na znacznik przy krawędzi okna).
 * Paska ikon już nie ma (właściciel 29.10: panele „w pełni chowane i wysuwane”) — schowany panel
 * zostawia tylko znacznik przy krawędzi.
 */
export type PanelPlacement = 'docked' | 'drawer' | 'sheet' | 'peek' | 'hidden';
export type PanelSide = 'left' | 'right';

export const LEFT_MIN = 240;
export const LEFT_MAX = 320;
export const RIGHT_MIN = 300;
export const RIGHT_MAX = 520;
/** Domyślna szerokość prawego panelu i szerokość na dużym oknie (mieści wszystkie karty w jednym wierszu). */
export const RIGHT_DEFAULT = 360;
export const RIGHT_WIDE = 460;
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
 * - 1100–1440: zadokowany jeden panel; gdy otwarte są oba, lewy chowa się za znacznik przy krawędzi
 *   (wysuwa się nad rozmowę), żeby rozmowa nie była za wąska (laptop 1080p przy 150 % to 1280 px);
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
        left: hide(leftOpen && !rightOpen, 'docked'),
        right: hide(rightOpen, 'docked'),
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

/**
 * Panele chowane (właściciel 27.10: „mają się chować, wysuwać i mieć znaczniki, że tam są”). Na
 * szerokim i średnim oknie schowany panel wysuwa się nad rozmowę po najechaniu na znacznik przy
 * krawędzi; z `autohide` panele są zawsze schowane i tylko się wysuwają. Szuflady i arkusze
 * węższych okien zostają bez zmian.
 */
export function withPeek(
  base: PanelLayout,
  mode: LayoutMode,
  focus: boolean,
  autohide: boolean,
  peek: PanelSide | null,
): PanelLayout {
  if (focus || (mode !== 'wide' && mode !== 'medium')) return base;
  const side = (s: PanelSide, placement: PanelPlacement): PanelPlacement => {
    if (peek === s && (autohide || placement === 'hidden')) return 'peek';
    return autohide ? 'hidden' : placement;
  };
  return { left: side('left', base.left), right: side('right', base.right) };
}

export const clamp = (value: number, min: number, max: number): number =>
  Math.min(max, Math.max(min, Math.round(value)));

export const clampLeft = (width: number): number => clamp(width, LEFT_MIN, LEFT_MAX);
export const clampRight = (width: number): number => clamp(width, RIGHT_MIN, RIGHT_MAX);
/**
 * Szerokość zadokowanego prawego panelu: na szerokim oknie panel o domyślnej szerokości rośnie,
 * żeby karty nie łamały się na dwa wiersze; szerokość ustawiona przez użytkownika zostaje.
 */
export const dockedRightWidth = (stored: number, mode: LayoutMode): number =>
  mode === 'wide' && stored === RIGHT_DEFAULT ? RIGHT_WIDE : stored;
export const clampZoom = (zoom: number): number => clamp(zoom, ZOOM_MIN, ZOOM_MAX);

export function stepZoom(zoom: number, direction: 1 | -1 | 0): number {
  if (direction === 0) return 100;
  return clampZoom(Math.round(zoom / ZOOM_STEP) * ZOOM_STEP + direction * ZOOM_STEP);
}

/** Szerokości kolumny rozmowy (`ui.column`) w kolejności przełączania z palety. */
export const COLUMN_MODES = ['auto', 'narrow', 'wide'] as const;

export function nextColumn(current: string): string {
  const i = COLUMN_MODES.indexOf(current as (typeof COLUMN_MODES)[number]);
  return COLUMN_MODES[(i + 1) % COLUMN_MODES.length] ?? 'auto';
}
