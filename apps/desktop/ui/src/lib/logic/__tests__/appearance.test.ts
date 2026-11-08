import { describe, expect, it } from 'vitest';
import { effectiveScale, effectiveWidth, fitScale } from '../../appearance';
import { layoutMode, nextColumn } from '../layout';

describe('dopasowanie skali do okna (ui.zoom_fit)', () => {
  it('do 1600 px skala 1 — laptop 1080p przy 125–150 % i okna pomniejszone bez zmian', () => {
    expect(fitScale(400)).toBe(1);
    expect(fitScale(1280)).toBe(1);
    expect(fitScale(1536)).toBe(1);
    expect(fitScale(1600)).toBe(1);
  });

  it('powyżej rośnie liniowo: 2560 px → 125 %, 3840 px → 150 %, dalej stała', () => {
    expect(fitScale(1920)).toBe(1.08);
    expect(fitScale(2560)).toBe(1.25);
    expect(fitScale(3200)).toBe(1.38);
    expect(fitScale(3840)).toBe(1.5);
    expect(fitScale(5120)).toBe(1.5);
  });

  it('skala nigdy nie maleje wraz z szerokością, a szerokość efektywna nie spada', () => {
    let scale = 0;
    let effective = 0;
    for (let w = 400; w <= 5200; w += 40) {
      const s = fitScale(w);
      expect(s).toBeGreaterThanOrEqual(scale);
      expect(w / s).toBeGreaterThanOrEqual(effective - 1);
      scale = s;
      effective = w / s;
    }
  });

  it('zoom użytkownika mnoży się z dopasowaniem; wyłączone dopasowanie zostawia sam zoom', () => {
    expect(effectiveScale(100, true, 2560)).toBe(1.25);
    expect(effectiveScale(120, true, 2560)).toBeCloseTo(1.5);
    expect(effectiveScale(120, false, 2560)).toBe(1.2);
    expect(effectiveScale(100, true, 1280)).toBe(1);
  });

  it('szerokie okno zostaje w trybie „wide” po przeskalowaniu (oba panele zadokowane)', () => {
    for (const w of [1920, 2560, 3440, 3840]) {
      const width = effectiveWidth({ innerWidth: w } as Window, 100, true);
      expect(layoutMode(width)).toBe('wide');
    }
    expect(effectiveWidth({ innerWidth: 2560 } as Window, 100, true)).toBe(2048);
    expect(effectiveWidth({ innerWidth: 2560 } as Window, 100)).toBe(2560);
  });
});

describe('szerokość kolumny rozmowy (ui.column)', () => {
  it('paleta przełącza dopasowana → wąska → szeroka → dopasowana', () => {
    expect(nextColumn('auto')).toBe('narrow');
    expect(nextColumn('narrow')).toBe('wide');
    expect(nextColumn('wide')).toBe('auto');
    expect(nextColumn('nieznana')).toBe('auto');
  });
});
