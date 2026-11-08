import { describe, expect, it } from 'vitest';
import {
  RIGHT_DEFAULT,
  RIGHT_WIDE,
  clampLeft,
  clampRight,
  dockedRightWidth,
  layoutMode,
  resolvePanels,
  stepZoom,
  withPeek,
} from '../layout';
import { LayoutState } from '../../state/layout.svelte';

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
    // Średnie okno: oba panele się nie mieszczą — lewy chowa się za znacznik (bez paska ikon).
    expect(resolvePanels('medium', true, true, false)).toEqual({ left: 'hidden', right: 'docked' });
    expect(resolvePanels('medium', true, false, false)).toEqual({
      left: 'docked',
      right: 'hidden',
    });
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

describe('szerokość prawego panelu', () => {
  it('na szerokim oknie domyślny panel rośnie, a ustawiony przez użytkownika zostaje', () => {
    expect(dockedRightWidth(RIGHT_DEFAULT, 'wide')).toBe(RIGHT_WIDE);
    expect(dockedRightWidth(RIGHT_DEFAULT, 'medium')).toBe(RIGHT_DEFAULT);
    expect(dockedRightWidth(320, 'wide')).toBe(320);
    expect(dockedRightWidth(500, 'wide')).toBe(500);
  });
});

describe('panele chowane (znacznik przy krawędzi)', () => {
  const both = { left: 'docked', right: 'docked' } as const;
  const none = { left: 'hidden', right: 'hidden' } as const;

  it('schowany panel wysuwa się nad rozmowę tylko na szerokim i średnim oknie', () => {
    expect(withPeek(none, 'wide', false, false, 'left')).toEqual({ left: 'peek', right: 'hidden' });
    expect(withPeek(none, 'medium', false, false, 'right')).toEqual({
      left: 'hidden',
      right: 'peek',
    });
    expect(withPeek(none, 'narrow', false, false, 'left')).toEqual(none);
    expect(withPeek(none, 'wide', true, false, 'left')).toEqual(none);
  });

  it('zadokowany panel bez autohide zostaje na miejscu', () => {
    expect(withPeek(both, 'wide', false, false, 'left')).toEqual(both);
  });

  it('autohide chowa zadokowane panele, wysuwa tylko jeden', () => {
    expect(withPeek(both, 'wide', false, true, null)).toEqual(none);
    expect(withPeek(both, 'wide', false, true, 'right')).toEqual({ left: 'hidden', right: 'peek' });
  });

  it('autohide: przełącznik w pasku i „otwórz” wysuwają panel, zamiast zmieniać układ', () => {
    const layout = new LayoutState();
    layout.width = 1600;
    layout.autohide = true;
    layout.toggleLeft('s1');
    expect(layout.placement('s1').left).toBe('peek');
    layout.toggleLeft('s1');
    expect(layout.placement('s1').left).toBe('hidden');
    layout.openTab('s1', 'agents');
    expect(layout.placement('s1')).toEqual({ left: 'hidden', right: 'peek' });
    layout.update('s1', { right_open: false });
    expect(layout.placement('s1').right).toBe('hidden');
  });
});
