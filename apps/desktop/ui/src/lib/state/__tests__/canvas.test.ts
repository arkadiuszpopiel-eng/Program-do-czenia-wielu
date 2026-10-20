// Kanwa artefaktów (C2.8): otwarcie, powrót do karty Podgląd przy innym artefakcie, Esc z okna.
import { afterEach, describe, expect, it } from 'vitest';
import type { CanvasItem } from '../../logic/canvas';
import { canvas, closeCanvas, openCanvas, syncCanvas } from '../canvas.svelte';
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

  it('zmiana sesji i zniknięcie tury z widocznej ścieżki zamykają kanwę', () => {
    syncCanvas('s1', null);
    openCanvas(item('a'));
    syncCanvas('s1', (id) => id === 's1:t1');
    expect(canvas.item?.key).toBe('a');
    // Ponowienie / usunięcie: tury źródłowej nie ma już na ścieżce.
    syncCanvas('s1', () => false);
    expect(canvas.item).toBeNull();
    openCanvas(item('b'));
    syncCanvas('s2', null);
    expect(canvas.item).toBeNull();
    // Plik sesji nie zależy od tur, ale od sesji — tak.
    openCanvas({ ...item('c'), origin: { type: 'file', artifactId: 'a3' } });
    syncCanvas('s2', () => false);
    expect(canvas.item?.key).toBe('c');
    syncCanvas(null, null);
    expect(canvas.item).toBeNull();
  });
});
