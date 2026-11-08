import { describe, expect, it } from 'vitest';
import { ManualFrames, RafBatcher } from '../raf-batcher';

describe('RafBatcher — aktualizacja DOM najwyżej raz na klatkę', () => {
  it('łączy wszystkie zdarzenia z jednej klatki w jedną paczkę', () => {
    const frames = new ManualFrames();
    const batches: number[][] = [];
    const batcher = new RafBatcher<number>((b) => batches.push([...b]), { frames });
    for (let i = 0; i < 100; i++) batcher.push(i);
    expect(frames.scheduled).toBe(1);
    expect(batches).toHaveLength(0);
    frames.tick();
    expect(batches).toHaveLength(1);
    expect(batches[0]).toHaveLength(100);
  });

  it('nie planuje klatki, gdy nic nie przyszło', () => {
    const frames = new ManualFrames();
    const batcher = new RafBatcher<number>(() => undefined, { frames });
    batcher.push();
    expect(frames.scheduled).toBe(0);
    expect(batcher.pending).toBe(0);
  });

  it('kolejne klatki dostają tylko nowe zdarzenia', () => {
    const frames = new ManualFrames();
    const batches: number[][] = [];
    const batcher = new RafBatcher<number>((b) => batches.push([...b]), { frames });
    batcher.push(1, 2);
    frames.tick();
    batcher.push(3);
    frames.tick();
    expect(batches).toEqual([[1, 2], [3]]);
  });

  it('opróżnia od razu po przekroczeniu limitu kolejki (okno ukryte, rAF wstrzymany)', () => {
    const frames = new ManualFrames();
    const batches: number[][] = [];
    const batcher = new RafBatcher<number>((b) => batches.push([...b]), { frames, maxQueue: 3 });
    batcher.push(1, 2, 3);
    expect(batches).toEqual([[1, 2, 3]]);
    expect(frames.scheduled).toBe(0);
  });

  it('flushNow i dispose', () => {
    const frames = new ManualFrames();
    const batches: number[][] = [];
    const batcher = new RafBatcher<number>((b) => batches.push([...b]), { frames });
    batcher.push(1);
    batcher.flushNow();
    expect(batches).toEqual([[1]]);
    batcher.push(2);
    batcher.dispose();
    frames.tick();
    batcher.push(3);
    expect(batches).toEqual([[1]]);
  });
});
