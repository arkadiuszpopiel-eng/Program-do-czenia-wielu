import { describe, expect, it } from 'vitest';
import { clampLeft, clampRight, layoutMode, resolvePanels, stepZoom } from '../layout';

describe('responsywność §14.2', () => {
  it('progi szerokości', () => {
    expect(layoutMode(1920)).toBe('wide');
    expect(layoutMode(1440)).toBe('wide');
    expect(layoutMode(1280)).toBe('medium');
    expect(layoutMode(1100)).toBe('medium');
    expect(layoutMode(900)).toBe('narrow');
    expect(layoutMode(719)).toBe('compact');
  });

  it('rozmieszczenie paneli', () => {
    expect(resolvePanels('wide', true, true, false)).toEqual({ left: 'docked', right: 'docked' });
    expect(resolvePanels('medium', true, true, false)).toEqual({ left: 'docked', right: 'drawer' });
    expect(resolvePanels('medium', false, true, false)).toEqual({
      left: 'hidden',
      right: 'docked',
    });
    expect(resolvePanels('narrow', true, false, false)).toEqual({
      left: 'drawer',
      right: 'hidden',
    });
    expect(resolvePanels('narrow', true, true, false)).toEqual({ left: 'hidden', right: 'drawer' });
    expect(resolvePanels('compact', false, true, false)).toEqual({
      left: 'hidden',
      right: 'sheet',
    });
    expect(resolvePanels('wide', true, true, true)).toEqual({ left: 'hidden', right: 'hidden' });
  });

  it('granice szerokości i zoomu', () => {
    expect(clampLeft(100)).toBe(240);
    expect(clampLeft(400)).toBe(320);
    expect(clampRight(1000)).toBe(520);
    expect(stepZoom(100, 1)).toBe(110);
    expect(stepZoom(200, 1)).toBe(200);
    expect(stepZoom(80, -1)).toBe(80);
    expect(stepZoom(150, 0)).toBe(100);
  });
});
