// Kanwa artefaktów (C2.8): otwarcie, powrót do karty Podgląd przy innym artefakcie, Esc z okna.
import { afterEach, describe, expect, it } from 'vitest';
import type { CanvasItem } from '../../logic/canvas';
import { canvas, closeCanvas, openCanvas } from '../canvas.svelte';
import { escapeChain } from '../commands';
import { setupApp } from './helpers';

const item = (key: string): CanvasItem => ({
  key,
  kind: 'html',
  title: null,
  source: '<p>a</p>',
  origin: { type: 'block', turnId: 's1:t1', blockIndex: 0 },
});

afterEach(() => closeCanvas());

describe('stan kanwy', () => {
  it('ten sam artefakt zachowuje kartę, inny wraca do podglądu', () => {
    openCanvas(item('a'));
    canvas.tab = 'source';
    openCanvas(item('a'));
    expect(canvas.tab).toBe('source');
    openCanvas(item('b'));
    expect(canvas.item?.key).toBe('b');
    expect(canvas.tab).toBe('preview');
    closeCanvas();
    expect(canvas.item).toBeNull();
  });

  it('Esc w oknie zamyka kanwę przed panelami', async () => {
    const { app } = setupApp();
    await app.start();
    openCanvas(item('a'));
    expect(escapeChain(app)).toBe(true);
    expect(canvas.item).toBeNull();
  });
});
