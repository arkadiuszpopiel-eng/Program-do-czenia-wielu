// Wygląd okna z ustawień (PLAN §14.3): motyw, gęstość, ruch, zoom 80–200 % i dopasowanie skali do
// szerokości okna. Zoom przez CSS `zoom` na <html> — szerokość efektywna dla responsywności to
// innerWidth / skala.
import { clampZoom } from './logic/layout';
import { applySkin, applyTheme } from './window-boot';

export interface Appearance {
  readonly theme: string;
  /** Styl interfejsu (`ui.style`): `classic`, `glass`, `next`, `junior`. */
  readonly style?: string;
  readonly density: string;
  readonly animations: boolean;
  readonly zoom: number;
  /** Dopasowanie skali do okna (`ui.zoom_fit`). */
  readonly fit: boolean;
  /** Szerokość okna w px urządzenia logicznego (`innerWidth`). */
  readonly width: number;
}

/** Do tej szerokości okna interfejs ma skalę 1 — układ jest projektowany dla 1280–1600 px. */
export const FIT_FROM = 1600;
const FIT_STEPS: readonly (readonly [number, number])[] = [
  [FIT_FROM, 1],
  [2560, 1.25],
  [3840, 1.5],
];

/**
 * Skala dopasowania: na szerokich oknach interfejs rośnie razem z oknem (2560 px → 125 %,
 * 3840 px → 150 %), żeby tekst i kontrolki nie zostawały małe na dużym ekranie. Zaokrąglona
 * do 0,01, żeby drobna zmiana szerokości nie przeliczała układu.
 */
export function fitScale(width: number): number {
  const first = FIT_STEPS[0];
  const last = FIT_STEPS[FIT_STEPS.length - 1];
  if (!first || !last || !(width > first[0])) return 1;
  if (width >= last[0]) return last[1];
  for (let i = 1; i < FIT_STEPS.length; i++) {
    const a = FIT_STEPS[i - 1];
    const b = FIT_STEPS[i];
    if (a && b && width <= b[0]) {
      const part = (width - a[0]) / (b[0] - a[0]);
      return Math.round((a[1] + part * (b[1] - a[1])) * 100) / 100;
    }
  }
  return last[1];
}

/** Styl „Junior” (dla dzieci): cały interfejs większy — duże litery i przyciski w każdym widoku. */
export const JUNIOR_SCALE = 1.15;
/**
 * Najwęższy układ, dla którego projektowany jest interfejs (px efektywne). Powiększenie Juniora nie
 * schodzi poniżej — w oknie 360 px dawało 313 px i przyciski nachodziły na znaczniki paneli.
 */
export const JUNIOR_MIN_WIDTH = 360;

/** Skala całego interfejsu: zoom użytkownika razy dopasowanie do okna (razy skala stylu). */
export function effectiveScale(zoom: number, fit: boolean, width: number, style?: string): number {
  const base = (clampZoom(zoom) / 100) * (fit ? fitScale(width) : 1);
  if (style !== 'junior') return base;
  return base * Math.max(1, Math.min(JUNIOR_SCALE, width / (JUNIOR_MIN_WIDTH * base)));
}

export function applyAppearance(root: HTMLElement, a: Appearance): void {
  applyTheme(root, a.theme);
  applySkin(root, a.style);
  root.dataset['density'] = a.density === 'compact' ? 'compact' : 'comfortable';
  if (a.animations) delete root.dataset['motion'];
  else root.dataset['motion'] = 'off';
  const scale = String(effectiveScale(a.zoom, a.fit, a.width, a.style));
  root.style.setProperty('zoom', scale);
  // Dla rozmiarów liczonych od okna (`--alfa-vh`, `--alfa-vw` w app.css).
  root.style.setProperty('--alfa-zoom', scale);
}

export function effectiveWidth(win: Window, zoom: number, fit = false, style?: string): number {
  return Math.round(win.innerWidth / effectiveScale(zoom, fit, win.innerWidth, style));
}
