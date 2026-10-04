// Wygląd okna z ustawień (PLAN §14.3): motyw, gęstość, ruch, zoom 80–200 %. Zoom przez CSS `zoom`
// na <html> — szerokość efektywna dla responsywności to innerWidth / zoom.
import { clampZoom } from './logic/layout';
import { applyTheme } from './window-boot';

export interface Appearance {
  readonly theme: string;
  readonly density: string;
  readonly animations: boolean;
  readonly zoom: number;
}

export function applyAppearance(root: HTMLElement, a: Appearance): void {
  applyTheme(root, a.theme);
  root.dataset['density'] = a.density === 'compact' ? 'compact' : 'comfortable';
  if (a.animations) delete root.dataset['motion'];
  else root.dataset['motion'] = 'off';
  root.style.setProperty('zoom', String(clampZoom(a.zoom) / 100));
}

export function effectiveWidth(win: Window, zoom: number): number {
  return Math.round(win.innerWidth / (clampZoom(zoom) / 100));
}
